use std::sync::Arc;

use magpie_core::*;
use magpie_engine::{Harness, HarnessOptions};
use magpie_providers::testing::{model, MockAdapter, Script};
use magpie_security::{MemorySecretStore, SecretBackend};
use magpie_store::paths::Paths;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

struct Server {
    url: String,
    admin: String,
    harness: Arc<Harness>,
    mock: Arc<MockAdapter>,
    stop: CancellationToken,
    dir: std::path::PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.cancel();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn server() -> Server {
    let dir = std::env::temp_dir().join(format!("magpie-api-test-{}", uuid::Uuid::new_v4().simple()));
    let caps = Capabilities { streaming: true, tools: true, structured_output: true, vision: false, reasoning: false, agentic: false, system_prompt: true };
    let mock = Arc::new(MockAdapter::new(
        Account {
            id: "x".into(),
            kind: ProviderKind::Ollama,
            label: "x".into(),
            auth_method: AuthMethod::None,
            billing_mode: BillingMode::Local,
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
            options: json!({}),
        },
        vec![model("llama-test", QualityTier::Standard, SpeedClass::Fast, caps, None)],
    ));
    let m = mock.clone();
    let harness = Harness::open(HarnessOptions {
        paths: Paths::at(dir.clone()),
        secrets: Arc::new(MemorySecretStore::default()),
        secret_backend: SecretBackend::Memory,
        adapter_factory: Some(Arc::new(move |_, _| Ok(m.clone() as magpie_providers::SharedAdapter))),
        launch_command: None,
        background: false,
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let stop = CancellationToken::new();
    tokio::spawn(crate::serve_on(harness.clone(), listener, false, stop.clone()));
    let admin = std::fs::read_to_string(harness.paths.admin_token()).unwrap();
    Server { url: format!("http://127.0.0.1:{port}"), admin, harness, mock, stop, dir }
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

async fn connect_mock(s: &Server) {
    let r = client()
        .post(format!("{}/v1/providers", s.url))
        .bearer_auth(&s.admin)
        .json(&json!({"kind": "ollama"}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201, "{}", r.text().await.unwrap());
}

#[tokio::test]
async fn health_is_public_everything_else_requires_auth() {
    let s = server().await;
    let r = client().get(format!("{}/health", s.url)).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = client().get(format!("{}/v1/status", s.url)).send().await.unwrap();
    assert_eq!(r.status(), 401);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_api_key");
    let r = client().get(format!("{}/v1/status", s.url)).bearer_auth("mgp_notarealkey123456789").send().await.unwrap();
    assert_eq!(r.status(), 401);
    let r = client().get(format!("{}/v1/status", s.url)).bearer_auth(&s.admin).send().await.unwrap();
    assert_eq!(r.status(), 200);
    // Runtime file advertises the server for the CLI and desktop app.
    let info: crate::RuntimeInfo = serde_json::from_str(&std::fs::read_to_string(s.harness.paths.runtime_file()).unwrap()).unwrap();
    assert_eq!(info.url, s.url);
}

#[tokio::test]
async fn rejects_foreign_host_headers() {
    let s = server().await;
    let r = client().get(format!("{}/health", s.url)).header("host", "evil.example.com").send().await.unwrap();
    assert_eq!(r.status(), 403);
}

#[tokio::test]
async fn cors_only_allows_desktop_origins() {
    let s = server().await;
    let preflight = |origin: &'static str| {
        client()
            .request(reqwest::Method::OPTIONS, format!("{}/v1/status", s.url))
            .header("origin", origin)
            .header("access-control-request-method", "GET")
            .header("access-control-request-headers", "authorization")
            .send()
    };
    let r = preflight("https://evil.example.com").await.unwrap();
    assert!(r.headers().get("access-control-allow-origin").is_none());
    let r = preflight("tauri://localhost").await.unwrap();
    assert_eq!(r.headers().get("access-control-allow-origin").unwrap(), "tauri://localhost");
}

#[tokio::test]
async fn chat_completions_end_to_end() {
    let s = server().await;
    connect_mock(&s).await;
    s.mock.script("llama-test", vec![Script::text("Hello from mock")]);

    let models: Value = client().get(format!("{}/v1/models", s.url)).bearer_auth(&s.admin).send().await.unwrap().json().await.unwrap();
    assert_eq!(models["object"], "list");
    assert_eq!(models["data"][0]["id"], "auto");
    assert_eq!(models["data"][1]["id"], "ollama/llama-test");

    let r: Value = client()
        .post(format!("{}/v1/chat/completions", s.url))
        .bearer_auth(&s.admin)
        .json(&json!({"model": "auto", "messages": [{"role": "system", "content": "be nice"}, {"role": "user", "content": "hi"}]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["object"], "chat.completion");
    assert_eq!(r["choices"][0]["message"]["content"], "Hello from mock");
    assert_eq!(r["usage"]["prompt_tokens"], 10);
    assert_eq!(r["model"], "ollama/llama-test");

    // Streaming
    let text = client()
        .post(format!("{}/v1/chat/completions", s.url))
        .bearer_auth(&s.admin)
        .json(&json!({"model": "ollama/llama-test", "stream": true, "stream_options": {"include_usage": true}, "messages": [{"role": "user", "content": "hi"}]}))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(text.contains("chat.completion.chunk"));
    assert!(text.contains("Hello from mock"));
    assert!(text.contains("\"usage\""));
    assert!(text.trim_end().ends_with("data: [DONE]"));
}

#[tokio::test]
async fn responses_endpoint_and_telemetry() {
    let s = server().await;
    connect_mock(&s).await;
    let r: Value = client()
        .post(format!("{}/v1/responses", s.url))
        .bearer_auth(&s.admin)
        .json(&json!({"model": "auto", "task_type": "code", "input": "Inspect this function for bugs",
                      "preferences": {"priority": "quality", "allow_fallback": true}}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["status"], "completed");
    assert_eq!(r["output_text"], "ok");
    assert_eq!(r["routing"]["task"], "code_generation");
    let id = r["id"].as_str().unwrap().to_string();

    let rec: Value = client().get(format!("{}/v1/executions/{id}", s.url)).bearer_auth(&s.admin).send().await.unwrap().json().await.unwrap();
    assert_eq!(rec["status"], "succeeded");
    assert!(rec["routing"]["candidates"].as_array().unwrap().len() >= 1);

    let sum: Value = client().get(format!("{}/v1/usage/summary?range=1h", s.url)).bearer_auth(&s.admin).send().await.unwrap().json().await.unwrap();
    assert_eq!(sum["current"]["requests"], 1);
    let ts: Value = client().get(format!("{}/v1/usage/timeseries?range=1h", s.url)).bearer_auth(&s.admin).send().await.unwrap().json().await.unwrap();
    assert_eq!(ts["points"].as_array().unwrap().iter().map(|p| p["requests"].as_u64().unwrap()).sum::<u64>(), 1);
    assert!(ts["points"].as_array().unwrap().len() >= 60);
    let csv = client().get(format!("{}/v1/usage/export?range=1h", s.url)).bearer_auth(&s.admin).send().await.unwrap().text().await.unwrap();
    assert_eq!(csv.lines().count(), 2);
    assert!(!csv.contains("Inspect this function"), "exports must not contain request content");

    // Streaming responses use named SSE events.
    let text = client()
        .post(format!("{}/v1/responses", s.url))
        .bearer_auth(&s.admin)
        .json(&json!({"input": "hi", "stream": true}))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(text.contains("event: started"));
    assert!(text.contains("event: completed"));
}

#[tokio::test]
async fn scoped_keys_are_enforced() {
    let s = server().await;
    connect_mock(&s).await;
    let created: Value = client()
        .post(format!("{}/v1/keys", s.url))
        .bearer_auth(&s.admin)
        .json(&json!({"name": "Editor", "scopes": ["execute"]}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let key = created["token"].as_str().unwrap().to_string();
    assert!(key.starts_with("mgp_"));
    // Execute allowed, admin and read denied.
    let r = client().post(format!("{}/v1/responses", s.url)).bearer_auth(&key).json(&json!({"input": "hi"})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = client().put(format!("{}/v1/routing", s.url)).bearer_auth(&key).json(&json!({})).send().await.unwrap();
    assert_eq!(r.status(), 403);
    let r = client().get(format!("{}/v1/usage/summary", s.url)).bearer_auth(&key).send().await.unwrap();
    assert_eq!(r.status(), 403);
    // Anthropic-style header also works.
    let r = client().post(format!("{}/v1/responses", s.url)).header("x-api-key", &key).json(&json!({"input": "hi"})).send().await.unwrap();
    assert_eq!(r.status(), 200);
    // Keys list never exposes tokens.
    let keys = client().get(format!("{}/v1/keys", s.url)).bearer_auth(&s.admin).send().await.unwrap().text().await.unwrap();
    assert!(!keys.contains(&key));
    // Revoke.
    let id = created["client"]["id"].as_str().unwrap();
    let r = client().delete(format!("{}/v1/keys/{id}", s.url)).bearer_auth(&s.admin).send().await.unwrap();
    assert_eq!(r.status(), 204);
    let r = client().post(format!("{}/v1/responses", s.url)).bearer_auth(&key).json(&json!({"input": "hi"})).send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn errors_use_openai_shape_and_status_codes() {
    let s = server().await;
    // No providers: routing fails with 503 and a helpful message.
    let r = client().post(format!("{}/v1/chat/completions", s.url)).bearer_auth(&s.admin).json(&json!({"messages": [{"role": "user", "content": "hi"}]})).send().await.unwrap();
    assert_eq!(r.status(), 503);
    let b: Value = r.json().await.unwrap();
    assert_eq!(b["error"]["type"], "no_eligible_model");
    // Bad input
    let r = client().post(format!("{}/v1/chat/completions", s.url)).bearer_auth(&s.admin).json(&json!({"model": "auto"})).send().await.unwrap();
    assert_eq!(r.status(), 400);
    // Upstream rate limit surfaces as 429
    connect_mock(&s).await;
    s.mock.script("llama-test", vec![Script::Fail(HarnessError::new(ErrorKind::RateLimited, "slow down").with_retry_after(Some(7)))]);
    let r = client().post(format!("{}/v1/responses", s.url)).bearer_auth(&s.admin).json(&json!({"input": "hi"})).send().await.unwrap();
    assert_eq!(r.status(), 429);
    assert_eq!(r.headers().get("retry-after").unwrap(), "7");
}

#[tokio::test]
async fn settings_and_routing_roundtrip() {
    let s = server().await;
    let v: Value = client().get(format!("{}/v1/settings", s.url)).bearer_auth(&s.admin).send().await.unwrap().json().await.unwrap();
    let mut settings = v["settings"].clone();
    settings["analytics"]["retention_days"] = json!(14);
    let r = client().put(format!("{}/v1/settings", s.url)).bearer_auth(&s.admin).json(&settings).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(s.harness.settings().analytics.retention_days, 14);
    settings["server"]["bind"] = json!("0.0.0.0");
    let r = client().put(format!("{}/v1/settings", s.url)).bearer_auth(&s.admin).json(&settings).send().await.unwrap();
    assert_eq!(r.status(), 400);

    let mut routing: Value = client().get(format!("{}/v1/routing", s.url)).bearer_auth(&s.admin).send().await.unwrap().json().await.unwrap();
    routing["preset"] = json!("fastest");
    let r = client().put(format!("{}/v1/routing", s.url)).bearer_auth(&s.admin).json(&routing).send().await.unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(s.harness.routing_config().preset, RoutingPreset::Fastest);
}

#[tokio::test]
async fn ordinary_execute_keys_cannot_start_filesystem_agents_or_cancel_others() {
    let s = server().await;
    let key = s.harness.create_client("limited", &["execute".into()]).unwrap();
    let response = client().post(format!("{}/v1/responses", s.url)).bearer_auth(&key.token)
        .json(&json!({"input":"inspect files", "agent":{"working_dir":"/tmp", "allow_writes":true}})).send().await.unwrap();
    assert_eq!(response.status(), 403);
    let response = client().post(format!("{}/v1/executions/another-client/cancel", s.url)).bearer_auth(&key.token).send().await.unwrap();
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn disabling_read_scopes_never_disables_authentication() {
    let s = server().await;
    let key = s.harness.create_client("execute-only", &["execute".into()]).unwrap();
    let url = format!("{}/v1/limits", s.url);
    assert_eq!(client().get(&url).bearer_auth(&key.token).send().await.unwrap().status(), 403);
    let mut settings = s.harness.settings();
    settings.security.require_scopes = false;
    s.harness.update_settings(settings).unwrap();
    assert_eq!(client().get(&url).bearer_auth(&key.token).send().await.unwrap().status(), 200);
    assert_eq!(client().get(&url).send().await.unwrap().status(), 401);
}
