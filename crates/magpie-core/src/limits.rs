use serde::{Deserialize, Serialize};

use crate::{Provenance, Timestamp};

/// The dimension a limit window measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitMetric {
    Requests,
    Tokens,
    InputTokens,
    OutputTokens,
    /// Opaque subscription usage expressed as a percentage.
    UsagePercent,
    /// Prepaid credit balance in USD.
    Credits,
    /// Monetary spend against a cap in USD.
    Spend,
}

/// One provider limit window, e.g. "requests per minute" or a weekly
/// subscription window. Every field is optional because providers expose
/// wildly different subsets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitWindow {
    pub account_id: String,
    /// Stable key within the account, e.g. `requests`, `codex.primary`.
    pub key: String,
    pub label: String,
    pub metric: LimitMetric,
    /// Model scope, when the limit only applies to specific models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<f64>,
    /// Amount consumed, when reported without a limit (e.g. spend).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_percent: Option<f64>,
    /// Window length in seconds for rolling/fixed windows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<Timestamp>,
    pub provenance: Provenance,
    /// Set when the provider told us the limit was hit.
    #[serde(default)]
    pub exhausted: bool,
    /// Qualitative provider warning; does not imply a known percentage.
    #[serde(default)]
    pub approaching: bool,
    pub observed_at: Timestamp,
}

/// User-facing limit status. Rendered with icons and labels, never colour
/// alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitState {
    Unknown,
    Available,
    Approaching,
    Limited,
    Exhausted,
    /// Exhausted but the known reset time has passed; awaiting confirmation.
    ResetPending,
}

impl LimitWindow {
    /// Fraction used in [0,1], when it can be determined.
    pub fn used_fraction(&self) -> Option<f64> {
        if let Some(p) = self.used_percent {
            return Some((p / 100.0).clamp(0.0, 1.0));
        }
        match (self.limit, self.remaining) {
            (Some(limit), Some(rem)) if limit > 0.0 => Some(((limit - rem) / limit).clamp(0.0, 1.0)),
            _ => None,
        }
    }

    /// Fraction remaining in [0,1], when it can be determined.
    pub fn remaining_fraction(&self) -> Option<f64> {
        self.used_fraction().map(|u| 1.0 - u)
    }

    pub fn state_at(&self, now: Timestamp, approaching_threshold: f64) -> LimitState {
        let reset_passed = self.resets_at.map(|r| r <= now).unwrap_or(false);
        let exhausted = self.exhausted || self.remaining == Some(0.0) || self.used_fraction() == Some(1.0);
        if exhausted {
            return if reset_passed { LimitState::ResetPending } else { LimitState::Exhausted };
        }
        if self.approaching && !reset_passed && self.used_fraction().is_none() {
            return LimitState::Approaching;
        }
        match self.used_fraction() {
            Some(_) if reset_passed => LimitState::Available,
            Some(u) if u >= 0.95 => LimitState::Limited,
            Some(u) if u >= approaching_threshold => LimitState::Approaching,
            Some(_) => LimitState::Available,
            None => match self.metric {
                LimitMetric::Credits => match self.remaining {
                    Some(r) if r <= 0.0 => LimitState::Exhausted,
                    Some(_) => LimitState::Available,
                    None => LimitState::Unknown,
                },
                _ => LimitState::Unknown,
            },
        }
    }

    /// Whether this window currently blocks execution.
    pub fn blocks(&self, now: Timestamp) -> bool {
        matches!(self.state_at(now, 0.8), LimitState::Exhausted)
    }
}

/// Aggregated limit status for an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountLimits {
    pub account_id: String,
    pub state: LimitState,
    pub windows: Vec<LimitWindow>,
    /// Earliest known future reset across windows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_reset: Option<Timestamp>,
}

impl AccountLimits {
    pub fn from_windows(account_id: String, windows: Vec<LimitWindow>, now: Timestamp) -> Self {
        let state = windows.iter().map(|w| w.state_at(now, 0.8)).filter(|s| *s != LimitState::Unknown).max().unwrap_or(LimitState::Unknown);
        let next_reset = windows.iter().filter_map(|w| w.resets_at).filter(|r| *r > now).min();
        Self { account_id, state, windows, next_reset }
    }
}

impl LimitWindow {
    /// A window with only identifying fields set.
    pub fn new(account_id: &str, key: &str, label: &str, metric: LimitMetric, provenance: Provenance) -> Self {
        Self {
            account_id: account_id.to_string(),
            key: key.to_string(),
            label: label.to_string(),
            metric,
            model_scope: None,
            limit: None,
            remaining: None,
            used: None,
            used_percent: None,
            window_secs: None,
            resets_at: None,
            provenance,
            exhausted: false,
            approaching: false,
            observed_at: crate::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn window(limit: Option<f64>, remaining: Option<f64>, pct: Option<f64>) -> LimitWindow {
        LimitWindow {
            account_id: "a".into(),
            key: "k".into(),
            label: "k".into(),
            metric: LimitMetric::Requests,
            model_scope: None,
            limit,
            remaining,
            used: None,
            used_percent: pct,
            window_secs: None,
            resets_at: None,
            provenance: Provenance::Reported,
            exhausted: false,
            approaching: false,
            observed_at: crate::now(),
        }
    }

    #[test]
    fn states() {
        let now = crate::now();
        assert_eq!(window(Some(100.0), Some(90.0), None).state_at(now, 0.8), LimitState::Available);
        assert_eq!(window(Some(100.0), Some(15.0), None).state_at(now, 0.8), LimitState::Approaching);
        assert_eq!(window(Some(100.0), Some(2.0), None).state_at(now, 0.8), LimitState::Limited);
        assert_eq!(window(Some(100.0), Some(0.0), None).state_at(now, 0.8), LimitState::Exhausted);
        assert_eq!(window(None, None, None).state_at(now, 0.8), LimitState::Unknown);
        assert_eq!(window(None, None, Some(50.0)).state_at(now, 0.8), LimitState::Available);
    }

    #[test]
    fn reset_pending_after_reset_time() {
        let now = crate::now();
        let mut w = window(Some(10.0), Some(0.0), None);
        w.resets_at = Some(now - Duration::seconds(5));
        assert_eq!(w.state_at(now, 0.8), LimitState::ResetPending);
        assert!(!w.blocks(now));
    }

    #[test]
    fn aggregate_takes_worst_known_state() {
        let now = crate::now();
        let a = AccountLimits::from_windows("a".into(), vec![window(None, None, None), window(Some(100.0), Some(10.0), None)], now);
        assert_eq!(a.state, LimitState::Approaching);
    }
}
