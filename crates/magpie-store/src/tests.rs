use super::*;

fn account(id: &str) -> Account {
    Account {
        id: id.into(),
        kind: ProviderKind::OpenAi,
        label: "Work".into(),
        auth_method: AuthMethod::ApiKey,
        billing_mode: BillingMode::Metered,
        billing_reported: false,
        base_url: None,
        identity: None,
        plan: None,
        status: ConnectionStatus::Connected,
        status_message: None,
        enabled: true,
        last_verified_at: Some(Utc::now()),
        created_at: Utc::now(),
        has_secret: true,
        secret_store: Some("memory".into()),
        options: serde_json::json!({}),
    }
}

fn exec(id: &str, status: ExecutionStatus, input: u64, output: u64, cost: Option<Cost>, at: Timestamp) -> ExecutionRecord {
    ExecutionRecord {
        id: id.into(),
        created_at: at,
        completed_at: Some(at),
        client: "test".into(),
        status,
        task: TaskClass::CodeGeneration,
        complexity: Complexity::Medium,
        preset: RoutingPreset::Automatic,
        model: Some(SelectedModel {
            key: "acc_1/gpt".into(),
            account_id: "acc_1".into(),
            provider: ProviderKind::OpenAi,
            model_id: "gpt".into(),
            display_name: "GPT".into(),
        }),
        usage: TokenUsage {
            input_tokens: Some(input),
            output_tokens: Some(output),
            provenance: Provenance::Reported,
            ..Default::default()
        },
        cost,
        duration_ms: Some(1000),
        time_to_first_token_ms: Some(200),
        finish_reason: Some(FinishReason::Stop),
        error: None,
        stream: false,
        attempts: vec![],
        routing: None,
        request_content: None,
        response_content: None,
    }
}

#[test]
fn migrations_apply_once() {
    let s = Store::open_in_memory().unwrap();
    assert_eq!(s.applied_migrations().unwrap(), vec!["0001_initial".to_string()]);
    s.migrate().unwrap();
    assert_eq!(s.applied_migrations().unwrap().len(), 1);
}

#[test]
fn migrations_survive_reopen() {
    let dir = std::env::temp_dir().join(format!("magpie-store-{}", std::process::id()));
    let path = dir.join("m.db");
    {
        let s = Store::open(&path).unwrap();
        s.upsert_account(&account("acc_1")).unwrap();
    }
    let s = Store::open(&path).unwrap();
    assert_eq!(s.list_accounts().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn account_roundtrip_and_delete_cascades() {
    let s = Store::open_in_memory().unwrap();
    let a = account("acc_1");
    s.upsert_account(&a).unwrap();
    let got = s.get_account("acc_1").unwrap().unwrap();
    assert_eq!(got.kind, ProviderKind::OpenAi);
    assert_eq!(got.status, ConnectionStatus::Connected);
    assert!(got.has_secret);

    let m = DiscoveredModel {
        model_id: "gpt".into(),
        display_name: "GPT".into(),
        description: None,
        context_window: Some(128_000),
        max_output_tokens: None,
        capabilities: Capabilities::default(),
        tier: QualityTier::High,
        speed: SpeedClass::Medium,
        pricing: None,
        metadata_provenance: Provenance::Reported,
        is_default: false,
        reasoning_efforts: vec![],
    };
    s.replace_models("acc_1", &[m]).unwrap();
    s.set_model_pref("acc_1/gpt", &ModelPreference { favourite: true, ..Default::default() }).unwrap();
    assert_eq!(s.list_models().unwrap().len(), 1);
    assert!(s.model_prefs().unwrap()["acc_1/gpt"].favourite);

    s.delete_account("acc_1").unwrap();
    assert!(s.get_account("acc_1").unwrap().is_none());
    assert!(s.list_models().unwrap().is_empty());
    assert!(s.model_prefs().unwrap().is_empty());
}

#[test]
fn settings_default_and_persist() {
    let s = Store::open_in_memory().unwrap();
    let mut st = s.settings().unwrap();
    assert_eq!(st.server.port, 7878);
    st.general.launch_on_startup = true;
    s.save_settings(&st).unwrap();
    assert!(s.settings().unwrap().general.launch_on_startup);

    let mut r = s.routing_config().unwrap();
    r.preset = RoutingPreset::Fastest;
    s.save_routing_config(&r).unwrap();
    assert_eq!(s.routing_config().unwrap().preset, RoutingPreset::Fastest);
}

#[test]
fn usage_aggregation_respects_provenance() {
    let s = Store::open_in_memory().unwrap();
    let now = Utc::now();
    let reported = Some(Cost { usd: 0.5, provenance: Provenance::Reported, api_equivalent: false });
    let estimated = Some(Cost { usd: 0.25, provenance: Provenance::Estimated, api_equivalent: false });
    let sub = Some(Cost { usd: 2.0, provenance: Provenance::Reported, api_equivalent: true });
    s.insert_execution(&exec("e1", ExecutionStatus::Succeeded, 100, 50, reported, now)).unwrap();
    s.insert_execution(&exec("e2", ExecutionStatus::Succeeded, 200, 10, estimated, now)).unwrap();
    s.insert_execution(&exec("e3", ExecutionStatus::Failed, 0, 0, None, now)).unwrap();
    s.insert_execution(&exec("e4", ExecutionStatus::Succeeded, 10, 10, sub, now)).unwrap();

    let from = ms(now) - 1000;
    let to = ms(now) + 1000;
    let sum = s.usage_summary(from, to, &UsageFilter::default()).unwrap();
    assert_eq!(sum.requests, 4);
    assert_eq!(sum.succeeded, 3);
    assert_eq!(sum.failed, 1);
    assert_eq!(sum.input_tokens, 310);
    assert_eq!(sum.output_tokens, 70);
    assert!((sum.cost_reported_usd - 0.5).abs() < 1e-9);
    assert!((sum.cost_estimated_usd - 0.25).abs() < 1e-9);
    assert!((sum.api_equivalent_usd - 2.0).abs() < 1e-9);
    assert_eq!(sum.p95_duration_ms, Some(1000.0));

    let ts = s.usage_timeseries(from, to, 60_000, GroupBy::None, &UsageFilter::default()).unwrap();
    assert_eq!(ts.len(), 1);
    assert_eq!(ts[0].requests, 4);

    let by_model = s.usage_breakdown(from, to, GroupBy::Model, &UsageFilter::default()).unwrap();
    assert_eq!(by_model[0].label, "GPT");

    let stats = s.model_stats(from).unwrap();
    assert_eq!(stats[0].requests, 4);
    assert_eq!(stats[0].failures, 1);

    let filtered = s
        .usage_summary(from, to, &UsageFilter { provider: Some("anthropic".into()), ..Default::default() })
        .unwrap();
    assert_eq!(filtered.requests, 0);
}

#[test]
fn execution_detail_roundtrip_and_recovery() {
    let s = Store::open_in_memory().unwrap();
    let now = Utc::now();
    let mut e = exec("e1", ExecutionStatus::Running, 0, 0, None, now);
    e.usage = TokenUsage::default();
    s.insert_execution(&e).unwrap();
    assert_eq!(s.recover_interrupted().unwrap(), 1);
    let got = s.get_execution("e1").unwrap().unwrap();
    assert_eq!(got.status, ExecutionStatus::Failed);
    assert!(got.error.is_some());

    let list = s.list_executions(&ExecutionQuery { limit: 10, ..Default::default() }).unwrap();
    assert_eq!(list.len(), 1);
    let none = s
        .list_executions(&ExecutionQuery { limit: 10, status: Some("succeeded".into()), ..Default::default() })
        .unwrap();
    assert!(none.is_empty());
}

#[test]
fn api_clients_hash_lookup_and_revoke() {
    let s = Store::open_in_memory().unwrap();
    let c = ApiClient {
        id: "cli_1".into(),
        name: "VS Code".into(),
        prefix: "mgp_abc".into(),
        scopes: vec!["execute".into(), "read".into()],
        created_at: Utc::now(),
        last_used_at: None,
        revoked_at: None,
    };
    s.insert_client(&c, "hash1").unwrap();
    assert_eq!(s.client_by_hash("hash1").unwrap().unwrap().scopes.len(), 2);
    assert!(s.revoke_client("cli_1").unwrap());
    assert!(s.client_by_hash("hash1").unwrap().is_none());
}
