use super::*;

fn caps(tools: bool, vision: bool) -> Capabilities {
    Capabilities { streaming: true, tools, vision, structured_output: tools, reasoning: true, agentic: false, system_prompt: true }
}

fn model(account: &str, id: &str, tier: QualityTier, speed: SpeedClass, billing: BillingMode, price: Option<(f64, f64)>, c: Capabilities) -> ModelInfo {
    ModelInfo {
        key: format!("{account}/{id}"),
        account_id: account.into(),
        account_label: account.into(),
        provider: match billing {
            BillingMode::Subscription => ProviderKind::ClaudeCode,
            BillingMode::Local => ProviderKind::Ollama,
            _ => ProviderKind::OpenAi,
        },
        billing_mode: billing,
        model: DiscoveredModel {
            model_id: id.into(),
            display_name: id.into(),
            description: None,
            context_window: Some(200_000),
            max_output_tokens: None,
            capabilities: c,
            tier,
            speed,
            pricing: price.map(|(i, o)| Pricing { input_per_mtok: i, output_per_mtok: o, cached_input_per_mtok: None, provenance: Provenance::Estimated }),
            metadata_provenance: Provenance::Reported,
            is_default: false,
            reasoning_efforts: vec![],
        },
        preference: ModelPreference::default(),
        available: true,
        unavailable_reason: None,
        discovered_at: now(),
    }
}

fn inventory() -> Vec<ModelInfo> {
    vec![
        model("sub", "frontier", QualityTier::Frontier, SpeedClass::Slow, BillingMode::Subscription, None, caps(false, false)),
        model("api", "big", QualityTier::High, SpeedClass::Medium, BillingMode::Metered, Some((3.0, 15.0)), caps(true, true)),
        model("api", "small", QualityTier::Light, SpeedClass::Fast, BillingMode::Metered, Some((0.1, 0.4)), caps(true, false)),
        model("local", "llama", QualityTier::Standard, SpeedClass::Medium, BillingMode::Local, None, caps(false, false)),
    ]
}

fn run(req: &ExecRequest, models: &[ModelInfo], cfg: &RoutingConfig, limits: &[LimitWindow]) -> HarnessResult<RoutingDecision> {
    let history = HashMap::new();
    route(&RouteInput { request: req, models, config: cfg, limits, history: &history, now: now() })
}

#[test]
fn best_quality_prefers_frontier() {
    let mut req = ExecRequest::simple("Design a detailed architecture plan for a distributed queue with exactly-once semantics");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    let cfg = RoutingConfig { allow_subscription_to_api: true, ..Default::default() };
    let d = run(&req, &inventory(), &cfg, &[]).unwrap();
    assert_eq!(d.selected().unwrap().model.model_id, "frontier");
    assert!(!d.selected().unwrap().reasons.is_empty());
}

#[test]
fn fastest_prefers_fast_models() {
    let mut req = ExecRequest::simple("hi");
    req.preferences.preset = Some(RoutingPreset::Fastest);
    let d = run(&req, &inventory(), &RoutingConfig { allow_subscription_to_api: true, ..Default::default() }, &[]).unwrap();
    assert_eq!(d.selected().unwrap().model.model_id, "small");
}

#[test]
fn economical_prefers_free_execution() {
    let mut req = ExecRequest::simple("What's 2+2?");
    req.preferences.preset = Some(RoutingPreset::Economical);
    let d = run(&req, &inventory(), &RoutingConfig::default(), &[]).unwrap();
    assert_eq!(d.selected().unwrap().model.model_id, "llama");
}

#[test]
fn capability_negotiation_rejects_models_without_tools() {
    let mut req = ExecRequest::simple("call the tool");
    req.tools.push(ToolDefinition { name: "t".into(), description: None, parameters: serde_json::json!({}) });
    let d = run(&req, &inventory(), &RoutingConfig::default(), &[]).unwrap();
    assert!(d.candidates.iter().all(|c| c.model.account_id == "api"));
    assert!(d.rejected.iter().any(|r| r.model_key == "sub/frontier" && r.reason.contains("tool calling")));
}

#[test]
fn exhausted_limits_remove_model() {
    let mut w = LimitWindow::new("sub", "weekly", "Weekly", LimitMetric::UsagePercent, Provenance::Reported);
    w.used_percent = Some(100.0);
    w.resets_at = Some(now() + chrono::Duration::hours(3));
    let mut req = ExecRequest::simple("Plan a complex thorough migration strategy");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    let d = run(&req, &inventory(), &RoutingConfig::default(), &[w]).unwrap();
    assert_ne!(d.selected().unwrap().model.account_id, "sub");
    assert!(d.rejected.iter().any(|r| r.model_key == "sub/frontier" && r.reason.contains("exhausted")));
}

#[test]
fn no_subscription_to_api_fallback_without_opt_in() {
    let mut req = ExecRequest::simple("Plan a complex thorough migration strategy");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    let d = run(&req, &inventory(), &RoutingConfig::default(), &[]).unwrap();
    assert_eq!(d.selected().unwrap().model.model_id, "frontier");
    assert!(d.candidates.iter().all(|c| !c.billable));
    assert!(d.rejected.iter().any(|r| r.reason.contains("subscription to billable")));

    let d = run(&req, &inventory(), &RoutingConfig { allow_subscription_to_api: true, ..Default::default() }, &[]).unwrap();
    assert!(d.candidates.iter().any(|c| c.billable));
}

#[test]
fn reservation_applies_only_with_known_allowance() {
    let cfg = RoutingConfig { preserve_premium_percent: Some(20.0), allow_subscription_to_api: true, ..Default::default() };
    let mut req = ExecRequest::simple("What is a mutex?");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    // Unknown allowance: no reservation.
    let d = run(&req, &inventory(), &cfg, &[]).unwrap();
    assert_eq!(d.selected().unwrap().model.model_id, "frontier");
    // Known 10% remaining: reserved for complex work.
    let mut w = LimitWindow::new("sub", "5h", "5-hour", LimitMetric::UsagePercent, Provenance::Reported);
    w.used_percent = Some(90.0);
    let d = run(&req, &inventory(), &cfg, &[w.clone()]).unwrap();
    assert_ne!(d.selected().unwrap().model.model_id, "frontier");
    // Complex work may still use it.
    let mut req = ExecRequest::simple("Thoroughly prove this complex theorem about graph colouring");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    let d = run(&req, &inventory(), &cfg, &[w]).unwrap();
    assert_eq!(d.selected().unwrap().model.model_id, "frontier");
}

#[test]
fn explicit_model_is_pinned_and_unknown_fails() {
    let mut req = ExecRequest::simple("hi");
    req.model = "api/big".into();
    let d = run(&req, &inventory(), &RoutingConfig::default(), &[]).unwrap();
    assert_eq!(d.selected().unwrap().model.key, "api/big");
    req.model = "openai/small".into();
    assert_eq!(run(&req, &inventory(), &RoutingConfig::default(), &[]).unwrap().selected().unwrap().model.model_id, "small");
    req.model = "nonexistent".into();
    assert_eq!(run(&req, &inventory(), &RoutingConfig::default(), &[]).unwrap_err().kind, ErrorKind::ModelNotFound);
}

#[test]
fn manual_without_fallback_has_single_candidate() {
    let cfg = RoutingConfig { preset: RoutingPreset::Manual, manual_model: Some("api/small".into()), allow_fallback: false, ..Default::default() };
    let d = run(&ExecRequest::simple("x"), &inventory(), &cfg, &[]).unwrap();
    assert_eq!(d.candidates.len(), 1);
    assert_eq!(d.selected().unwrap().model.key, "api/small");
}

#[test]
fn task_rules_and_disabled_models() {
    let mut models = inventory();
    models[3].preference.disabled = true;
    let cfg = RoutingConfig {
        task_rules: vec![TaskRule { task: TaskClass::SimpleQuestion, models: vec!["api/big".into()], preset: None }],
        ..Default::default()
    };
    let d = run(&ExecRequest::simple("What time zone is Tokyo in?"), &models, &cfg, &[]).unwrap();
    assert_eq!(d.selected().unwrap().model.key, "api/big");
    assert!(d.rejected.iter().any(|r| r.model_key == "local/llama" && r.reason.contains("Disabled")));
}

#[test]
fn cost_ceiling_rejects_expensive_models() {
    let mut req = ExecRequest::simple(&"word ".repeat(40_000));
    req.preferences.max_cost_usd = Some(0.001);
    req.preferences.providers = vec![ProviderKind::OpenAi];
    let err = run(&req, &inventory(), &RoutingConfig::default(), &[]).unwrap_err();
    assert_eq!(err.kind, ErrorKind::NoEligibleModel);
    assert!(err.message.contains("exceeds limit"));
}

#[test]
fn no_models_gives_helpful_error() {
    let err = run(&ExecRequest::simple("x"), &[], &RoutingConfig::default(), &[]).unwrap_err();
    assert!(err.message.contains("Connect a provider"));
}

#[test]
fn history_penalises_unreliable_models() {
    let models = vec![
        model("a", "m1", QualityTier::High, SpeedClass::Medium, BillingMode::Local, None, caps(false, false)),
        model("b", "m2", QualityTier::High, SpeedClass::Medium, BillingMode::Local, None, caps(false, false)),
    ];
    let mut history = HashMap::new();
    history.insert("a/m1".to_string(), ModelHistory { requests: 50, failures: 30, avg_duration_ms: None, output_tps: None });
    let req = ExecRequest::simple("Explain how TCP slow start works in some detail please, thanks");
    let d = route(&RouteInput { request: &req, models: &models, config: &RoutingConfig::default(), limits: &[], history: &history, now: now() }).unwrap();
    assert_eq!(d.selected().unwrap().model.key, "b/m2");
}
