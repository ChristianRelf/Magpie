use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// Classification of a provider failure. The classification drives retry and
/// fallback decisions, so adapters must map provider responses carefully.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// Credentials missing, invalid, expired or revoked.
    Authentication,
    /// Credentials valid but not permitted to use this model/feature.
    PermissionDenied,
    /// Short-term rate limit (requests/tokens per minute).
    RateLimited,
    /// Allowance, credit balance or subscription window exhausted.
    QuotaExhausted,
    /// The request was malformed or unsupported by the target model.
    InvalidRequest,
    /// The request exceeds the model's context window.
    ContextLength,
    /// The model or capability is not supported by this provider.
    Unsupported,
    /// The model was not found on this account.
    ModelNotFound,
    /// Provider returned a 5xx or reported overload.
    ProviderUnavailable,
    /// Network-level failure before a response was received.
    Network,
    /// Request exceeded the configured timeout.
    Timeout,
    /// The request was cancelled by the client or user.
    Cancelled,
    /// A local dependency (e.g. a provider CLI) is missing or broken.
    LocalDependency,
    /// The provider refused to produce output (safety refusal).
    Refused,
    /// No model satisfied the routing constraints.
    NoEligibleModel,
    /// Unexpected internal failure.
    Internal,
}

impl ErrorKind {
    /// Whether retrying the same request on a *different* model is safe and
    /// potentially useful. Requests that were malformed will fail anywhere.
    pub fn is_failover_candidate(self) -> bool {
        matches!(
            self,
            ErrorKind::RateLimited
                | ErrorKind::QuotaExhausted
                | ErrorKind::ProviderUnavailable
                | ErrorKind::Network
                | ErrorKind::Timeout
                | ErrorKind::Authentication
                | ErrorKind::PermissionDenied
                | ErrorKind::ModelNotFound
                | ErrorKind::LocalDependency
                | ErrorKind::ContextLength
                | ErrorKind::Unsupported
        )
    }

    /// Whether retrying the same request on the *same* model might succeed.
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            ErrorKind::ProviderUnavailable | ErrorKind::Network | ErrorKind::Timeout
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::Authentication => "authentication",
            ErrorKind::PermissionDenied => "permission_denied",
            ErrorKind::RateLimited => "rate_limited",
            ErrorKind::QuotaExhausted => "quota_exhausted",
            ErrorKind::InvalidRequest => "invalid_request",
            ErrorKind::ContextLength => "context_length",
            ErrorKind::Unsupported => "unsupported",
            ErrorKind::ModelNotFound => "model_not_found",
            ErrorKind::ProviderUnavailable => "provider_unavailable",
            ErrorKind::Network => "network",
            ErrorKind::Timeout => "timeout",
            ErrorKind::Cancelled => "cancelled",
            ErrorKind::LocalDependency => "local_dependency",
            ErrorKind::Refused => "refused",
            ErrorKind::NoEligibleModel => "no_eligible_model",
            ErrorKind::Internal => "internal",
        }
    }
}

/// A structured provider/harness error. Messages must never contain secrets;
/// adapters pass provider messages through `magpie_security::redact`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct HarnessError {
    pub kind: ErrorKind,
    pub message: String,
    /// HTTP status or process exit code reported by the provider, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    /// Provider-suggested wait before retrying.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
    /// Provider-reported reset time of the limit that was hit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<Timestamp>,
}

impl HarnessError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            status: None,
            retry_after_secs: None,
            resets_at: None,
        }
    }

    pub fn with_status(mut self, status: i32) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_retry_after(mut self, secs: Option<u64>) -> Self {
        self.retry_after_secs = secs;
        self
    }

    pub fn with_resets_at(mut self, at: Option<Timestamp>) -> Self {
        self.resets_at = at;
        self
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidRequest, message)
    }

    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "Request cancelled")
    }

    /// Map an HTTP status code to a default error classification.
    pub fn from_http_status(status: u16, message: impl Into<String>) -> Self {
        let kind = match status {
            401 => ErrorKind::Authentication,
            403 => ErrorKind::PermissionDenied,
            404 => ErrorKind::ModelNotFound,
            408 => ErrorKind::Timeout,
            413 => ErrorKind::ContextLength,
            400 | 409 | 415 | 422 => ErrorKind::InvalidRequest,
            402 => ErrorKind::QuotaExhausted,
            429 => ErrorKind::RateLimited,
            500..=599 => ErrorKind::ProviderUnavailable,
            _ => ErrorKind::Internal,
        };
        Self::new(kind, message).with_status(status as i32)
    }
}

pub type HarnessResult<T> = Result<T, HarnessError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_status_classification() {
        assert_eq!(HarnessError::from_http_status(429, "x").kind, ErrorKind::RateLimited);
        assert_eq!(HarnessError::from_http_status(401, "x").kind, ErrorKind::Authentication);
        assert_eq!(HarnessError::from_http_status(503, "x").kind, ErrorKind::ProviderUnavailable);
        assert_eq!(HarnessError::from_http_status(400, "x").kind, ErrorKind::InvalidRequest);
    }

    #[test]
    fn invalid_requests_do_not_fail_over() {
        assert!(!ErrorKind::InvalidRequest.is_failover_candidate());
        assert!(!ErrorKind::Cancelled.is_failover_candidate());
        assert!(ErrorKind::RateLimited.is_failover_candidate());
    }
}
