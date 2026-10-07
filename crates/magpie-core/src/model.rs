use serde::{Deserialize, Serialize};

use crate::{BillingMode, Provenance, ProviderKind, Timestamp};

/// Capabilities a model is known to support. Capability negotiation in the
/// router compares a request's requirements against these flags; nothing is
/// assumed to be supported unless the provider or catalog says so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    pub streaming: bool,
    /// Client-defined function/tool calling.
    pub tools: bool,
    /// JSON-schema constrained output.
    pub structured_output: bool,
    /// Image inputs.
    pub vision: bool,
    /// Native reasoning / extended thinking.
    pub reasoning: bool,
    /// The model runs as an agent with its own tool loop (CLI agents).
    pub agentic: bool,
    /// System prompts are supported.
    pub system_prompt: bool,
}

impl Capabilities {
    pub fn satisfies(&self, req: &Capabilities) -> bool {
        (!req.streaming || self.streaming)
            && (!req.tools || self.tools)
            && (!req.structured_output || self.structured_output)
            && (!req.vision || self.vision)
            && (!req.reasoning || self.reasoning)
            && (!req.agentic || self.agentic)
    }

    pub fn missing(&self, req: &Capabilities) -> Vec<&'static str> {
        let mut out = Vec::new();
        if req.tools && !self.tools {
            out.push("tool calling");
        }
        if req.structured_output && !self.structured_output {
            out.push("structured output");
        }
        if req.vision && !self.vision {
            out.push("image input");
        }
        if req.reasoning && !self.reasoning {
            out.push("reasoning");
        }
        if req.agentic && !self.agentic {
            out.push("agentic execution");
        }
        if req.streaming && !self.streaming {
            out.push("streaming");
        }
        out
    }
}

/// Coarse expected-quality tier used by the router. Derived from provider
/// metadata where available and from the built-in catalog otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityTier {
    Light = 1,
    Standard = 2,
    High = 3,
    Frontier = 4,
}

impl QualityTier {
    pub fn score(self) -> f64 {
        match self {
            QualityTier::Light => 0.35,
            QualityTier::Standard => 0.6,
            QualityTier::High => 0.8,
            QualityTier::Frontier => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedClass {
    Slow = 1,
    Medium = 2,
    Fast = 3,
}

impl SpeedClass {
    pub fn score(self) -> f64 {
        match self {
            SpeedClass::Slow => 0.3,
            SpeedClass::Medium => 0.6,
            SpeedClass::Fast => 1.0,
        }
    }
}

/// Per-million-token prices in USD.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_per_mtok: Option<f64>,
    /// `Reported` when the provider's API returned the price, `Calculated`
    /// when the user configured it, `Estimated` when taken from the built-in
    /// catalog (which may be stale).
    pub provenance: Provenance,
}

impl Pricing {
    pub fn cost(&self, input: u64, cached_input: u64, output: u64) -> f64 {
        let uncached = input.saturating_sub(cached_input) as f64;
        let cached_rate = self.cached_input_per_mtok.unwrap_or(self.input_per_mtok);
        (uncached * self.input_per_mtok + cached_input as f64 * cached_rate + output as f64 * self.output_per_mtok)
            / 1_000_000.0
    }
}

/// A model as reported by an adapter's discovery call, before user
/// preferences are applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredModel {
    /// Wire identifier passed to the provider.
    pub model_id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    pub capabilities: Capabilities,
    pub tier: QualityTier,
    pub speed: SpeedClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<Pricing>,
    /// Provenance of the model metadata (context window, capabilities).
    pub metadata_provenance: Provenance,
    /// Provider marks this as the default model for the account.
    #[serde(default)]
    pub is_default: bool,
    /// Supported reasoning effort levels, if the provider reports them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasoning_efforts: Vec<String>,
}

/// User-controlled model preferences.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelPreference {
    #[serde(default)]
    pub favourite: bool,
    #[serde(default)]
    pub disabled: bool,
    /// Higher priority models are preferred; 0 is neutral.
    #[serde(default)]
    pub priority: i32,
    /// User-provided price override (marks cost as Calculated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_override: Option<Pricing>,
    /// Percentage (0-100) of a known allowance reserved for complex tasks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reserve_percent: Option<f64>,
}

/// A model available to the harness: discovered metadata joined with its
/// account and user preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Stable harness-wide key: `<account_id>/<model_id>`.
    pub key: String,
    pub account_id: String,
    pub account_label: String,
    pub provider: ProviderKind,
    pub billing_mode: BillingMode,
    #[serde(flatten)]
    pub model: DiscoveredModel,
    pub preference: ModelPreference,
    /// Account connected, model enabled, and not blocked by a known limit.
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub discovered_at: Timestamp,
}

impl ModelInfo {
    pub fn make_key(account_id: &str, model_id: &str) -> String {
        format!("{account_id}/{model_id}")
    }

    /// Effective pricing, preferring user overrides.
    pub fn pricing(&self) -> Option<Pricing> {
        self.preference.price_override.or(self.model.pricing)
    }

    /// Public identifier exposed to API clients: `<provider>/<model_id>`.
    pub fn public_id(&self) -> String {
        format!("{}/{}", self.provider.as_str(), self.model.model_id)
    }
}
