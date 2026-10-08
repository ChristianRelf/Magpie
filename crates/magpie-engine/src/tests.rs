use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use magpie_providers::testing::{model, MockAdapter, Script};
use magpie_security::{MemorySecretStore, SecretBackend};
use magpie_store::paths::Paths;

use crate::*;

struct Fixture {
    h: Arc<Harness>,
    mocks: HashMap<ProviderKind, Arc<MockAdapter>>,
    _dir: TempDir,
}

struct TempDir(std::path::PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn caps(tools: bool) -> Capabilities {
    Capabilities { streaming: true, tools, structured_output: tools, vision: false, reasoning: true, agentic: false, system_prompt: true }
}

fn dummy_account(kind: ProviderKind) -> Account {
    Account {
        id: "mock".into(),
        kind,
        label: "mock".into(),
        auth_method: AuthMethod::None,
        billing_mode: BillingMode::Unknown,
        billing_reported: false,
        base_url: None,
        identity: None,
        plan: None,
        status: ConnectionStatus::Connected,
        status_message: None,
        enabled: true,
        last_verified_at: None,
        created_at: now(),
        has_secret: false,
        secret_store: None,
        options: serde_json::json!({}),
    }
}

async fn fixture() -> Fixture {
    let dir = std::env::temp_dir().join(format!("magpie-engine-test-{}", uuid::Uuid::new_v4().simple()));
    let mut mocks = HashMap::new();
    mocks.insert(
        ProviderKind::CodexCli,
        Arc::new(MockAdapter::new(
            dummy_account(ProviderKind::CodexCli),
            vec![model("codex", QualityTier::High, SpeedClass::Medium, caps(false), None)],
        )),
    );
    mocks.insert(
        ProviderKind::ClaudeCode,
        Arc::new(MockAdapter::new(
            dummy_account(ProviderKind::ClaudeCode),
            vec![model("sub-top", QualityTier::Frontier, SpeedClass::Medium, caps(false), None)],
        )),
    );
    mocks.insert(
        ProviderKind::OpenAi,
        Arc::new(MockAdapter::new(
            dummy_account(ProviderKind::OpenAi),
            vec![model(
                "api-mid",
                QualityTier::High,
                SpeedClass::Medium,
                caps(true),
                Some(Pricing { input_per_mtok: 1.0, output_per_mtok: 2.0, cached_input_per_mtok: None, provenance: Provenance::Reported }),
            )],
        )),
    );
    mocks.insert(
        ProviderKind::Ollama,
        Arc::new(MockAdapter::new(
            dummy_account(ProviderKind::Ollama),
            vec![model("local-small", QualityTier::Light, SpeedClass::Fast, caps(false), None)],
        )),
    );
    let m2 = mocks.clone();
    let factory: AdapterFactory = Arc::new(move |account: Account, _secret| {
        let a: magpie_providers::SharedAdapter = m2.get(&account.kind).cloned().expect("mock for kind");
        Ok(a)
    });
    let h = Harness::open(HarnessOptions {
        paths: Paths::at(dir.clone()),
        secrets: Arc::new(MemorySecretStore::default()),
        secret_backend: SecretBackend::Memory,
        adapter_factory: Some(factory),
        launch_command: None,
        background: false,
    })
    .await
    .unwrap();
    Fixture { h, mocks, _dir: TempDir(dir) }
}

async fn connect(f: &Fixture, kind: ProviderKind) -> Account {
    f.h.connect(ConnectRequest {
        auth_mode: CliAuthMode::Existing,
        oauth_token: None,
        kind,
        label: None,
        api_key: if kind == ProviderKind::OpenAi { Some("sk-test-1234567890abcdef".into()) } else { None },
        base_url: None,
        options: None,
        billing_mode: None,
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn connect_discovers_models_and_executes_end_to_end() {
    let f = fixture().await;
    let acc = connect(&f, ProviderKind::OpenAi).await;
    assert_eq!(acc.status, ConnectionStatus::Connected);
    assert!(acc.has_secret);
    assert_eq!(acc.identity.as_deref(), Some("test@example.com"));
    let models = f.h.list_models();
    assert_eq!(models.len(), 1);
    assert!(models[0].available);

    f.mocks[&ProviderKind::OpenAi].script(
        "api-mid",
        vec![Script::Reply {
            chunks: vec!["Hel".into(), "lo".into()],
            usage: TokenUsage {
                input_tokens: Some(1000),
                output_tokens: Some(500),
                provenance: Provenance::Reported,
                ..Default::default()
            },
            tool_calls: vec![],
        }],
    );
    let (result, _) = f.h.execute_collect(ExecRequest::simple("Say hello"), "test".into()).await.unwrap();
    assert_eq!(result.output_text, "Hello");
    assert_eq!(result.usage.output_tokens, Some(500));
    let cost = result.cost.unwrap();
    assert!((cost.usd - 0.002).abs() < 1e-9, "{}", cost.usd);
    assert_eq!(cost.provenance, Provenance::Calculated);

    let rec = f.h.store.get_execution(&result.execution_id).unwrap().unwrap();
    assert_eq!(rec.status, ExecutionStatus::Succeeded);
    assert!(rec.routing.is_some());
    assert!(rec.request_content.is_none(), "content must not be stored by default");
    assert!(rec.response_content.is_none());
    let sum = f.h.store.usage_summary(0, i64::MAX, &Default::default()).unwrap();
    assert_eq!(sum.requests, 1);
    assert_eq!(sum.input_tokens, 1000);
}

#[tokio::test]
async fn streaming_events_arrive_in_order() {
    let f = fixture().await;
    connect(&f, ProviderKind::Ollama).await;
    f.mocks[&ProviderKind::Ollama].script(
        "local-small",
        vec![Script::Reply { chunks: vec!["a".into(), "b".into(), "c".into()], usage: TokenUsage::default(), tool_calls: vec![] }],
    );
    let mut req = ExecRequest::simple("hi");
    req.stream = true;
    let mut handle = f.h.execute(req, "test".into()).await.unwrap();
    let mut kinds = Vec::new();
    while let Some(ev) = handle.events.recv().await {
        kinds.push(match ev {
            ExecEvent::Started { .. } => "started".to_string(),
            ExecEvent::TextDelta { text } => text,
            ExecEvent::Completed { result } => {
                // No provider usage: estimated locally and labelled as such.
                assert_eq!(result.usage.provenance, Provenance::Estimated);
                assert!(result.cost.is_none(), "local models have no cost");
                "done".into()
            }
            other => panic!("unexpected {other:?}"),
        });
    }
    assert_eq!(kinds, vec!["started", "a", "b", "c", "done"]);
}

#[tokio::test]
async fn falls_back_on_rate_limit_and_records_limit() {
    let f = fixture().await;
    connect(&f, ProviderKind::ClaudeCode).await;
    connect(&f, ProviderKind::Ollama).await;
    f.mocks[&ProviderKind::ClaudeCode].script("sub-top", vec![Script::fail(ErrorKind::RateLimited)]);
    let mut req = ExecRequest::simple("Plan a thorough, complex database migration strategy with rollback steps");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    let (result, notable) = f.h.execute_collect(req, "test".into()).await.unwrap();
    assert_eq!(result.model.model_id, "local-small");
    assert_eq!(result.attempts, 2);
    assert!(notable.iter().any(|e| matches!(e, ExecEvent::RoutingChanged { .. })));
    let limits = f.h.limit_windows();
    assert!(limits.iter().any(|w| w.exhausted && w.key.starts_with("observed.rate")));
    // The next request avoids the rate-limited model.
    let d = f.h.route_preview(&ExecRequest::simple("Plan a thorough, complex migration")).unwrap();
    assert_eq!(d.selected().unwrap().model.model_id, "local-small");
}

#[tokio::test]
async fn never_replays_after_partial_output() {
    let f = fixture().await;
    connect(&f, ProviderKind::ClaudeCode).await;
    connect(&f, ProviderKind::Ollama).await;
    f.mocks[&ProviderKind::ClaudeCode].script(
        "sub-top",
        vec![Script::FailAfterOutput("partial".into(), HarnessError::new(ErrorKind::ProviderUnavailable, "overloaded"))],
    );
    let mut req = ExecRequest::simple("Plan a thorough, complex migration");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    let err = f.h.execute_collect(req, "test".into()).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::ProviderUnavailable);
    assert_eq!(f.mocks[&ProviderKind::Ollama].calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn transient_errors_retry_same_model_once() {
    let f = fixture().await;
    connect(&f, ProviderKind::Ollama).await;
    f.mocks[&ProviderKind::Ollama].script("local-small", vec![Script::fail(ErrorKind::Network), Script::text("recovered")]);
    let (result, _) = f.h.execute_collect(ExecRequest::simple("hi"), "test".into()).await.unwrap();
    assert_eq!(result.output_text, "recovered");
    assert_eq!(result.attempts, 2);
}

#[tokio::test]
async fn subscription_does_not_fall_back_to_billable_api_without_opt_in() {
    let f = fixture().await;
    connect(&f, ProviderKind::ClaudeCode).await;
    connect(&f, ProviderKind::OpenAi).await;
    f.mocks[&ProviderKind::ClaudeCode].script("sub-top", vec![Script::fail(ErrorKind::QuotaExhausted)]);
    let mut req = ExecRequest::simple("Plan a thorough, complex migration");
    req.preferences.preset = Some(RoutingPreset::BestQuality);
    let err = f.h.execute_collect(req.clone(), "test".into()).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::QuotaExhausted);
    assert_eq!(f.mocks[&ProviderKind::OpenAi].calls.load(Ordering::SeqCst), 0);

    // The exhausted subscription is now avoided; with opt-in the API is used.
    let mut cfg = f.h.routing_config();
    cfg.allow_subscription_to_api = true;
    f.h.update_routing(cfg).unwrap();
    let (result, _) = f.h.execute_collect(req, "test".into()).await.unwrap();
    assert_eq!(result.model.model_id, "api-mid");
}

#[tokio::test]
async fn cancellation_stops_execution() {
    let f = fixture().await;
    connect(&f, ProviderKind::Ollama).await;
    f.mocks[&ProviderKind::Ollama].script("local-small", vec![Script::Delay(Duration::from_secs(30), Box::new(Script::text("late")))]);
    let mut handle = f.h.execute(ExecRequest::simple("hi"), "test".into()).await.unwrap();
    let _started = handle.events.recv().await;
    assert!(f.h.cancel(&handle.id));
    let mut failed = None;
    while let Some(ev) = handle.events.recv().await {
        if let ExecEvent::Failed { error, .. } = ev {
            failed = Some(error);
        }
    }
    assert_eq!(failed.unwrap().kind, ErrorKind::Cancelled);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let rec = f.h.store.get_execution(&handle.id).unwrap().unwrap();
    assert_eq!(rec.status, ExecutionStatus::Cancelled);
    assert!(f.h.active_executions().is_empty());
}

#[tokio::test]
async fn auth_failure_marks_account_and_notifies() {
    let f = fixture().await;
    let acc = connect(&f, ProviderKind::Ollama).await;
    f.mocks[&ProviderKind::Ollama].script("local-small", vec![Script::fail(ErrorKind::Authentication)]);
    let err = f.h.execute_collect(ExecRequest::simple("hi"), "test".into()).await.unwrap_err();
    assert_eq!(err.kind, ErrorKind::Authentication);
    assert_eq!(f.h.get_account(&acc.id).unwrap().status, ConnectionStatus::NeedsAuth);
    assert!(f.h.notifications(10).unwrap().iter().any(|n| n.kind == NotificationKind::AuthExpired));
    // No models available any more: routing explains why.
    let err = f.h.route_preview(&ExecRequest::simple("hi")).unwrap_err();
    assert_eq!(err.kind, ErrorKind::NoEligibleModel);
}

#[tokio::test]
async fn routing_failures_are_recorded() {
    let f = fixture().await;
    let err = f.h.execute(ExecRequest::simple("hi"), "test".into()).await.err().unwrap();
    assert_eq!(err.kind, ErrorKind::NoEligibleModel);
    let list = f.h.store.list_executions(&magpie_store::ExecutionQuery { limit: 10, ..Default::default() }).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].status, ExecutionStatus::Failed);
}

#[tokio::test]
async fn content_retention_is_opt_in_and_purged_on_disable() {
    let f = fixture().await;
    connect(&f, ProviderKind::Ollama).await;
    let mut s = f.h.settings();
    s.security.retain_request_content = true;
    f.h.update_settings(s.clone()).unwrap();
    let (r, _) = f.h.execute_collect(ExecRequest::simple("remember me"), "test".into()).await.unwrap();
    let rec = f.h.store.get_execution(&r.execution_id).unwrap().unwrap();
    assert!(rec.request_content.is_some());
    assert_eq!(rec.response_content.as_deref(), Some("ok"));
    s.security.retain_request_content = false;
    f.h.update_settings(s).unwrap();
    let rec = f.h.store.get_execution(&r.execution_id).unwrap().unwrap();
    assert!(rec.request_content.is_none() && rec.response_content.is_none());
}

#[tokio::test]
async fn api_keys_and_admin_token() {
    let f = fixture().await;
    let token = std::fs::read_to_string(f.h.paths.admin_token()).unwrap();
    assert!(matches!(f.h.authenticate(&token), Some(Principal::Admin)));
    assert!(f.h.authenticate("mga_wrong_token_value_123456").is_none());
    let created = f.h.create_client("VS Code", &["execute".into()]).unwrap();
    let p = f.h.authenticate(&created.token).unwrap();
    assert!(p.has(Scope::Execute));
    assert!(!p.has(Scope::Admin));
    assert!(f.h.create_client("bad", &["root".into()]).is_err());
    f.h.revoke_client(&created.client.id).unwrap();
    assert!(f.h.authenticate(&created.token).is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(f.h.paths.admin_token()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[tokio::test]
async fn settings_validation_and_persistence() {
    let f = fixture().await;
    let mut s = f.h.settings();
    s.server.bind = "0.0.0.0".into();
    assert!(f.h.update_settings(s.clone()).is_err());
    s.server.bind = "127.0.0.1".into();
    s.analytics.retention_days = 30;
    f.h.update_settings(s).unwrap();
    assert_eq!(f.h.store.settings().unwrap().analytics.retention_days, 30);

    let mut r = f.h.routing_config();
    r.preset = RoutingPreset::Manual;
    r.manual_model = None;
    assert!(f.h.update_routing(r).is_err());
}

#[tokio::test]
async fn model_preferences_disable_models() {
    let f = fixture().await;
    let acc = connect(&f, ProviderKind::Ollama).await;
    let key = format!("{}/local-small", acc.id);
    f.h.set_model_preference(&key, ModelPreference { disabled: true, ..Default::default() }).unwrap();
    assert!(f.h.route_preview(&ExecRequest::simple("hi")).is_err());
    assert!(f.h.set_model_preference("nope/x", ModelPreference::default()).is_err());
}

#[tokio::test]
async fn deleting_account_removes_everything() {
    let f = fixture().await;
    let acc = connect(&f, ProviderKind::OpenAi).await;
    f.h.delete_account(&acc.id).await.unwrap();
    assert!(f.h.list_accounts().is_empty());
    assert!(f.h.list_models().is_empty());
    assert!(f.h.secrets.get(&acc.id, None).unwrap().is_none());
}

#[tokio::test]
async fn limit_notifications_fire_once() {
    let f = fixture().await;
    let acc = connect(&f, ProviderKind::ClaudeCode).await;
    let mut w = LimitWindow::new(&acc.id, "weekly", "Weekly", LimitMetric::UsagePercent, Provenance::Reported);
    w.used_percent = Some(85.0);
    w.resets_at = Some(now() + chrono::Duration::days(2));
    f.h.apply_limits(&acc.id, vec![w.clone()]);
    f.h.apply_limits(&acc.id, vec![w]);
    let n = f.h.notifications(10).unwrap();
    assert_eq!(n.iter().filter(|n| n.kind == NotificationKind::LimitApproaching).count(), 1);
    let al = f.h.account_limits();
    assert_eq!(al[0].state, LimitState::Approaching);
}

#[test]
fn bind_validation() {
    assert!(validate_bind("127.0.0.1", false).is_ok());
    assert!(validate_bind("::1", false).is_ok());
    assert!(validate_bind("0.0.0.0", false).is_err());
    assert!(validate_bind("0.0.0.0", true).is_ok());
    assert!(validate_bind("localhost", false).is_err());
}

#[tokio::test]
async fn reported_windows_survive_failed_claude_execution() {
    let f = fixture().await;
    let acc = connect(&f, ProviderKind::ClaudeCode).await;
    let mut w = LimitWindow::new("adapter-local-id", "claude.five_hour", "5-hour session", LimitMetric::UsagePercent, Provenance::Reported);
    w.exhausted = true;
    w.used_percent = Some(100.0);
    w.resets_at = Some(now() + chrono::Duration::hours(2));
    let error = HarnessError::new(ErrorKind::QuotaExhausted, "Allowance reached");
    f.mocks[&ProviderKind::ClaudeCode].script("sub-top", vec![Script::FailAfterLimits(vec![w.clone()], error)]);
    assert!(f.h.execute_collect(ExecRequest::simple("hi"), "test".into()).await.is_err());
    let limits = f.h.limit_windows();
    let reported = limits.iter().find(|l| l.key == "claude.five_hour").unwrap();
    assert_eq!(reported.account_id, acc.id);
    assert_eq!(reported.resets_at, w.resets_at);
    assert_eq!(reported.used_percent, Some(100.0));
    assert!(f.h.store.list_limits().unwrap().iter().any(|l| l.key == w.key));
}

#[tokio::test]
async fn imports_only_matching_new_claude_statusline_reports() {
    use magpie_providers::cli::claude_usage::{UsageSnapshot, SNAPSHOT_FILE};
    let f = fixture().await;
    let acc = connect(&f, ProviderKind::ClaudeCode).await;
    let payload = serde_json::json!({"rate_limits":{"five_hour":{"used_percentage":37.0,"resets_at":(now()+chrono::Duration::hours(3)).timestamp()}}});
    let mut snapshot = UsageSnapshot::from_statusline("another-account", &payload).unwrap();
    let path = f.h.paths.root.join(SNAPSHOT_FILE);
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    f.h.import_claude_usage(&acc.id);
    assert!(f.h.limit_windows().is_empty());
    snapshot.account_id = acc.id.clone();
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    f.h.refresh_limits(&acc.id).await.unwrap();
    let observed = f.h.limit_windows()[0].clone();
    assert_eq!(observed.used_percent, Some(37.0));
    assert_eq!(f.h.store.list_limits().unwrap()[0].used_percent, Some(37.0));
    snapshot.observed_at -= chrono::Duration::seconds(5);
    snapshot.rate_limits.five_hour.as_mut().unwrap().used_percentage = Some(12.0);
    std::fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    f.h.import_claude_usage(&acc.id);
    assert_eq!(f.h.limit_windows()[0], observed);
}

#[tokio::test]
async fn failed_autostart_does_not_persist_success() {
    let f = fixture().await;
    let before = f.h.settings();
    let mut changed = before.clone();
    changed.general.launch_on_startup = true;
    assert!(f.h.update_settings(changed).is_err());
    assert_eq!(f.h.settings(), before);
    assert_eq!(f.h.store.settings().unwrap(), before);
}

#[tokio::test]
async fn invalid_server_settings_are_rejected_without_mutation() {
    let f = fixture().await;
    let before = f.h.settings();
    let mut changed = before.clone();
    changed.server.max_concurrency = 0;
    assert!(f.h.update_settings(changed).is_err());
    assert_eq!(f.h.settings(), before);
}

fn profile_request(kind: ProviderKind, label: &str, mode: CliAuthMode, token: Option<&str>) -> ConnectRequest {
    serde_json::from_value(serde_json::json!({"kind": kind, "label": label, "auth_mode": mode, "oauth_token": token})).unwrap()
}

#[tokio::test]
async fn saved_credentials_are_independent_persist_and_revoke_individually() {
    let f = fixture().await;
    let first =
        f.h.connect(profile_request(ProviderKind::ClaudeCode, "Work", CliAuthMode::SavedToken, Some("sk-ant-oat01-work-test-credential")))
            .await
            .unwrap();
    let second =
        f.h.connect(profile_request(
            ProviderKind::ClaudeCode,
            "Personal",
            CliAuthMode::SavedToken,
            Some("sk-ant-oat01-personal-test-credential"),
        ))
        .await
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(first.auth_method, AuthMethod::CliToken);
    assert_eq!(f.h.secrets.get(&first.id, None).unwrap().as_deref(), Some("sk-ant-oat01-work-test-credential"));
    assert_eq!(f.h.secrets.get(&second.id, None).unwrap().as_deref(), Some("sk-ant-oat01-personal-test-credential"));
    let public = serde_json::to_string(&f.h.list_accounts()).unwrap();
    assert!(!public.contains("sk-ant"));
    assert!(!public.contains("_profile_home"));
    let restored = Harness::open(HarnessOptions {
        paths: f.h.paths.clone(),
        secrets: f.h.secrets.clone(),
        secret_backend: SecretBackend::Memory,
        adapter_factory: Some(f.h.factory.clone()),
        launch_command: None,
        background: false,
    })
    .await
    .unwrap();
    assert_eq!(restored.list_accounts().len(), 2);
    assert!(restored.adapter_for(&second.id).is_some());
    restored.delete_account(&first.id).await.unwrap();
    assert_eq!(restored.secrets.get(&first.id, None).unwrap(), None);
    assert!(restored.secrets.get(&second.id, None).unwrap().is_some());
    assert!(!restored.paths.root.join("auth-profiles").join(&first.id).exists());
    assert!(restored.paths.root.join("auth-profiles").join(&second.id).exists());
}

#[tokio::test]
async fn separate_codex_profiles_start_disconnected_and_cannot_override_another_home() {
    let f = fixture().await;
    let first = f.h.connect(profile_request(ProviderKind::CodexCli, "Work", CliAuthMode::Isolated, None)).await.unwrap();
    let second = f.h.connect(profile_request(ProviderKind::CodexCli, "Personal", CliAuthMode::Isolated, None)).await.unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(first.status, ConnectionStatus::NeedsAuth);
    assert!(first.last_verified_at.is_none());
    assert!(f.h.list_models().is_empty(), "unauthed profiles cannot route requests");
    let err =
        f.h.update_account(
            &first.id,
            AccountPatch { options: Some(serde_json::json!({"_profile_home": "/another-account"})), ..Default::default() },
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::InvalidRequest);
    let renamed = f.h.update_account(&first.id, AccountPatch { label: Some("Renamed".into()), ..Default::default() }).await.unwrap();
    assert_eq!(renamed.cli_auth_mode(), CliAuthMode::Isolated);
    assert_eq!(renamed.status, ConnectionStatus::NeedsAuth);
    assert!(ProviderKind::CodexCli.descriptor().allow_multiple);
    assert!(ProviderKind::ClaudeCode.descriptor().allow_multiple);
    assert!(!ProviderKind::GeminiCli.descriptor().allow_multiple);
}

#[tokio::test]
async fn a_shared_cli_login_cannot_be_connected_twice_even_concurrently() {
    let f = fixture().await;
    let req = profile_request(ProviderKind::ClaudeCode, "Existing", CliAuthMode::Existing, None);
    let (a, b) = tokio::join!(f.h.connect(req.clone()), f.h.connect(req));
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(f.h.list_accounts().len(), 1);
    assert!(f.h.connect(profile_request(ProviderKind::GeminiCli, "Unsupported", CliAuthMode::Isolated, None)).await.is_err());
    assert!(f.h.connect(profile_request(ProviderKind::ClaudeCode, "No token", CliAuthMode::SavedToken, None)).await.is_err());
}

#[tokio::test]
async fn fallback_skips_all_models_on_an_expired_or_exhausted_saved_account() {
    for failure in [ErrorKind::Authentication, ErrorKind::QuotaExhausted] {
        let f = fixture().await;
        let first =
            f.h.connect(profile_request(
                ProviderKind::ClaudeCode,
                "Primary",
                CliAuthMode::SavedToken,
                Some("sk-ant-oat01-primary-test-credential"),
            ))
            .await
            .unwrap();
        let second =
            f.h.connect(profile_request(
                ProviderKind::ClaudeCode,
                "Backup",
                CliAuthMode::SavedToken,
                Some("sk-ant-oat01-backup-test-credential"),
            ))
            .await
            .unwrap();
        // More unusable models than MAX_ATTEMPTS, all ahead of the backup.
        for i in 0..5 {
            f.h.models.write().push((
                first.id.clone(),
                model(&format!("premium-{i}"), QualityTier::Frontier, SpeedClass::Fast, caps(false), None),
                now(),
            ));
        }
        for m in f.h.list_models().iter().filter(|m| m.account_id == first.id) {
            f.h.set_model_preference(&m.key, ModelPreference { priority: 10, ..Default::default() }).unwrap();
        }
        f.mocks[&ProviderKind::ClaudeCode].script("sub-top", vec![Script::fail(failure), Script::text("backup credential")]);
        let mut request = ExecRequest::simple("Plan a complex migration");
        request.model = ModelInfo::make_key(&first.id, "sub-top");
        request.preferences.allow_fallback = Some(true);
        let (result, events) = f.h.execute_collect(request, "profile test".into()).await.unwrap();
        assert_eq!(result.model.account_id, second.id);
        assert_eq!(result.attempts, 2, "unusable models must not spend attempts");
        assert_eq!(result.output_text, "backup credential");
        assert!(events
            .iter()
            .any(|e| matches!(e, ExecEvent::RoutingChanged { from, to, .. } if from.account_id == first.id && to.account_id == second.id)));
        if failure == ErrorKind::Authentication {
            assert_eq!(f.h.get_account(&first.id).unwrap().status, ConnectionStatus::NeedsAuth);
            assert_eq!(
                f.h.verify_account(&first.id).await.unwrap().status,
                ConnectionStatus::NeedsAuth,
                "local credential presence cannot revive an expired token"
            );
        }
        assert_eq!(f.h.get_account(&second.id).unwrap().status, ConnectionStatus::Connected);
    }
}

#[tokio::test]
async fn saved_accounts_respect_manual_no_fallback_and_partial_stream_safety() {
    for partial in [false, true] {
        let f = fixture().await;
        let first =
            f.h.connect(profile_request(
                ProviderKind::ClaudeCode,
                "Primary",
                CliAuthMode::SavedToken,
                Some("sk-ant-oat01-primary-test-credential"),
            ))
            .await
            .unwrap();
        f.h.connect(profile_request(
            ProviderKind::ClaudeCode,
            "Backup",
            CliAuthMode::SavedToken,
            Some("sk-ant-oat01-backup-test-credential"),
        ))
        .await
        .unwrap();
        let fail = HarnessError::new(ErrorKind::Authentication, "expired");
        f.mocks[&ProviderKind::ClaudeCode]
            .script("sub-top", vec![if partial { Script::FailAfterOutput("partial".into(), fail) } else { Script::Fail(fail) }]);
        let mut req = ExecRequest::simple("test");
        req.model = ModelInfo::make_key(&first.id, "sub-top");
        req.preferences.allow_fallback = Some(partial);
        assert_eq!(f.h.execute_collect(req, "test".into()).await.unwrap_err().kind, ErrorKind::Authentication);
        assert_eq!(f.mocks[&ProviderKind::ClaudeCode].calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn cannot_disconnect_a_saved_credential_while_a_fallback_is_using_it() {
    let f = fixture().await;
    let primary =
        f.h.connect(profile_request(
            ProviderKind::ClaudeCode,
            "Primary",
            CliAuthMode::SavedToken,
            Some("sk-ant-oat01-primary-test-credential"),
        ))
        .await
        .unwrap();
    let backup = f
        .h
        .connect(profile_request(ProviderKind::ClaudeCode, "Backup", CliAuthMode::SavedToken, Some("sk-ant-oat01-backup-test-credential")))
        .await
        .unwrap();
    f.mocks[&ProviderKind::ClaudeCode].script(
        "sub-top",
        vec![Script::fail(ErrorKind::Authentication), Script::Delay(Duration::from_secs(30), Box::new(Script::text("ok")))],
    );
    let mut req = ExecRequest::simple("test");
    req.model = ModelInfo::make_key(&primary.id, "sub-top");
    req.preferences.allow_fallback = Some(true);
    let mut handle = f.h.execute(req, "test".into()).await.unwrap();
    while let Some(event) = handle.events.recv().await {
        if matches!(event, ExecEvent::RoutingChanged { .. }) {
            break;
        }
    }
    assert_eq!(f.h.active.read()[&handle.id].summary.model.as_ref().unwrap().account_id, backup.id);
    assert_eq!(f.h.delete_account(&backup.id).await.unwrap_err().kind, ErrorKind::InvalidRequest);
    assert!(f.h.secrets.get(&backup.id, None).unwrap().is_some());
    assert!(f.h.cancel(&handle.id));
    while handle.events.recv().await.is_some() {}
    f.h.delete_account(&backup.id).await.unwrap();
}

#[tokio::test]
async fn disconnect_waits_for_credential_replacement_then_removes_the_new_secret() {
    use magpie_security::secrets::SecretError;
    use magpie_security::SecretStore;
    struct DelayedStore {
        inner: MemorySecretStore,
        armed: std::sync::atomic::AtomicBool,
        entered: tokio::sync::Notify,
        release: std::sync::Barrier,
    }
    impl SecretStore for DelayedStore {
        fn set(&self, id: &str, secret: &str) -> Result<SecretBackend, SecretError> {
            if self.armed.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.wait();
            }
            self.inner.set(id, secret)
        }
        fn get(&self, id: &str, backend: Option<SecretBackend>) -> Result<Option<String>, SecretError> {
            self.inner.get(id, backend)
        }
        fn delete(&self, id: &str, backend: Option<SecretBackend>) -> Result<(), SecretError> {
            self.inner.delete(id, backend)
        }
    }
    let mut f = fixture().await;
    let secrets = Arc::new(DelayedStore {
        inner: MemorySecretStore::default(),
        armed: false.into(),
        entered: tokio::sync::Notify::new(),
        release: std::sync::Barrier::new(2),
    });
    Arc::get_mut(&mut f.h).unwrap().secrets = secrets.clone();
    let account = connect(&f, ProviderKind::OpenAi).await;
    secrets.armed.store(true, Ordering::SeqCst);
    let h = f.h.clone();
    let id = account.id.clone();
    let replacing = tokio::spawn(async move {
        h.update_account(&id, AccountPatch { api_key: Some("sk-test-replacement-credential".into()), ..Default::default() }).await
    });
    secrets.entered.notified().await;
    let h = f.h.clone();
    let id = account.id.clone();
    let revoking = tokio::spawn(async move { h.delete_account(&id).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let removed_during_write = revoking.is_finished();
    // Always release the blocking writer, even if the regression was detected.
    secrets.release.wait();
    let replacement = replacing.await.unwrap();
    let revocation = revoking.await.unwrap();
    assert!(!removed_during_write, "disconnect must not race a keyring write");
    replacement.unwrap();
    revocation.unwrap();
    assert!(f.h.get_account(&account.id).is_none());
    assert!(secrets.get(&account.id, None).unwrap().is_none(), "replacement must not leave an orphan credential");
}
