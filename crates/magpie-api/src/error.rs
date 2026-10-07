use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use magpie_core::{ErrorKind, HarnessError};
use serde_json::json;

/// API error rendered in the OpenAI-compatible `{ "error": { ... } }` shape
/// so existing client libraries surface messages correctly.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub kind: String,
    pub message: String,
    pub retry_after: Option<u64>,
    pub detail: Option<HarnessError>,
}

impl ApiError {
    pub fn new(status: StatusCode, kind: &str, message: impl Into<String>) -> Self {
        Self { status, kind: kind.into(), message: message.into(), retry_after: None, detail: None }
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "invalid_api_key", "Missing or invalid Magpie API key")
    }

    pub fn forbidden(scope: &str) -> Self {
        Self::new(StatusCode::FORBIDDEN, "insufficient_scope", format!("This key lacks the `{scope}` scope"))
    }

    pub fn not_found(what: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", format!("{what} not found"))
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", msg)
    }
}

pub fn status_for(kind: ErrorKind) -> StatusCode {
    match kind {
        ErrorKind::InvalidRequest | ErrorKind::ContextLength | ErrorKind::Unsupported => StatusCode::BAD_REQUEST,
        // Upstream credential problems are not the client's fault.
        ErrorKind::Authentication | ErrorKind::PermissionDenied => StatusCode::BAD_GATEWAY,
        ErrorKind::ModelNotFound => StatusCode::NOT_FOUND,
        ErrorKind::RateLimited | ErrorKind::QuotaExhausted => StatusCode::TOO_MANY_REQUESTS,
        ErrorKind::ProviderUnavailable | ErrorKind::Network => StatusCode::BAD_GATEWAY,
        ErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT,
        ErrorKind::Cancelled => StatusCode::from_u16(499).unwrap_or(StatusCode::BAD_REQUEST),
        ErrorKind::LocalDependency | ErrorKind::NoEligibleModel => StatusCode::SERVICE_UNAVAILABLE,
        ErrorKind::Refused => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl From<HarnessError> for ApiError {
    fn from(e: HarnessError) -> Self {
        let kind = match e.kind {
            ErrorKind::QuotaExhausted => "insufficient_quota".to_string(),
            ErrorKind::Authentication => "upstream_authentication".to_string(),
            k => k.as_str().to_string(),
        };
        Self {
            status: status_for(e.kind),
            kind,
            message: magpie_security::redact(&e.message),
            retry_after: e.retry_after_secs,
            detail: Some(e),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({"error": {"message": self.message, "type": self.kind, "code": self.kind}});
        if let Some(d) = &self.detail {
            if let Some(r) = d.resets_at {
                body["error"]["resets_at"] = json!(r);
            }
        }
        let mut resp = (self.status, Json(body)).into_response();
        if let Some(s) = self.retry_after {
            if let Ok(v) = s.to_string().parse() {
                resp.headers_mut().insert("retry-after", v);
            }
        }
        resp
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
