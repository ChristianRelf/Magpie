//! The harness runtime: owns provider connections, executes requests with
//! routing and fallback, tracks usage and limits, and runs background
//! monitoring. It is independent of any UI; the local API and the desktop
//! app are clients of this crate.

mod accounts;
pub mod autostart;
mod background;
mod clients;
mod events;
mod exec;
mod limits;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use magpie_core::*;
use magpie_providers::SharedAdapter;
use magpie_security::{SecretBackend, SecretStore};
use magpie_store::{paths::Paths, Store};
use parking_lot::RwLock;
use serde::Serialize;
use tokio::sync::{broadcast, Semaphore};
use tokio_util::sync::CancellationToken;

pub use accounts::{AccountPatch, ConnectRequest};
pub use clients::{CreatedClient, Principal, Scope};
pub use events::{ExecutionSummary, HarnessEvent};
pub use exec::ExecutionHandle;
pub use magpie_providers::cli::CliStatus;

/// Builds provider adapters. Replaceable in tests.
pub type AdapterFactory = Arc<dyn Fn(Account, Option<String>) -> HarnessResult<SharedAdapter> + Send + Sync>;

pub struct HarnessOptions {
    pub paths: Paths,
    pub secrets: Arc<dyn SecretStore>,
    pub secret_backend: SecretBackend,
    pub adapter_factory: Option<AdapterFactory>,
    /// Command used to launch this harness, for start-on-login registration.
    pub launch_command: Option<(PathBuf, Vec<String>)>,
    /// Disable background tasks (tests).
    pub background: bool,
}

pub(crate) struct AccountEntry {
    pub account: Account,
    pub adapter: Option<SharedAdapter>,
}

pub(crate) struct ActiveExecution {
    pub cancel: CancellationToken,
    pub summary: ExecutionSummary,
}

pub struct Harness {
    pub store: Arc<Store>,
    pub paths: Paths,
    pub(crate) secrets: Arc<dyn SecretStore>,
    pub(crate) secret_backend: SecretBackend,
    pub(crate) factory: AdapterFactory,
    pub(crate) accounts: RwLock<HashMap<String, AccountEntry>>,
    pub(crate) models: RwLock<Vec<(String, DiscoveredModel, Timestamp)>>,
    pub(crate) prefs: RwLock<HashMap<String, ModelPreference>>,
    /// Session-level model blocks after model-not-found/permission errors.
    pub(crate) blocked_models: RwLock<HashMap<String, (Timestamp, String)>>,
    pub(crate) limits: RwLock<HashMap<String, Vec<LimitWindow>>>,
    pub(crate) usage_reports: RwLock<HashMap<String, ProviderUsageReport>>,
    pub(crate) history: RwLock<HashMap<String, magpie_router::ModelHistory>>,
    pub(crate) settings: RwLock<Settings>,
    pub(crate) routing: RwLock<RoutingConfig>,
    pub(crate) active: RwLock<HashMap<String, ActiveExecution>>,
    pub(crate) notified: RwLock<HashSet<String>>,
    pub(crate) last_activity: RwLock<HashMap<String, Instant>>,
    pub(crate) events: broadcast::Sender<HarnessEvent>,
    pub(crate) semaphore: Arc<Semaphore>,
    pub(crate) admin_token_hash: String,
    pub(crate) launch_command: Option<(PathBuf, Vec<String>)>,
    pub(crate) poke_limits: tokio::sync::mpsc::UnboundedSender<String>,
    pub shutdown: CancellationToken,
    pub started_at: Timestamp,
}

#[derive(Debug, Clone, Serialize)]
pub struct HarnessStatus {
    pub product: &'static str,
    pub version: &'static str,
    pub api_version: u32,
    pub started_at: Timestamp,
    pub uptime_secs: i64,
    pub accounts: usize,
    pub connected_accounts: usize,
    pub models: usize,
    pub available_models: usize,
    pub active_executions: usize,
    pub data_dir: String,
    pub secret_store: &'static str,
    pub pid: u32,
}

impl Harness {
    /// Open the harness: database, settings, admin token and accounts.
    pub async fn open(opts: HarnessOptions) -> HarnessResult<Arc<Self>> {
        opts.paths.ensure().map_err(|e| HarnessError::internal(format!("cannot create data directory: {e}")))?;
        let store = Arc::new(Store::open(&opts.paths.database())?);
        let recovered = store.recover_interrupted()?;
        if recovered > 0 {
            tracing::warn!(recovered, "marked interrupted executions as failed");
        }
        let settings = store.settings()?;
        let routing = store.routing_config()?;
        let admin_token_hash = clients::ensure_admin_token(&opts.paths)?;
        let (events, _) = broadcast::channel(512);
        let (poke_tx, poke_rx) = tokio::sync::mpsc::unbounded_channel();
        let factory = opts.adapter_factory.unwrap_or_else(|| Arc::new(magpie_providers::build_adapter));
        let max = settings.server.max_concurrency.max(1);

        let mut limits: HashMap<String, Vec<LimitWindow>> = HashMap::new();
        for w in store.list_limits()? {
            limits.entry(w.account_id.clone()).or_default().push(w);
        }

        let harness = Arc::new(Self {
            paths: opts.paths,
            secrets: opts.secrets,
            secret_backend: opts.secret_backend,
            factory,
            accounts: RwLock::new(HashMap::new()),
            models: RwLock::new(store.list_models()?),
            prefs: RwLock::new(store.model_prefs()?),
            blocked_models: RwLock::new(HashMap::new()),
            limits: RwLock::new(limits),
            usage_reports: RwLock::new(HashMap::new()),
            history: RwLock::new(HashMap::new()),
            settings: RwLock::new(settings),
            routing: RwLock::new(routing),
            active: RwLock::new(HashMap::new()),
            notified: RwLock::new(HashSet::new()),
            last_activity: RwLock::new(HashMap::new()),
            events,
            semaphore: Arc::new(Semaphore::new(max)),
            admin_token_hash,
            launch_command: opts.launch_command,
            poke_limits: poke_tx,
            shutdown: CancellationToken::new(),
            started_at: now(),
            store,
        });
        harness.load_accounts().await?;
        harness.refresh_history();
        if opts.background {
            background::spawn(harness.clone(), poke_rx);
        }
        Ok(harness)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HarnessEvent> {
        self.events.subscribe()
    }

    pub(crate) fn emit(&self, ev: HarnessEvent) {
        let _ = self.events.send(ev);
    }

    pub fn status(&self) -> HarnessStatus {
        let accounts = self.accounts.read();
        let models = self.list_models();
        HarnessStatus {
            product: PRODUCT_NAME,
            version: VERSION,
            api_version: API_VERSION,
            started_at: self.started_at,
            uptime_secs: (now() - self.started_at).num_seconds(),
            accounts: accounts.len(),
            connected_accounts: accounts.values().filter(|a| a.account.is_usable()).count(),
            models: models.len(),
            available_models: models.iter().filter(|m| m.available && !m.preference.disabled).count(),
            active_executions: self.active.read().len(),
            data_dir: self.paths.root.display().to_string(),
            secret_store: self.secret_backend.as_str(),
            pid: std::process::id(),
        }
    }

    // ------------------------------------------------------------ settings

    pub fn settings(&self) -> Settings {
        self.settings.read().clone()
    }

    pub fn update_settings(&self, new: Settings) -> HarnessResult<Settings> {
        let old = self.settings();
        if new.server.bind != old.server.bind || new.server.allow_network != old.server.allow_network {
            validate_bind(&new.server.bind, new.server.allow_network)?;
        }
        if new.analytics.retention_days > 3650 {
            return Err(HarnessError::invalid("Retention must be at most 3650 days"));
        }
        if !(1..=3600).contains(&new.analytics.refresh_interval_secs) {
            return Err(HarnessError::invalid("Refresh interval must be between 1 and 3600 seconds"));
        }
        if new.server.port == 0
            || !(1..=3600).contains(&new.server.request_timeout_secs)
            || !(1..=128).contains(&new.server.max_concurrency)
        {
            return Err(HarnessError::invalid("Choose a non-zero port, a timeout of 1–3600 seconds, and 1–128 concurrent executions"));
        }
        if new.analytics.spend_alert_usd.is_some_and(|v| !v.is_finite() || v < 0.0) {
            return Err(HarnessError::invalid("Spend notification threshold must be non-negative"));
        }
        if old.general.launch_on_startup != new.general.launch_on_startup {
            self.apply_autostart(new.general.launch_on_startup)?;
        }
        if let Err(error) = self.store.save_settings(&new) {
            if old.general.launch_on_startup != new.general.launch_on_startup {
                let _ = self.apply_autostart(old.general.launch_on_startup);
            }
            return Err(error.into());
        }
        *self.settings.write() = new.clone();
        if old.security.retain_request_content && !new.security.retain_request_content {
            let purged = self.store.purge_content()?;
            tracing::info!(purged, "removed stored request content");
        }
        if old.analytics.retention_days != new.analytics.retention_days {
            let _ = self.store.apply_retention(new.analytics.retention_days);
        }
        self.emit(HarnessEvent::SettingsUpdated);
        Ok(new)
    }

    fn apply_autostart(&self, enabled: bool) -> HarnessResult<()> {
        match (&self.launch_command, enabled) {
            (_, false) => autostart::disable(),
            (Some((program, args)), true) => autostart::enable(program, args),
            (None, true) => Err(HarnessError::invalid("Start on login is unavailable for this launch mode")),
        }
    }

    pub fn autostart_enabled(&self) -> bool {
        autostart::is_enabled()
    }

    pub fn routing_config(&self) -> RoutingConfig {
        self.routing.read().clone()
    }

    pub fn update_routing(&self, cfg: RoutingConfig) -> HarnessResult<RoutingConfig> {
        if let Some(c) = cfg.max_cost_per_request_usd {
            if c < 0.0 {
                return Err(HarnessError::invalid("Maximum cost must be non-negative"));
            }
        }
        if let Some(p) = cfg.preserve_premium_percent {
            if !(0.0..=90.0).contains(&p) {
                return Err(HarnessError::invalid("Reserved allowance must be between 0% and 90%"));
            }
        }
        if cfg.preset == RoutingPreset::Manual && cfg.manual_model.is_none() {
            return Err(HarnessError::invalid("Choose a model for the Manual preset"));
        }
        self.store.save_routing_config(&cfg)?;
        *self.routing.write() = cfg.clone();
        self.emit(HarnessEvent::RoutingUpdated);
        Ok(cfg)
    }

    // --------------------------------------------------------- notifications

    pub(crate) fn notify(&self, kind: NotificationKind, title: impl Into<String>, body: impl Into<String>, account_id: Option<String>) {
        if !self.settings.read().notifications.allows(kind) {
            return;
        }
        let n =
            Notification { id: new_id("ntf"), kind, title: title.into(), body: body.into(), account_id, created_at: now(), read: false };
        if let Err(e) = self.store.insert_notification(&n) {
            tracing::warn!(error = %e, "could not store notification");
        }
        self.emit(HarnessEvent::Notification { notification: n });
    }

    /// Notify once per unique key (e.g. per limit window and reset period).
    pub(crate) fn notify_once(
        &self,
        key: String,
        kind: NotificationKind,
        title: impl Into<String>,
        body: impl Into<String>,
        account_id: Option<String>,
    ) {
        if self.notified.write().insert(key) {
            self.notify(kind, title, body, account_id);
        }
    }

    pub fn notifications(&self, limit: u32) -> HarnessResult<Vec<Notification>> {
        Ok(self.store.list_notifications(limit)?)
    }

    pub fn mark_notifications_read(&self) -> HarnessResult<()> {
        Ok(self.store.mark_notifications_read()?)
    }

    pub(crate) fn refresh_history(&self) {
        let since = (now() - chrono::Duration::days(7)).timestamp_millis();
        match self.store.model_stats(since) {
            Ok(stats) => {
                let map = stats
                    .into_iter()
                    .map(|s| {
                        (
                            s.model_key.clone(),
                            magpie_router::ModelHistory {
                                requests: s.requests,
                                failures: s.failures,
                                avg_duration_ms: s.avg_duration_ms,
                                output_tps: s.output_tps,
                            },
                        )
                    })
                    .collect();
                *self.history.write() = map;
            }
            Err(e) => tracing::warn!(error = %e, "could not load model history"),
        }
    }

    /// Stop background work and active executions, and release adapters.
    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        for a in self.active.read().values() {
            a.cancel.cancel();
        }
        let adapters: Vec<SharedAdapter> = self.accounts.read().values().filter_map(|e| e.adapter.clone()).collect();
        for a in adapters {
            a.shutdown().await;
        }
    }
}

/// Only loopback binds are permitted unless network access is explicitly
/// enabled.
pub fn validate_bind(bind: &str, allow_network: bool) -> HarnessResult<()> {
    let ip: std::net::IpAddr = bind.parse().map_err(|_| HarnessError::invalid(format!("Invalid bind address: {bind}")))?;
    if !ip.is_loopback() && !allow_network {
        return Err(HarnessError::invalid("Binding to a non-loopback address requires enabling network access"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
