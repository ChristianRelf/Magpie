use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// Every integration Magpie knows how to drive. A *provider kind* describes
/// the protocol and authentication mechanism; an *account* is a configured
/// connection of a given kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// OpenAI Platform API, authenticated with an API key.
    OpenAi,
    /// Anthropic API, authenticated with an API key.
    Anthropic,
    /// Google Gemini API (AI Studio), authenticated with an API key.
    Gemini,
    OpenRouter,
    Groq,
    Mistral,
    DeepSeek,
    /// Local Ollama server.
    Ollama,
    /// Local LM Studio server.
    LmStudio,
    /// Any server implementing the OpenAI chat-completions protocol.
    OpenAiCompatible,
    /// Claude subscription access delegated to the official Claude Code CLI.
    ClaudeCode,
    /// ChatGPT subscription access delegated to the official Codex CLI.
    CodexCli,
    /// Google account access delegated to the official Gemini CLI.
    GeminiCli,
}

/// How an account authenticates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// API key stored in the OS credential store.
    ApiKey,
    /// The provider's official CLI owns authentication; Magpie never sees the
    /// credential and invokes the CLI on the user's behalf.
    CliDelegated,
    /// No authentication (local servers).
    None,
}

/// How usage on an account is billed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingMode {
    /// Included in a consumer/pro subscription with usage windows.
    Subscription,
    /// Metered pay-as-you-go API billing.
    Metered,
    /// Prepaid credits (e.g. OpenRouter).
    Credits,
    /// Runs locally; no provider billing.
    Local,
    Unknown,
}

impl BillingMode {
    /// True when executing on this account may incur marginal charges.
    pub fn is_billable(self) -> bool {
        matches!(self, BillingMode::Metered | BillingMode::Credits | BillingMode::Unknown)
    }
}

/// Static description of a provider kind, used by the UI connect flow and
/// for defaults.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderDescriptor {
    pub kind: ProviderKind,
    pub name: &'static str,
    /// Model vendor family used for monochrome identity marks.
    pub vendor: &'static str,
    pub auth_method: AuthMethod,
    pub default_billing: BillingMode,
    pub default_base_url: Option<&'static str>,
    /// Name of the official CLI binary for CLI-delegated providers.
    pub cli_binary: Option<&'static str>,
    /// Official installation command shown to the user.
    pub cli_install: Option<&'static str>,
    /// Official sign-in command shown to the user.
    pub cli_login: Option<&'static str>,
    pub key_url: Option<&'static str>,
    pub docs_url: &'static str,
    pub is_local: bool,
    pub allow_multiple: bool,
    pub summary: &'static str,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 13] = [
        ProviderKind::ClaudeCode,
        ProviderKind::CodexCli,
        ProviderKind::GeminiCli,
        ProviderKind::Anthropic,
        ProviderKind::OpenAi,
        ProviderKind::Gemini,
        ProviderKind::OpenRouter,
        ProviderKind::Groq,
        ProviderKind::Mistral,
        ProviderKind::DeepSeek,
        ProviderKind::Ollama,
        ProviderKind::LmStudio,
        ProviderKind::OpenAiCompatible,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::OpenAi => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Gemini => "gemini",
            ProviderKind::OpenRouter => "open_router",
            ProviderKind::Groq => "groq",
            ProviderKind::Mistral => "mistral",
            ProviderKind::DeepSeek => "deep_seek",
            ProviderKind::Ollama => "ollama",
            ProviderKind::LmStudio => "lm_studio",
            ProviderKind::OpenAiCompatible => "open_ai_compatible",
            ProviderKind::ClaudeCode => "claude_code",
            ProviderKind::CodexCli => "codex_cli",
            ProviderKind::GeminiCli => "gemini_cli",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    pub fn descriptor(self) -> ProviderDescriptor {
        use ProviderKind::*;
        let base = ProviderDescriptor {
            kind: self,
            name: "",
            vendor: "",
            auth_method: AuthMethod::ApiKey,
            default_billing: BillingMode::Metered,
            default_base_url: None,
            cli_binary: None,
            cli_install: None,
            cli_login: None,
            key_url: None,
            docs_url: "",
            is_local: false,
            allow_multiple: true,
            summary: "",
        };
        match self {
            ClaudeCode => ProviderDescriptor {
                name: "Claude Code",
                vendor: "anthropic",
                auth_method: AuthMethod::CliDelegated,
                default_billing: BillingMode::Subscription,
                cli_binary: Some("claude"),
                cli_install: Some("npm install -g @anthropic-ai/claude-code"),
                cli_login: Some("claude auth login"),
                docs_url: "https://docs.anthropic.com/en/docs/claude-code",
                allow_multiple: false,
                summary: "Use your Claude subscription through the official Claude Code CLI.",
                ..base
            },
            CodexCli => ProviderDescriptor {
                name: "Codex",
                vendor: "openai",
                auth_method: AuthMethod::CliDelegated,
                default_billing: BillingMode::Subscription,
                cli_binary: Some("codex"),
                cli_install: Some("npm install -g @openai/codex"),
                cli_login: Some("codex login"),
                docs_url: "https://developers.openai.com/codex/cli",
                allow_multiple: false,
                summary: "Use your ChatGPT plan through the official Codex CLI.",
                ..base
            },
            GeminiCli => ProviderDescriptor {
                name: "Gemini CLI",
                vendor: "google",
                auth_method: AuthMethod::CliDelegated,
                default_billing: BillingMode::Subscription,
                cli_binary: Some("gemini"),
                cli_install: Some("npm install -g @google/gemini-cli"),
                cli_login: Some("gemini"),
                docs_url: "https://github.com/google-gemini/gemini-cli",
                allow_multiple: false,
                summary: "Use your Google account through the official Gemini CLI.",
                ..base
            },
            Anthropic => ProviderDescriptor {
                name: "Anthropic API",
                vendor: "anthropic",
                default_base_url: Some("https://api.anthropic.com"),
                key_url: Some("https://console.anthropic.com/settings/keys"),
                docs_url: "https://docs.anthropic.com/en/api",
                summary: "Metered access to Claude models with an Anthropic API key.",
                ..base
            },
            OpenAi => ProviderDescriptor {
                name: "OpenAI API",
                vendor: "openai",
                default_base_url: Some("https://api.openai.com/v1"),
                key_url: Some("https://platform.openai.com/api-keys"),
                docs_url: "https://platform.openai.com/docs/api-reference",
                summary: "Metered access to OpenAI models with an API key.",
                ..base
            },
            Gemini => ProviderDescriptor {
                name: "Gemini API",
                vendor: "google",
                default_base_url: Some("https://generativelanguage.googleapis.com"),
                key_url: Some("https://aistudio.google.com/apikey"),
                docs_url: "https://ai.google.dev/gemini-api/docs",
                default_billing: BillingMode::Unknown,
                summary: "Gemini models with a Google AI Studio API key.",
                ..base
            },
            OpenRouter => ProviderDescriptor {
                name: "OpenRouter",
                vendor: "openrouter",
                default_base_url: Some("https://openrouter.ai/api/v1"),
                default_billing: BillingMode::Credits,
                key_url: Some("https://openrouter.ai/settings/keys"),
                docs_url: "https://openrouter.ai/docs",
                summary: "Hundreds of models through one prepaid OpenRouter key.",
                ..base
            },
            Groq => ProviderDescriptor {
                name: "Groq",
                vendor: "groq",
                default_base_url: Some("https://api.groq.com/openai/v1"),
                key_url: Some("https://console.groq.com/keys"),
                docs_url: "https://console.groq.com/docs",
                summary: "Low-latency inference for open models.",
                ..base
            },
            Mistral => ProviderDescriptor {
                name: "Mistral",
                vendor: "mistral",
                default_base_url: Some("https://api.mistral.ai/v1"),
                key_url: Some("https://console.mistral.ai/api-keys"),
                docs_url: "https://docs.mistral.ai",
                summary: "Mistral models with a La Plateforme API key.",
                ..base
            },
            DeepSeek => ProviderDescriptor {
                name: "DeepSeek",
                vendor: "deepseek",
                default_base_url: Some("https://api.deepseek.com/v1"),
                key_url: Some("https://platform.deepseek.com/api_keys"),
                docs_url: "https://api-docs.deepseek.com",
                summary: "DeepSeek models with an API key.",
                ..base
            },
            Ollama => ProviderDescriptor {
                name: "Ollama",
                vendor: "ollama",
                auth_method: AuthMethod::None,
                default_billing: BillingMode::Local,
                default_base_url: Some("http://127.0.0.1:11434/v1"),
                cli_binary: Some("ollama"),
                docs_url: "https://ollama.com",
                is_local: true,
                summary: "Models running locally through Ollama.",
                ..base
            },
            LmStudio => ProviderDescriptor {
                name: "LM Studio",
                vendor: "lmstudio",
                auth_method: AuthMethod::None,
                default_billing: BillingMode::Local,
                default_base_url: Some("http://127.0.0.1:1234/v1"),
                docs_url: "https://lmstudio.ai/docs",
                is_local: true,
                summary: "Models served locally by LM Studio.",
                ..base
            },
            OpenAiCompatible => ProviderDescriptor {
                name: "OpenAI-compatible",
                vendor: "generic",
                default_billing: BillingMode::Unknown,
                docs_url: "https://platform.openai.com/docs/api-reference/chat",
                summary: "Any endpoint implementing the chat-completions protocol.",
                ..base
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Connected,
    /// Authentication is required or has expired.
    NeedsAuth,
    /// A local dependency (CLI, local server) is missing or unreachable.
    Unavailable,
    Error,
    Disabled,
    /// Verification has not run yet.
    Pending,
}

/// A configured provider connection. Secrets are never part of this struct;
/// they live in the OS credential store keyed by `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub kind: ProviderKind,
    pub label: String,
    pub auth_method: AuthMethod,
    pub billing_mode: BillingMode,
    /// Whether billing mode was reported by the provider or assumed.
    pub billing_reported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Account identity (email, org), when the provider reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// Subscription plan name, when reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    pub status: ConnectionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_verified_at: Option<Timestamp>,
    pub created_at: Timestamp,
    /// Whether a secret is stored for this account.
    pub has_secret: bool,
    /// Where the secret is stored ("keyring" or "file").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_store: Option<String>,
    /// Kind-specific options (e.g. CLI path override, extra headers).
    #[serde(default)]
    pub options: serde_json::Value,
}

impl Account {
    pub fn descriptor(&self) -> ProviderDescriptor {
        self.kind.descriptor()
    }

    pub fn base_url(&self) -> Option<String> {
        self.base_url
            .clone()
            .or_else(|| self.kind.descriptor().default_base_url.map(str::to_string))
    }

    pub fn is_usable(&self) -> bool {
        self.enabled && self.status == ConnectionStatus::Connected
    }
}

/// Result of verifying a connection against the provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerifiedIdentity {
    pub identity: Option<String>,
    pub plan: Option<String>,
    pub billing_mode: Option<BillingMode>,
    pub detail: Option<String>,
}
