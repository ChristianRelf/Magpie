//! Provider adapters.
//!
//! Each adapter implements [`ProviderAdapter`] for one protocol family and is
//! independently testable against mocked HTTP servers or scripted CLIs.
//! Adapters never decide routing; they translate the unified request to the
//! provider protocol, stream events back, classify errors, and report
//! usage/limit information exactly as the provider exposes it.

pub mod anthropic;
pub mod catalog;
pub mod cli;
pub mod gemini;
pub mod http;
pub mod openai_compat;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use magpie_core::*;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// A request as handed to an adapter: the unified request plus the concrete
/// model chosen by the router.
#[derive(Debug, Clone)]
pub struct AdapterRequest {
    pub model_id: String,
    pub request: ExecRequest,
    /// Maximum output tokens to request when the client did not specify.
    pub default_max_output: u32,
    pub timeout: Duration,
    /// Working directory for CLI adapters when not running in agent mode.
    pub scratch_dir: std::path::PathBuf,
}

pub type EventSender = mpsc::Sender<ProviderEvent>;

#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    fn account(&self) -> &Account;

    /// Check that the connection works and report identity/plan where the
    /// provider exposes them. Must not consume model allowance.
    async fn verify(&self) -> HarnessResult<VerifiedIdentity>;

    /// List models available to this account.
    async fn discover_models(&self) -> HarnessResult<Vec<DiscoveredModel>>;

    /// Execute a request, streaming events to `events`. Implementations must
    /// stop promptly when `cancel` fires.
    async fn execute(&self, req: &AdapterRequest, events: EventSender, cancel: CancellationToken) -> HarnessResult<ProviderOutcome>;

    /// Fetch current limit/quota state from a dedicated, allowance-free
    /// endpoint. Returns an empty list when the provider exposes none.
    async fn fetch_limits(&self) -> HarnessResult<Vec<LimitWindow>> {
        Ok(vec![])
    }

    /// Whether `fetch_limits` queries something useful. Used to schedule
    /// polling only where it adds information.
    fn supports_limit_polling(&self) -> bool {
        false
    }

    /// Account-wide usage reported by the provider, when available.
    async fn fetch_usage_report(&self) -> HarnessResult<Option<ProviderUsageReport>> {
        Ok(None)
    }

    /// Start the official CLI's browser flow for an isolated profile.
    async fn begin_login(&self) -> HarnessResult<LoginChallenge> {
        Err(HarnessError::new(ErrorKind::Unsupported, "Browser sign-in is unavailable for this connection"))
    }

    /// Revoke only credentials owned by this Magpie profile. Never log out
    /// the user's shared CLI/IDE account.
    async fn revoke_auth(&self) -> HarnessResult<()> {
        Ok(())
    }

    /// Release long-lived resources (child processes).
    async fn shutdown(&self) {}
}

pub type SharedAdapter = Arc<dyn ProviderAdapter>;

/// Construct the adapter for an account. `secret` is the API key for
/// key-authenticated providers.
pub fn build_adapter(account: Account, secret: Option<String>) -> HarnessResult<SharedAdapter> {
    use ProviderKind::*;
    let adapter: SharedAdapter = match account.kind {
        Anthropic => Arc::new(anthropic::AnthropicAdapter::new(account, secret)?),
        Gemini => Arc::new(gemini::GeminiAdapter::new(account, secret)?),
        OpenAi | OpenRouter | Groq | Mistral | DeepSeek | Ollama | LmStudio | OpenAiCompatible => {
            Arc::new(openai_compat::OpenAiCompatAdapter::new(account, secret)?)
        }
        ClaudeCode => Arc::new(cli::claude_code::ClaudeCodeAdapter::with_token(account, secret)?),
        CodexCli => Arc::new(cli::codex::CodexAdapter::new(account)),
        GeminiCli => Arc::new(cli::gemini_cli::GeminiCliAdapter::new(account)),
    };
    Ok(adapter)
}

/// Emit an event, treating a closed receiver as cancellation.
pub(crate) async fn emit(events: &EventSender, ev: ProviderEvent) -> HarnessResult<()> {
    events.send(ev).await.map_err(|_| HarnessError::cancelled())
}

/// Extra models configured by the user on an account (`options.extra_models`).
pub(crate) fn extra_models(account: &Account) -> Vec<String> {
    account
        .options
        .get("extra_models")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}
