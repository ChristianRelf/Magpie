//! Only the documented Claude Code status-line payload is accepted here.
//! No Claude credentials, transcripts, or private endpoints are accessed.

use std::{io::Read, path::Path};

use chrono::TimeZone;
use magpie_core::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SNAPSHOT_FILE: &str = "claude-usage.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub account_id: String,
    pub observed_at: Timestamp,
    pub rate_limits: Windows,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Windows {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<Window>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_percentage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
}

fn window(v: &Value) -> Option<Window> {
    let used_percentage = v["used_percentage"].as_f64().filter(|n| n.is_finite() && (0.0..=100.0).contains(n));
    let resets_at = v["resets_at"].as_i64().filter(|t| *t > 0 && chrono::Utc.timestamp_opt(*t, 0).single().is_some());
    (used_percentage.is_some() || resets_at.is_some()).then_some(Window { used_percentage, resets_at })
}

impl UsageSnapshot {
    /// Whitelist quota fields; never persist the rest of the status-line input.
    pub fn from_statusline(account_id: &str, input: &Value) -> Option<Self> {
        let rate_limits =
            Windows { five_hour: window(&input["rate_limits"]["five_hour"]), seven_day: window(&input["rate_limits"]["seven_day"]) };
        (rate_limits.five_hour.is_some() || rate_limits.seven_day.is_some()).then(|| Self {
            account_id: account_id.to_string(),
            observed_at: now(),
            rate_limits,
        })
    }

    pub fn limits(&self) -> Vec<LimitWindow> {
        let mut result = Vec::new();
        for (kind, label, secs, entry) in [
            ("five_hour", "5-hour session", 5 * 3600, &self.rate_limits.five_hour),
            ("seven_day", "Weekly", 7 * 86_400, &self.rate_limits.seven_day),
        ] {
            let Some(entry) = entry else { continue };
            let mut w =
                LimitWindow::new(&self.account_id, &format!("claude.{kind}"), label, LimitMetric::UsagePercent, Provenance::Reported);
            w.used_percent = entry.used_percentage.filter(|n| n.is_finite() && (0.0..=100.0).contains(n));
            w.resets_at = entry.resets_at.and_then(|t| chrono::Utc.timestamp_opt(t, 0).single());
            w.window_secs = Some(secs);
            w.exhausted = w.used_percent == Some(100.0);
            w.observed_at = self.observed_at;
            if w.used_percent.is_some() || w.resets_at.is_some() {
                result.push(w);
            }
        }
        result
    }
}

pub fn read_snapshot(root: &Path, account_id: &str) -> Option<UsageSnapshot> {
    let file = std::fs::File::open(root.join(SNAPSHOT_FILE)).ok()?;
    if file.metadata().ok()?.len() > 16 * 1024 {
        return None;
    }
    let mut data = String::new();
    file.take(16 * 1024 + 1).read_to_string(&mut data).ok()?;
    let snapshot: UsageSnapshot = serde_json::from_str(&data).ok()?;
    let age = now() - snapshot.observed_at;
    (snapshot.account_id == account_id && age >= chrono::Duration::minutes(-1) && age < chrono::Duration::days(8)).then_some(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn statusline_percentages_are_not_fractions_and_unrelated_fields_are_discarded() {
        let input = json!({"rate_limits": {
            "five_hour": {"used_percentage": 0.5, "resets_at": 1900000000},
            "seven_day": {"used_percentage": 0, "resets_at": 1900400000}
        }, "transcript_path": "/private/session.jsonl", "secret": "never-retain"});
        let snapshot = UsageSnapshot::from_statusline("account", &input).unwrap();
        let limits = snapshot.limits();
        assert_eq!(limits[0].used_percent, Some(0.5));
        assert_eq!(limits[1].used_percent, Some(0.0));
        assert_eq!(limits[0].resets_at.unwrap().timestamp(), 1900000000);
        assert_eq!(limits[1].window_secs, Some(604800));
        assert!(limits.iter().all(|w| w.provenance == Provenance::Reported));
        let stored = serde_json::to_string(&snapshot).unwrap();
        assert!(!stored.contains("private") && !stored.contains("secret"));
    }

    #[test]
    fn absent_or_invalid_allowances_stay_unknown() {
        assert!(UsageSnapshot::from_statusline("a", &json!({"rate_limits": null})).is_none());
        assert!(UsageSnapshot::from_statusline("a", &json!({"rate_limits":{"five_hour":{"used_percentage":101,"resets_at":-1}}})).is_none());
        let snapshot = UsageSnapshot::from_statusline("a", &json!({"rate_limits":{"five_hour":{"resets_at":1900000000}}})).unwrap();
        assert_eq!(snapshot.limits()[0].used_percent, None);
    }
}
