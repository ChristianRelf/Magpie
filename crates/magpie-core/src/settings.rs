use serde::{Deserialize, Serialize};

/// All user settings. Persisted as JSON in the database; every field has a
/// default so older stored settings deserialize after upgrades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Settings {
    pub general: GeneralSettings,
    pub analytics: AnalyticsSettings,
    pub security: SecuritySettings,
    pub notifications: NotificationSettings,
    pub server: ServerSettings,
    /// Set once the first-run flow has been completed.
    pub onboarding_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// Start the harness when the user logs in.
    pub launch_on_startup: bool,
    /// Closing the window hides it to the tray instead of quitting.
    pub minimise_to_tray: bool,
    /// Keep the harness running after the desktop app quits.
    pub keep_harness_running: bool,
    pub automatic_updates: bool,
    pub theme: ThemePreference,
    /// Force reduced motion regardless of OS preference.
    pub reduced_motion: bool,
    pub sidebar_collapsed: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_on_startup: false,
            minimise_to_tray: false,
            keep_harness_running: false,
            automatic_updates: true,
            theme: ThemePreference::Dark,
            reduced_motion: false,
            sidebar_collapsed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalyticsSettings {
    /// Days of execution history to keep. 0 keeps everything.
    pub retention_days: u32,
    /// UI refresh interval for live telemetry, in seconds.
    pub refresh_interval_secs: u32,
    /// Keep a local execution history at all.
    pub local_history: bool,
    /// Poll provider usage endpoints (e.g. Codex limits). Polling is
    /// adaptive and never consumes model allowance.
    pub poll_provider_limits: bool,
    /// Daily spend threshold in USD that triggers a notification.
    pub spend_alert_usd: Option<f64>,
}

impl Default for AnalyticsSettings {
    fn default() -> Self {
        Self {
            retention_days: 90,
            refresh_interval_secs: 5,
            local_history: true,
            poll_provider_limits: true,
            spend_alert_usd: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SecuritySettings {
    /// Store request/response content with execution records.
    pub retain_request_content: bool,
    /// Verbose diagnostic logging (still redacted).
    pub diagnostic_logging: bool,
    /// Require an API key for all non-health endpoints. Disabling is not
    /// offered; this flag only controls whether read-only endpoints accept
    /// any valid client key or require the `read` scope.
    pub require_scopes: bool,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self { retain_request_content: false, diagnostic_logging: false, require_scopes: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub provider_disconnected: bool,
    pub auth_expired: bool,
    pub allowance_exhausted: bool,
    pub limit_approaching: bool,
    pub allowance_reset: bool,
    pub fallback_activated: bool,
    pub harness_stopped: bool,
    pub spend_threshold: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            provider_disconnected: true,
            auth_expired: true,
            allowance_exhausted: true,
            limit_approaching: true,
            allowance_reset: false,
            fallback_activated: false,
            harness_stopped: true,
            spend_threshold: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub port: u16,
    /// Bind address. Only loopback addresses are accepted unless
    /// `allow_network` is set.
    pub bind: String,
    pub allow_network: bool,
    /// Per-attempt timeout in seconds.
    pub request_timeout_secs: u64,
    /// Max concurrent executions.
    pub max_concurrency: usize,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            port: 7878,
            bind: "127.0.0.1".into(),
            allow_network: false,
            request_timeout_secs: 600,
            max_concurrency: 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    ProviderDisconnected,
    AuthExpired,
    AllowanceExhausted,
    LimitApproaching,
    AllowanceReset,
    FallbackActivated,
    HarnessStopped,
    SpendThreshold,
}

impl NotificationSettings {
    pub fn allows(&self, kind: NotificationKind) -> bool {
        self.enabled
            && match kind {
                NotificationKind::ProviderDisconnected => self.provider_disconnected,
                NotificationKind::AuthExpired => self.auth_expired,
                NotificationKind::AllowanceExhausted => self.allowance_exhausted,
                NotificationKind::LimitApproaching => self.limit_approaching,
                NotificationKind::AllowanceReset => self.allowance_reset,
                NotificationKind::FallbackActivated => self.fallback_activated,
                NotificationKind::HarnessStopped => self.harness_stopped,
                NotificationKind::SpendThreshold => self.spend_threshold,
            }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub kind: NotificationKind,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    pub created_at: crate::Timestamp,
    pub read: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_settings_deserialize() {
        let s: Settings = serde_json::from_str(r#"{"general":{"theme":"light"}}"#).unwrap();
        assert_eq!(s.general.theme, ThemePreference::Light);
        assert_eq!(s.server.port, 7878);
        assert!(!s.security.retain_request_content);
    }
}
