use std::sync::Arc;
use std::time::{Duration, Instant};

use magpie_core::*;
use magpie_providers::AdapterRequest;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{ActiveExecution, ExecutionSummary, Harness, HarnessEvent};

/// Maximum attempts per execution (initial + retries + fallbacks).
const MAX_ATTEMPTS: usize = 4;

pub struct ExecutionHandle {
    pub id: String,
    pub events: mpsc::Receiver<ExecEvent>,
}

impl Harness {
    fn route_input_snapshot(
        &self,
    ) -> (Vec<ModelInfo>, Vec<LimitWindow>, std::collections::HashMap<String, magpie_router::ModelHistory>, RoutingConfig) {
        let cfg = self.routing_config();
        let history = if cfg.learn_from_history { self.history.read().clone() } else { Default::default() };
        (self.list_models(), self.limit_windows(), history, cfg)
    }

    /// Explain how a request would be routed, without executing it.
    pub fn route_preview(&self, req: &ExecRequest) -> HarnessResult<RoutingDecision> {
        req.validate()?;
        let (models, limits, history, cfg) = self.route_input_snapshot();
        magpie_router::route(&magpie_router::RouteInput {
            request: req,
            models: &models,
            config: &cfg,
            limits: &limits,
            history: &history,
            now: now(),
        })
    }

    pub fn active_executions(&self) -> Vec<ExecutionSummary> {
        self.active.read().values().map(|a| a.summary.clone()).collect()
    }

    pub fn cancel(&self, id: &str) -> bool {
        match self.active.read().get(id) {
            Some(a) => {
                a.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Start an execution. Routing failures are returned immediately (and
    /// recorded); provider failures arrive as `ExecEvent::Failed`.
    pub async fn execute(self: &Arc<Self>, req: ExecRequest, client: String) -> HarnessResult<ExecutionHandle> {
        req.validate()?;
        let id = new_id("exe");
        let decision = match self.route_preview(&req) {
            Ok(d) => d,
            Err(e) => {
                self.record_routing_failure(&id, &req, &client, &e);
                return Err(e);
            }
        };
        let (tx, rx) = mpsc::channel(256);
        let cancel = self.shutdown.child_token();
        let this = self.clone();
        let id2 = id.clone();
        tokio::spawn(async move { this.run(id2, req, client, decision, tx, cancel).await });
        Ok(ExecutionHandle { id, events: rx })
    }

    /// Execute and wait for the final result.
    pub async fn execute_collect(self: &Arc<Self>, req: ExecRequest, client: String) -> HarnessResult<(ExecResult, Vec<ExecEvent>)> {
        let mut handle = self.execute(req, client).await?;
        let mut notable = Vec::new();
        while let Some(ev) = handle.events.recv().await {
            match ev {
                ExecEvent::Completed { result } => return Ok((result, notable)),
                ExecEvent::Failed { error, .. } => return Err(error),
                e @ (ExecEvent::Started { .. } | ExecEvent::RoutingChanged { .. }) => notable.push(e),
                _ => {}
            }
        }
        Err(HarnessError::internal("Execution ended without a result"))
    }

    fn record_routing_failure(&self, id: &str, req: &ExecRequest, client: &str, e: &HarnessError) {
        if !self.settings.read().analytics.local_history {
            return;
        }
        let cl = magpie_router::classify(req);
        let rec = ExecutionRecord {
            id: id.to_string(),
            created_at: now(),
            completed_at: Some(now()),
            client: client.to_string(),
            status: ExecutionStatus::Failed,
            task: cl.task,
            complexity: cl.complexity,
            preset: req.preferences.preset.unwrap_or(self.routing.read().preset),
            model: None,
            usage: TokenUsage::default(),
            cost: None,
            duration_ms: Some(0),
            time_to_first_token_ms: None,
            finish_reason: Some(FinishReason::Error),
            error: Some(e.clone()),
            stream: req.stream,
            attempts: vec![],
            routing: None,
            request_content: None,
            response_content: None,
        };
        let _ = self.store.insert_execution(&rec);
        self.emit(HarnessEvent::ExecutionFinished { execution: ExecutionSummary::from(&rec) });
    }

    fn model_info(&self, key: &str) -> Option<ModelInfo> {
        self.list_models().into_iter().find(|m| m.key == key)
    }

    async fn run(
        self: Arc<Self>,
        id: String,
        req: ExecRequest,
        client: String,
        decision: RoutingDecision,
        tx: mpsc::Sender<ExecEvent>,
        cancel: CancellationToken,
    ) {
        let settings = self.settings();
        let persist = settings.analytics.local_history;
        let retain = settings.security.retain_request_content;
        let started = Instant::now();
        let first = decision.candidates[0].clone();
        let mut record = ExecutionRecord {
            id: id.clone(),
            created_at: now(),
            completed_at: None,
            client: client.clone(),
            status: ExecutionStatus::Running,
            task: decision.classification.task,
            complexity: decision.classification.complexity,
            preset: decision.preset,
            model: Some(first.model.clone()),
            usage: TokenUsage::default(),
            cost: None,
            duration_ms: None,
            time_to_first_token_ms: None,
            finish_reason: None,
            error: None,
            stream: req.stream,
            attempts: vec![],
            routing: Some(decision.clone()),
            request_content: if retain { serde_json::to_value(&req).ok() } else { None },
            response_content: None,
        };
        if persist {
            if let Err(e) = self.store.insert_execution(&record) {
                tracing::warn!(error = %e, "could not record execution");
            }
        }
        self.active.write().insert(id.clone(), ActiveExecution { cancel: cancel.clone(), summary: ExecutionSummary::from(&record) });
        self.emit(HarnessEvent::ExecutionStarted { execution: ExecutionSummary::from(&record) });
        let _ = tx
            .send(ExecEvent::Started {
                execution_id: id.clone(),
                model: first.model.clone(),
                task: decision.classification.task,
                reasons: first.reasons.clone(),
            })
            .await;

        let non_idempotent = req.agent.as_ref().map(|a| a.allow_writes).unwrap_or(false);
        let mut output = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut usage = TokenUsage::default();
        let mut ttft: Option<u64> = None;
        let mut emitted_output = false;
        let mut idx = 0usize;
        let mut retried_same = false;
        let mut final_error: Option<HarnessError> = None;
        let mut outcome: Option<(ProviderOutcome, Candidate)> = None;

        while idx < decision.candidates.len() && record.attempts.len() < MAX_ATTEMPTS {
            let cand = decision.candidates[idx].clone();
            let attempt_started = now();
            let attempt_clock = Instant::now();
            record.model = Some(cand.model.clone());
            self.mark_activity(&cand.model.account_id);

            let result = match self.adapter_for(&cand.model.account_id) {
                None => Err(HarnessError::new(ErrorKind::LocalDependency, "Provider adapter unavailable")),
                Some(adapter) => {
                    let permit = tokio::select! {
                        p = self.semaphore.clone().acquire_owned() => p.ok(),
                        _ = cancel.cancelled() => None,
                    };
                    if permit.is_none() {
                        Err(HarnessError::cancelled())
                    } else {
                        let info = self.model_info(&cand.model.key);
                        let default_max =
                            info.as_ref().and_then(|m| m.model.max_output_tokens).map(|m| m.min(32_000) as u32).unwrap_or(16_000);
                        let areq = AdapterRequest {
                            model_id: cand.model.model_id.clone(),
                            request: req.clone(),
                            default_max_output: default_max,
                            timeout: Duration::from_secs(settings.server.request_timeout_secs.max(10)),
                            scratch_dir: self.paths.root.join("scratch"),
                        };
                        let (ptx, mut prx) = mpsc::channel::<ProviderEvent>(256);
                        let attempt_cancel = cancel.child_token();
                        let fut = adapter.execute(&areq, ptx, attempt_cancel.clone());
                        tokio::pin!(fut);
                        let mut channel_open = true;
                        let res = loop {
                            tokio::select! {
                                biased;
                                ev = prx.recv(), if channel_open => match ev {
                                    Some(ev) => {
                                        if !self.forward(ev, &cand.model.account_id, &tx, &mut output, &mut tool_calls, &mut usage, &mut ttft, &mut emitted_output, started).await {
                                            // Client went away.
                                            attempt_cancel.cancel();
                                            cancel.cancel();
                                        }
                                    }
                                    None => channel_open = false,
                                },
                                r = &mut fut => break r,
                            }
                        };
                        while let Ok(ev) = prx.try_recv() {
                            self.forward(
                                ev,
                                &cand.model.account_id,
                                &tx,
                                &mut output,
                                &mut tool_calls,
                                &mut usage,
                                &mut ttft,
                                &mut emitted_output,
                                started,
                            )
                            .await;
                        }
                        drop(permit);
                        res
                    }
                }
            };

            let duration_ms = attempt_clock.elapsed().as_millis() as u64;
            match result {
                Ok(o) => {
                    record.attempts.push(AttemptRecord {
                        model: cand.model.clone(),
                        started_at: attempt_started,
                        duration_ms,
                        error: None,
                    });
                    outcome = Some((o, cand));
                    break;
                }
                Err(e) => {
                    record.attempts.push(AttemptRecord {
                        model: cand.model.clone(),
                        started_at: attempt_started,
                        duration_ms,
                        error: Some(e.clone()),
                    });
                    self.handle_provider_error(&cand.model, &e);
                    if e.kind == ErrorKind::Cancelled || cancel.is_cancelled() {
                        final_error = Some(HarnessError::cancelled());
                        break;
                    }
                    if emitted_output || non_idempotent {
                        // Partial output already reached the client, or the
                        // agent may have changed files: never replay.
                        final_error = Some(e);
                        break;
                    }
                    if e.kind.is_transient() && !retried_same {
                        retried_same = true;
                        let wait = e.retry_after_secs.unwrap_or(1).clamp(1, 10);
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
                            _ = cancel.cancelled() => { final_error = Some(HarnessError::cancelled()); break; }
                        }
                        continue;
                    }
                    // The initial route is a snapshot. An expired credential
                    // or account-wide quota can invalidate many of its models.
                    // Re-evaluate eligibility while retaining the original
                    // billing boundary and user-defined fallback order.
                    let next_idx = if decision.allow_fallback && e.kind.is_failover_candidate() {
                        let eligible = self.route_preview(&req).ok();
                        (idx + 1..decision.candidates.len()).find(|i| {
                            eligible.as_ref().is_some_and(|r| r.candidates.iter().any(|c| c.model.key == decision.candidates[*i].model.key))
                        })
                    } else {
                        None
                    };
                    if let Some(next_idx) = next_idx {
                        let next = decision.candidates[next_idx].model.clone();
                        let reason = format!("{} ({})", e.message.chars().take(160).collect::<String>(), e.kind.as_str());
                        let _ =
                            tx.send(ExecEvent::RoutingChanged { from: cand.model.clone(), to: next.clone(), reason: reason.clone() }).await;
                        self.notify(
                            NotificationKind::FallbackActivated,
                            format!("Fallback to {}", next.display_name),
                            format!("{} failed: {reason}", cand.model.display_name),
                            Some(cand.model.account_id.clone()),
                        );
                        idx = next_idx;
                        retried_same = false;
                        continue;
                    }
                    final_error = Some(e);
                    break;
                }
            }
        }

        let duration_ms = started.elapsed().as_millis() as u64;
        record.duration_ms = Some(duration_ms);
        record.time_to_first_token_ms = ttft;
        record.completed_at = Some(now());

        match outcome {
            Some((o, cand)) => {
                usage.merge(&o.usage);
                if usage.input_tokens.is_none() && usage.output_tokens.is_none() {
                    let out_text: String = output.clone() + &tool_calls.iter().map(|c| c.arguments.as_str()).collect::<String>();
                    usage = TokenUsage {
                        input_tokens: Some(decision.classification.estimated_input_tokens),
                        output_tokens: Some(estimate_tokens(&out_text)),
                        provenance: Provenance::Estimated,
                        ..Default::default()
                    };
                }
                let info = self.model_info(&cand.model.key);
                let cost = compute_cost(info.as_ref(), &usage, o.cost);
                if !o.limits.is_empty() {
                    self.apply_limits(&cand.model.account_id, o.limits.clone());
                }
                record.status = ExecutionStatus::Succeeded;
                record.finish_reason = Some(if tool_calls.is_empty() { o.finish_reason } else { FinishReason::ToolCalls });
                record.usage = usage.clone();
                record.cost = cost;
                if retain {
                    record.response_content = Some(output.clone());
                }
                let result = ExecResult {
                    execution_id: id.clone(),
                    model: cand.model.clone(),
                    output_text: output,
                    tool_calls,
                    finish_reason: record.finish_reason.unwrap_or(FinishReason::Stop),
                    usage,
                    cost,
                    duration_ms,
                    time_to_first_token_ms: ttft,
                    attempts: record.attempts.len() as u32,
                    created_at: record.created_at,
                };
                let _ = tx.send(ExecEvent::Completed { result }).await;
                if self.adapter_for(&cand.model.account_id).map(|a| a.supports_limit_polling()).unwrap_or(false) {
                    let _ = self.poke_limits.send(cand.model.account_id.clone());
                }
                self.check_spend_threshold();
            }
            None => {
                let err = final_error.unwrap_or_else(|| HarnessError::new(ErrorKind::NoEligibleModel, "All candidate models failed"));
                record.status = if err.kind == ErrorKind::Cancelled { ExecutionStatus::Cancelled } else { ExecutionStatus::Failed };
                record.finish_reason = Some(if err.kind == ErrorKind::Cancelled { FinishReason::Cancelled } else { FinishReason::Error });
                if !usage.is_empty() {
                    record.usage = usage;
                }
                record.error = Some(err.clone());
                if retain && !output.is_empty() {
                    record.response_content = Some(output);
                }
                let _ = tx.send(ExecEvent::Failed { execution_id: id.clone(), error: err }).await;
            }
        }

        if persist {
            if let Err(e) = self.store.update_execution(&record) {
                tracing::warn!(error = %e, "could not update execution record");
            }
        }
        self.active.write().remove(&id);
        self.emit(HarnessEvent::ExecutionFinished { execution: ExecutionSummary::from(&record) });
    }

    /// Forward a provider event to the client. Returns false when the
    /// client has disconnected.
    #[allow(clippy::too_many_arguments)]
    async fn forward(
        &self,
        ev: ProviderEvent,
        account_id: &str,
        tx: &mpsc::Sender<ExecEvent>,
        output: &mut String,
        tool_calls: &mut Vec<ToolCall>,
        usage: &mut TokenUsage,
        ttft: &mut Option<u64>,
        emitted: &mut bool,
        started: Instant,
    ) -> bool {
        let out = match ev {
            ProviderEvent::TextDelta(t) => {
                if t.is_empty() {
                    return true;
                }
                output.push_str(&t);
                Some(ExecEvent::TextDelta { text: t })
            }
            ProviderEvent::ReasoningDelta(t) => Some(ExecEvent::ReasoningDelta { text: t }),
            ProviderEvent::ToolCall(c) => {
                tool_calls.push(c.clone());
                Some(ExecEvent::ToolCall { call: c })
            }
            ProviderEvent::Usage(u) => {
                usage.merge(&u);
                None
            }
            ProviderEvent::Limits(mut windows) => {
                for w in &mut windows {
                    w.account_id = account_id.to_string();
                }
                self.apply_limits(account_id, windows);
                None
            }
            ProviderEvent::ResolvedModel(_) => None,
        };
        match out {
            Some(e) => {
                if ttft.is_none() {
                    *ttft = Some(started.elapsed().as_millis() as u64);
                }
                *emitted = true;
                tx.send(e).await.is_ok()
            }
            None => true,
        }
    }

    fn handle_provider_error(&self, model: &SelectedModel, e: &HarnessError) {
        match e.kind {
            ErrorKind::RateLimited | ErrorKind::QuotaExhausted => self.observe_limit_error(model, e),
            ErrorKind::Authentication => {
                if let Some(mut account) = self.get_account(&model.account_id) {
                    let was = account.status == ConnectionStatus::Connected;
                    let _ = self.set_failure(&mut account, e, was);
                }
            }
            ErrorKind::ModelNotFound | ErrorKind::PermissionDenied => self.block_model(&model.key, e.message.clone()),
            ErrorKind::LocalDependency => {
                if let Some(mut account) = self.get_account(&model.account_id) {
                    if account.kind.descriptor().auth_method == AuthMethod::CliDelegated || account.kind.descriptor().is_local {
                        let was = account.status == ConnectionStatus::Connected;
                        let _ = self.set_failure(&mut account, e, was);
                    }
                }
            }
            _ => {}
        }
    }

    fn check_spend_threshold(&self) {
        let Some(threshold) = self.settings.read().analytics.spend_alert_usd else { return };
        let today = now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        if let Ok(spend) = self.store.spend_since(today.timestamp_millis()) {
            if spend >= threshold {
                self.notify_once(
                    format!("spend:{}", today.date_naive()),
                    NotificationKind::SpendThreshold,
                    format!("Daily API spend reached ${spend:.2}"),
                    format!("Your alert threshold is ${threshold:.2}. Figures include estimates."),
                    None,
                );
            }
        }
    }
}

/// Cost of an execution with honest provenance.
pub(crate) fn compute_cost(model: Option<&ModelInfo>, usage: &TokenUsage, reported: Option<Cost>) -> Option<Cost> {
    if let Some(c) = reported {
        return Some(c);
    }
    let model = model?;
    if model.billing_mode == BillingMode::Local {
        return None;
    }
    let pricing = model.pricing()?;
    let input = usage.input_tokens?;
    let usd = pricing.cost(input, usage.cached_input_tokens.unwrap_or(0), usage.output_tokens.unwrap_or(0));
    let provenance =
        if usage.provenance == Provenance::Reported && matches!(pricing.provenance, Provenance::Reported | Provenance::Calculated) {
            Provenance::Calculated
        } else {
            Provenance::Estimated
        };
    Some(Cost { usd, provenance, api_equivalent: model.billing_mode == BillingMode::Subscription })
}
