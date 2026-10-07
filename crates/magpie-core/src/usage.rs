use serde::{Deserialize, Serialize};

/// Where a number came from. Every user-visible metric carries one of these
/// so estimates are never presented as provider measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    /// Obtained directly from an authoritative provider response/endpoint.
    Reported,
    /// Computed locally from known token counts, prices or telemetry.
    Calculated,
    /// Inferred from incomplete data.
    Estimated,
    /// No data is available.
    Unavailable,
}

impl Provenance {
    /// Combine two provenances, keeping the weakest.
    pub fn weakest(self, other: Provenance) -> Provenance {
        fn rank(p: Provenance) -> u8 {
            match p {
                Provenance::Reported => 3,
                Provenance::Calculated => 2,
                Provenance::Estimated => 1,
                Provenance::Unavailable => 0,
            }
        }
        if rank(self) <= rank(other) {
            self
        } else {
            other
        }
    }
}

/// Token usage for one execution attempt, as reported by the provider.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Input tokens served from a prompt cache (subset of `input_tokens`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    /// Input tokens written to a prompt cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    /// Reasoning tokens (subset of `output_tokens` where reported).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
    /// Whether token counts came from the provider (`Reported`) or were
    /// estimated locally from text length (`Estimated`).
    pub provenance: Provenance,
}

impl Default for Provenance {
    fn default() -> Self {
        Provenance::Unavailable
    }
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input_tokens.unwrap_or(0) + self.output_tokens.unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none() && self.output_tokens.is_none()
    }

    /// Merge another usage report, preferring present values from `other`.
    pub fn merge(&mut self, other: &TokenUsage) {
        if other.input_tokens.is_some() {
            self.input_tokens = other.input_tokens;
        }
        if other.output_tokens.is_some() {
            self.output_tokens = other.output_tokens;
        }
        if other.cached_input_tokens.is_some() {
            self.cached_input_tokens = other.cached_input_tokens;
        }
        if other.cache_write_tokens.is_some() {
            self.cache_write_tokens = other.cache_write_tokens;
        }
        if other.reasoning_tokens.is_some() {
            self.reasoning_tokens = other.reasoning_tokens;
        }
        if other.provenance != Provenance::Unavailable {
            self.provenance = other.provenance;
        }
    }
}

/// Monetary cost with explicit provenance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub usd: f64,
    pub provenance: Provenance,
    /// True when the account is subscription-backed and the figure is an
    /// API-equivalent value rather than an actual charge.
    #[serde(default)]
    pub api_equivalent: bool,
}

/// Rough token estimate from text length. Used for routing decisions and
/// clearly marked as `Estimated` wherever it is surfaced.
pub fn estimate_tokens(text: &str) -> u64 {
    // ~4 characters per token for English prose and code; never zero for
    // non-empty input.
    let chars = text.chars().count() as u64;
    if chars == 0 {
        0
    } else {
        (chars / 4).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weakest_provenance() {
        assert_eq!(Provenance::Reported.weakest(Provenance::Estimated), Provenance::Estimated);
        assert_eq!(Provenance::Calculated.weakest(Provenance::Reported), Provenance::Calculated);
    }

    #[test]
    fn token_estimate() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("hi"), 1);
        assert_eq!(estimate_tokens(&"a".repeat(400)), 100);
    }
}

/// Account-wide usage reported by a provider (covers usage outside the
/// harness, unlike the local execution log).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderUsageReport {
    pub account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifetime_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_daily_tokens: Option<u64>,
    /// Daily token totals, oldest first. Dates are `YYYY-MM-DD`.
    pub daily: Vec<DailyTokens>,
    pub provenance: Provenance,
    pub observed_at: crate::Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyTokens {
    pub date: String,
    pub tokens: u64,
}
