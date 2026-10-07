//! Local HTTP API for the harness.
//!
//! * Loopback-only by default, with Host-header validation against DNS
//!   rebinding.
//! * Every endpoint except `/health` requires a bearer key; scopes gate
//!   execution, read and admin access.
//! * Native endpoints live under `/v1`; `/v1/chat/completions` and
//!   `/v1/models` are OpenAI-compatible.

mod auth;
mod convert;
mod error;
mod native;
mod openai;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderValue, Method};
use axum::routing::{delete, get, post};
use axum::Router;
use magpie_core::*;
use magpie_engine::Harness;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, CorsLayer};

pub use error::{ApiError, ApiResult};

#[derive(Clone)]
pub struct AppState {
    pub harness: Arc<Harness>,
    pub allow_network: bool,
    /// Cancelled to stop the server (e.g. `POST /v1/admin/shutdown`).
    pub stop: CancellationToken,
}

/// Origins allowed to call the API from a browser context: the desktop
/// app's webview and its development server.
const ALLOWED_ORIGINS: &[&str] = &["tauri://localhost", "http://tauri.localhost", "https://tauri.localhost", "http://localhost:1420", "http://127.0.0.1:1420"];

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(ALLOWED_ORIGINS.iter().map(|o| HeaderValue::from_static(o))))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
        .allow_headers([axum::http::header::AUTHORIZATION, axum::http::header::CONTENT_TYPE, "x-api-key".parse().unwrap()])
        .max_age(Duration::from_secs(600));

    Router::new()
        .route("/health", get(native::health))
        // Execution
        .route("/v1/responses", post(native::responses))
        .route("/v1/chat/completions", post(openai::chat_completions))
        .route("/v1/route", post(native::route_preview))
        // Inventory
        .route("/v1/models", get(openai::list_models))
        .route("/v1/model-preferences", post(native::set_model_preference))
        .route("/v1/status", get(native::status))
        .route("/v1/providers", get(native::list_providers).post(native::connect_provider))
        .route("/v1/providers/cli", get(native::detect_clis))
        .route("/v1/providers/{id}", axum::routing::patch(native::update_provider).delete(native::delete_provider))
        .route("/v1/providers/{id}/verify", post(native::verify_provider))
        .route("/v1/providers/{id}/refresh", post(native::refresh_provider))
        .route("/v1/limits", get(native::limits))
        // Telemetry
        .route("/v1/usage/summary", get(native::usage_summary))
        .route("/v1/usage/timeseries", get(native::usage_timeseries))
        .route("/v1/usage/breakdown", get(native::usage_breakdown))
        .route("/v1/usage/provider-reports", get(native::provider_reports))
        .route("/v1/usage/export", get(native::usage_export))
        .route("/v1/history", delete(native::clear_history))
        .route("/v1/executions", get(native::list_executions))
        .route("/v1/executions/active", get(native::active_executions))
        .route("/v1/executions/{id}", get(native::get_execution))
        .route("/v1/executions/{id}/cancel", post(native::cancel_execution))
        .route("/v1/events", get(native::events))
        // Configuration
        .route("/v1/settings", get(native::get_settings).put(native::put_settings))
        .route("/v1/routing", get(native::get_routing).put(native::put_routing))
        .route("/v1/keys", get(native::list_keys).post(native::create_key))
        .route("/v1/keys/{id}", delete(native::revoke_key))
        .route("/v1/notifications", get(native::list_notifications))
        .route("/v1/notifications/read", post(native::read_notifications))
        .route("/v1/admin/shutdown", post(native::shutdown))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::host_guard))
        .layer(cors)
        .with_state(state)
}

/// Information about the running harness, written to `harness.json` so the
/// desktop app and CLI can find it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub pid: u32,
    pub port: u16,
    pub url: String,
    pub version: String,
    pub api_version: u32,
    pub started_at: Timestamp,
}

/// Bind and serve until `stop` is cancelled. Writes the runtime file after
/// binding and removes it on exit.
pub async fn serve(harness: Arc<Harness>, stop: CancellationToken) -> HarnessResult<()> {
    let settings = harness.settings();
    magpie_engine::validate_bind(&settings.server.bind, settings.server.allow_network)?;
    let ip: std::net::IpAddr = settings.server.bind.parse().map_err(|_| HarnessError::invalid("invalid bind address"))?;
    let addr = SocketAddr::new(ip, settings.server.port);
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        HarnessError::new(ErrorKind::LocalDependency, format!("Could not listen on {addr}: {e}. Is another harness running?"))
    })?;
    serve_on(harness, listener, settings.server.allow_network, stop).await
}

pub async fn serve_on(harness: Arc<Harness>, listener: tokio::net::TcpListener, allow_network: bool, stop: CancellationToken) -> HarnessResult<()> {
    let local = listener.local_addr().map_err(|e| HarnessError::internal(e.to_string()))?;
    let host = if local.ip().is_unspecified() { "127.0.0.1".to_string() } else if local.ip().is_ipv6() { format!("[{}]", local.ip()) } else { local.ip().to_string() };
    let info = RuntimeInfo {
        pid: std::process::id(),
        port: local.port(),
        url: format!("http://{host}:{}", local.port()),
        version: VERSION.to_string(),
        api_version: API_VERSION,
        started_at: harness.started_at,
    };
    let runtime_path = harness.paths.runtime_file();
    let _ = magpie_security::secrets::write_private(&runtime_path, serde_json::to_string_pretty(&info).unwrap_or_default().as_bytes());
    tracing::info!(url = %info.url, "harness API listening");

    let state = AppState { harness: harness.clone(), allow_network, stop: stop.clone() };
    let app = router(state);
    let shutdown_harness = harness.clone();
    let result = axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(async move {
            stop.cancelled().await;
            shutdown_harness.shutdown().await;
        })
        .await;

    // Only remove the runtime file if it still describes this process.
    if let Ok(raw) = std::fs::read_to_string(&runtime_path) {
        if serde_json::from_str::<RuntimeInfo>(&raw).map(|r| r.pid == std::process::id()).unwrap_or(false) {
            let _ = std::fs::remove_file(&runtime_path);
        }
    }
    result.map_err(|e| HarnessError::internal(e.to_string()))
}

#[cfg(test)]
mod tests;
