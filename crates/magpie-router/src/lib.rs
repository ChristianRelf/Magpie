//! Model selection. Pure and synchronous: given the request, the model
//! inventory, limits, history and configuration, produce a ranked and
//! explained [`RoutingDecision`].

pub mod classify;

use std::collections::{BTreeMap, HashMap};

use magpie_core::*;

pub use classify::classify;

/// Historical performance of a model through the harness.
#[derive(Debug, Clone, Default)]
pub struct ModelHistory {
    pub requests: u64,
    pub failures: u64,
    pub avg_duration_ms: Option<f64>,
    pub output_tps: Option<f64>,
}

pub struct RouteInput<'a> {
    pub request: &'a ExecRequest,
    pub models: &'a [ModelInfo],
    pub config: &'a RoutingConfig,
    /// Current limit windows across accounts.
    pub limits: &'a [LimitWindow],
    pub history: &'a HashMap<String, ModelHistory>,
    pub now: Timestamp,
}

#[derive(Debug, Clone, Copy)]
struct Weights {
    quality: f64,
    speed: f64,
    cost: f64,
    headroom: f64,
}

fn weights(preset: RoutingPreset, complexity: Complexity, bias: f64) -> Weights {
    let w = match preset {
        RoutingPreset::BestQuality => Weights { quality: 0.75, speed: 0.05, cost: 0.05, headroom: 0.15 },
        RoutingPreset::Fastest => Weights { quality: 0.15, speed: 0.65, cost: 0.1, headroom: 0.1 },
        RoutingPreset::Economical => Weights { quality: 0.2, speed: 0.1, cost: 0.6, headroom: 0.1 },
        RoutingPreset::PreserveLimits => Weights { quality: 0.25, speed: 0.1, cost: 0.15, headroom: 0.5 },
        RoutingPreset::Automatic | RoutingPreset::Manual => match complexity {
            Complexity::Low => Weights { quality: 0.25, speed: 0.25, cost: 0.3, headroom: 0.2 },
            Complexity::Medium => Weights { quality: 0.4, speed: 0.2, cost: 0.2, headroom: 0.2 },
            Complexity::High => Weights { quality: 0.6, speed: 0.1, cost: 0.1, headroom: 0.2 },
        },
    };
    if preset != RoutingPreset::Automatic || bias == 0.0 {
        return w;
    }
    // Quality bias shifts weight between quality and cost.
    let shift = (bias.clamp(-1.0, 1.0) * 0.2).clamp(-w.quality + 0.05, w.cost - 0.02);
    Weights { quality: w.quality + shift, cost: w.cost - shift, ..w }
}

fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0).replace(".0M", "M")
    } else if n >= 1_000 {
        format!("{}k", n / 1_000)
    } else {
        n.to_string()
    }
}

fn selected(m: &ModelInfo) -> SelectedModel {
    SelectedModel {
        key: m.key.clone(),
        account_id: m.account_id.clone(),
        provider: m.provider,
        model_id: m.model.model_id.clone(),
        display_name: m.model.display_name.clone(),
    }
}

/// Windows relevant to a model on its account.
fn model_windows<'a>(m: &ModelInfo, limits: &'a [LimitWindow]) -> Vec<&'a LimitWindow> {
    limits
        .iter()
        .filter(|w| w.account_id == m.account_id)
        .filter(|w| match &w.model_scope {
            None => true,
            Some(scope) => scope == &m.model.model_id || m.model.model_id.contains(scope.as_str()) || scope.contains(&m.model.model_id),
        })
        .collect()
}

fn matches_model_ref(m: &ModelInfo, r: &str) -> bool {
    let r = r.trim();
    m.key == r
        || m.public_id() == r
        || m.model.model_id == r
        || m.model.model_id.eq_ignore_ascii_case(r)
        || m.model.display_name.eq_ignore_ascii_case(r)
}

/// Resolve explicit model references (`model` field or Manual preset).
fn pinned_models<'a>(input: &RouteInput<'a>, preset: RoutingPreset) -> Option<(String, Vec<&'a ModelInfo>)> {
    let req_model = input.request.model.trim();
    let reference = if !req_model.is_empty() && !req_model.eq_ignore_ascii_case("auto") {
        Some(req_model.to_string())
    } else if preset == RoutingPreset::Manual {
        input.config.manual_model.clone()
    } else {
        None
    };
    let reference = reference?;
    let found: Vec<&ModelInfo> = input.models.iter().filter(|m| matches_model_ref(m, &reference)).collect();
    Some((reference, found))
}

pub fn route(input: &RouteInput) -> HarnessResult<RoutingDecision> {
    let req = input.request;
    let cfg = input.config;
    let mut cl = classify(req);
    let rule = cfg.task_rules.iter().find(|r| r.task == cl.task);
    let preset = req.preferences.preset.or(rule.and_then(|r| r.preset)).unwrap_or(cfg.preset);
    let allow_fallback = req.preferences.allow_fallback.unwrap_or(cfg.allow_fallback);
    let allow_billable_req = req.preferences.allow_billable;
    let max_cost = req.preferences.max_cost_usd.or(cfg.max_cost_per_request_usd);

    let pinned = pinned_models(input, preset);
    if let Some((reference, found)) = &pinned {
        if found.is_empty() {
            return Err(HarnessError::new(ErrorKind::ModelNotFound, format!("No connected model matches \"{reference}\"")));
        }
    }
    let pinned_keys: Vec<String> = pinned.as_ref().map(|(_, f)| f.iter().map(|m| m.key.clone()).collect()).unwrap_or_default();

    let w = weights(preset, cl.complexity, cfg.quality_bias);
    let needed_context = cl.estimated_input_tokens + cl.estimated_output_tokens;
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut rejected: Vec<Rejection> = Vec::new();

    for m in input.models {
        let is_pinned = pinned_keys.contains(&m.key);
        if pinned.is_some() && !is_pinned && !allow_fallback {
            continue;
        }
        let reject = |reason: String, rejected: &mut Vec<Rejection>| {
            rejected.push(Rejection { model_key: m.key.clone(), display_name: m.model.display_name.clone(), reason });
        };
        if m.preference.disabled {
            reject("Disabled in model settings".into(), &mut rejected);
            continue;
        }
        if !m.available {
            reject(m.unavailable_reason.clone().unwrap_or_else(|| "Account not connected".into()), &mut rejected);
            continue;
        }
        if !req.preferences.providers.is_empty() && !req.preferences.providers.contains(&m.provider) {
            continue;
        }
        if req.preferences.exclude_providers.contains(&m.provider) {
            continue;
        }
        if !m.model.capabilities.satisfies(&cl.required) {
            reject(format!("Lacks {}", m.model.capabilities.missing(&cl.required).join(", ")), &mut rejected);
            continue;
        }
        // These CLIs expose built-in filesystem tools and user configuration.
        // A scratch cwd alone is not an isolation boundary for plain generation.
        if matches!(m.provider, ProviderKind::CodexCli | ProviderKind::GeminiCli) && req.agent.is_none() {
            reject("Requires an explicit agent working directory and agent permission".into(), &mut rejected);
            continue;
        }
        if let Some(ctx) = m.model.context_window {
            if needed_context > ctx {
                reject(
                    format!("Context window too small ({} available, ~{} needed)", fmt_tokens(ctx), fmt_tokens(needed_context)),
                    &mut rejected,
                );
                continue;
            }
        }
        let windows = model_windows(m, input.limits);
        if let Some(block) = windows.iter().find(|w| w.blocks(input.now)) {
            let until = block.resets_at.map(|r| format!(" until {}", r.format("%H:%M UTC %b %d"))).unwrap_or_default();
            reject(format!("{} exhausted{until}", block.label), &mut rejected);
            continue;
        }
        let billable = m.billing_mode.is_billable();
        if billable && !cfg.allow_metered && allow_billable_req != Some(true) && !is_pinned {
            reject("Metered API usage is disabled for automatic routing".into(), &mut rejected);
            continue;
        }
        if allow_billable_req == Some(false) && billable {
            reject("Request does not permit billable usage".into(), &mut rejected);
            continue;
        }

        let pricing = m.pricing();
        let est_cost = pricing.map(|p| p.cost(cl.estimated_input_tokens, 0, cl.estimated_output_tokens));
        if let (Some(limit), Some(cost), true) = (max_cost, est_cost, billable) {
            if cost > limit {
                reject(format!("Estimated cost ${cost:.4} exceeds limit ${limit:.4}"), &mut rejected);
                continue;
            }
        }

        // Known remaining allowance (most constrained window).
        let remaining = windows
            .iter()
            .filter(|w| w.provenance == Provenance::Reported && w.resets_at.is_none_or(|reset| reset > input.now))
            .filter_map(|w| w.remaining_fraction())
            .fold(None, |acc: Option<f64>, r| Some(acc.map_or(r, |a| a.min(r))));
        let reserve =
            m.preference.reserve_percent.or(if m.model.tier == QualityTier::Frontier { cfg.preserve_premium_percent } else { None });
        if let (Some(pct), Some(rem)) = (reserve, remaining) {
            if cl.complexity != Complexity::High && rem * 100.0 < pct && !is_pinned {
                reject(format!("Reserving last {pct:.0}% of allowance for complex tasks ({:.0}% left)", rem * 100.0), &mut rejected);
                continue;
            }
        }

        // ---- scoring
        let mut components = BTreeMap::new();
        let mut reasons: Vec<String> = Vec::new();
        let history = input.history.get(&m.key);

        let mut quality = m.model.tier.score();
        let reasoning_task =
            matches!(cl.task, TaskClass::MathReasoning | TaskClass::Planning | TaskClass::Debugging | TaskClass::RepositoryAnalysis);
        if reasoning_task && m.model.capabilities.reasoning {
            quality = (quality + 0.08).min(1.0);
        }
        if matches!(cl.task, TaskClass::CodeGeneration | TaskClass::Debugging | TaskClass::RepositoryAnalysis)
            && m.model.capabilities.agentic
        {
            quality = (quality + 0.04).min(1.0);
        }
        if m.model.tier >= QualityTier::High && cl.task != TaskClass::SimpleQuestion {
            reasons.push(format!("Strong fit for {}", cl.task.label().to_lowercase()));
        }

        let measured_speed = history.filter(|h| h.requests >= 5).and_then(|h| h.output_tps).map(|tps| (tps / 120.0).clamp(0.1, 1.0));
        let speed = measured_speed.unwrap_or_else(|| m.model.speed.score());
        if measured_speed.is_some() {
            components.insert("speed_measured".into(), 1.0);
        }

        let cost_score = match m.billing_mode {
            BillingMode::Local => 1.0,
            BillingMode::Subscription => 0.9,
            _ => match est_cost {
                Some(c) => 1.0 / (1.0 + c / 0.01),
                None => 0.4,
            },
        };
        match m.billing_mode {
            BillingMode::Local => reasons.push("Runs locally".into()),
            BillingMode::Subscription => reasons.push("No marginal cost (subscription)".into()),
            _ => {
                if let Some(c) = est_cost {
                    reasons.push(format!("Estimated cost ${c:.4}"));
                }
            }
        }

        let headroom = match (m.billing_mode, remaining) {
            (BillingMode::Local, _) => 1.0,
            (_, Some(r)) => r,
            (BillingMode::Subscription, None) => 0.7,
            (_, None) => 0.8,
        };
        if let Some(r) = remaining {
            reasons.push(format!("{:.0}% of known allowance available", r * 100.0));
        } else if m.billing_mode != BillingMode::Local {
            reasons.push("Provider allowance not reported".into());
        }

        let reliability = history.map(|h| (h.requests - h.failures.min(h.requests) + 9) as f64 / (h.requests + 10) as f64).unwrap_or(0.9);

        let mut score = w.quality * quality + w.speed * speed + w.cost * cost_score + w.headroom * headroom;
        score *= 0.7 + 0.3 * reliability;

        // Overkill penalty: keep premium models for work that needs them.
        if preset == RoutingPreset::Automatic && cl.complexity == Complexity::Low && m.model.tier == QualityTier::Frontier {
            score -= 0.05;
        }

        let mut bonus = 0.0;
        if is_pinned {
            bonus += 10.0;
            reasons.insert(0, "Requested explicitly".into());
        }
        if let Some(r) = rule {
            if let Some(pos) = r.models.iter().position(|k| k == &m.key) {
                bonus += 0.3 - pos as f64 * 0.02;
                reasons.insert(0, format!("Preferred for {} by routing rule", cl.task.label().to_lowercase()));
            }
        }
        if m.preference.priority != 0 {
            bonus += (m.preference.priority.clamp(-10, 10) as f64) * 0.015;
        }
        if m.preference.favourite {
            bonus += 0.03;
        }
        if let Some(pos) = cfg.provider_order.iter().position(|p| *p == m.provider) {
            bonus += 0.08 * (1.0 - pos as f64 / cfg.provider_order.len().max(1) as f64);
        }
        if let Some(pos) = cfg.fallback_order.iter().position(|k| k == &m.key) {
            bonus += 0.04 * (1.0 - pos as f64 / cfg.fallback_order.len().max(1) as f64);
        }
        if let Some(ctx) = m.model.context_window {
            if cl.estimated_input_tokens > 20_000 {
                reasons.push(format!("Sufficient context ({} available, ~{} needed)", fmt_tokens(ctx), fmt_tokens(needed_context)));
            }
        }
        if m.preference.favourite {
            reasons.push("Favourite model".into());
        }

        components.insert("quality".into(), quality);
        components.insert("speed".into(), speed);
        components.insert("cost".into(), cost_score);
        components.insert("headroom".into(), headroom);
        components.insert("reliability".into(), reliability);
        components.insert("bonus".into(), bonus);
        reasons.truncate(5);
        candidates.push(Candidate {
            model: selected(m),
            score: score + bonus,
            components,
            reasons,
            estimated_cost_usd: est_cost,
            billable,
        });
    }

    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Never fall back from subscription/local execution onto billable API
    // usage without explicit opt-in.
    if let Some(first) = candidates.first() {
        if !first.billable && !cfg.allow_subscription_to_api && allow_billable_req != Some(true) {
            let (keep, drop): (Vec<_>, Vec<_>) = candidates.into_iter().partition(|c| !c.billable);
            for c in drop {
                rejected.push(Rejection {
                    model_key: c.model.key.clone(),
                    display_name: c.model.display_name.clone(),
                    reason: "Fallback from subscription to billable API not permitted".into(),
                });
            }
            candidates = keep;
        }
    }
    if !allow_fallback {
        candidates.truncate(1);
    }

    if candidates.is_empty() {
        let detail = rejected.iter().take(4).map(|r| format!("{}: {}", r.display_name, r.reason)).collect::<Vec<_>>().join("; ");
        let msg = if input.models.is_empty() {
            "No models are available. Connect a provider in Magpie.".to_string()
        } else if detail.is_empty() {
            "No model satisfies the request constraints".to_string()
        } else {
            format!("No eligible model. {detail}")
        };
        return Err(HarnessError::new(ErrorKind::NoEligibleModel, msg));
    }
    cl.signals.truncate(6);
    Ok(RoutingDecision { preset, classification: cl, candidates, rejected, allow_fallback })
}

#[cfg(test)]
mod tests;
