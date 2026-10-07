use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use magpie_engine::{Principal, Scope};

use crate::error::ApiError;
use crate::AppState;

/// Authenticated caller, resolved from `Authorization: Bearer <key>` or
/// `x-api-key: <key>` (for Anthropic-style clients).
pub struct Auth(pub Principal);

impl Auth {
    pub fn require(&self, scope: Scope) -> Result<(), ApiError> {
        if self.0.has(scope) {
            Ok(())
        } else {
            Err(ApiError::forbidden(scope.as_str()))
        }
    }
}

fn token_from(parts: &Parts) -> Option<String> {
    if let Some(v) = parts.headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let v = v.trim();
        if let Some(t) = v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")) {
            return Some(t.trim().to_string());
        }
    }
    parts.headers.get("x-api-key").and_then(|v| v.to_str().ok()).map(|s| s.trim().to_string())
}

impl FromRequestParts<AppState> for Auth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = token_from(parts).ok_or_else(ApiError::unauthorized)?;
        state.harness.authenticate(&token).map(Auth).ok_or_else(ApiError::unauthorized)
    }
}

/// Reject requests whose Host header is not a loopback name. Prevents DNS
/// rebinding attacks from web pages against the local API.
pub async fn host_guard(State(state): State<AppState>, req: Request<axum::body::Body>, next: Next) -> Response {
    if state.allow_network {
        return next.run(req).await;
    }
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .or_else(|| req.uri().host())
        .unwrap_or("");
    let name = if host.starts_with('[') {
        host.split(']').next().map(|s| format!("{s}]")).unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or("").to_string()
    };
    let ok = matches!(name.as_str(), "127.0.0.1" | "localhost" | "[::1]" | "tauri.localhost" | "");
    if !ok {
        return (StatusCode::FORBIDDEN, "Host not allowed").into_response();
    }
    next.run(req).await
}
