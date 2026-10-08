//! Background monitoring. Polling is adaptive: provider limit endpoints are
//! queried only for adapters that expose allowance-free endpoints, more often
//! while an account is in active use, and with exponential backoff on
//! failure.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use crate::Harness;

const ACTIVE_WINDOW: Duration = Duration::from_secs(600);
const ACTIVE_INTERVAL: Duration = Duration::from_secs(120);
const IDLE_INTERVAL: Duration = Duration::from_secs(900);
const MAX_BACKOFF: Duration = Duration::from_secs(3600);
const MIN_GAP: Duration = Duration::from_secs(20);
const REVERIFY_INTERVAL: Duration = Duration::from_secs(12 * 3600);

struct PollState {
    last: Option<Instant>,
    backoff: Option<Duration>,
}

pub(crate) fn spawn(h: Arc<Harness>, mut poke_rx: mpsc::UnboundedReceiver<String>) {
    // Startup: verify connections and discover models concurrently.
    let h2 = h.clone();
    tokio::spawn(async move {
        let ids: Vec<String> = h2.list_accounts().into_iter().filter(|a| a.enabled).map(|a| a.id).collect();
        let futs = ids.iter().map(|id| {
            let h = h2.clone();
            let id = id.clone();
            async move {
                let _ = h.verify_account(&id).await;
                let stale = {
                    let models = h.models.read();
                    let newest = models.iter().filter(|(a, _, _)| *a == id).map(|(_, _, t)| *t).max();
                    newest.map(|t| magpie_core::now() - t > chrono::Duration::hours(12)).unwrap_or(true)
                };
                if stale {
                    if let Err(e) = h.refresh_models(&id).await {
                        tracing::debug!(account = %id, error = %e, "model refresh failed");
                    }
                }
                let _ = h.poke_limits.send(id);
            }
        });
        futures::future::join_all(futs).await;
    });

    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut polls: HashMap<String, PollState> = HashMap::new();
        let mut pending: HashMap<String, Instant> = HashMap::new();
        let mut last_history = Instant::now();
        let mut last_retention: Option<Instant> = None;
        let mut last_reverify = Instant::now();
        loop {
            tokio::select! {
                _ = h.shutdown.cancelled() => return,
                Some(id) = poke_rx.recv() => {
                    // Debounce: poll a few seconds after activity settles.
                    pending.insert(id, Instant::now() + Duration::from_secs(3));
                    continue;
                }
                _ = tick.tick() => {}
            }

            h.sweep_limits();
            for account in h.list_accounts().into_iter().filter(|a| a.kind == magpie_core::ProviderKind::ClaudeCode && a.enabled) {
                h.import_claude_usage(&account.id);
            }

            if last_history.elapsed() >= Duration::from_secs(60) {
                h.refresh_history();
                last_history = Instant::now();
            }

            if last_retention.map(|t| t.elapsed() >= Duration::from_secs(3600)).unwrap_or(true) {
                let days = h.settings.read().analytics.retention_days;
                match h.store.apply_retention(days) {
                    Ok(n) if n > 0 => tracing::info!(removed = n, "applied history retention"),
                    Err(e) => tracing::warn!(error = %e, "retention failed"),
                    _ => {}
                }
                last_retention = Some(Instant::now());
            }

            if last_reverify.elapsed() >= REVERIFY_INTERVAL {
                last_reverify = Instant::now();
                for a in h.list_accounts().into_iter().filter(|a| a.enabled) {
                    let _ = h.verify_account(&a.id).await;
                    let _ = h.refresh_models(&a.id).await;
                }
            }

            if !h.settings.read().analytics.poll_provider_limits {
                pending.clear();
                continue;
            }
            let now = Instant::now();
            for (account, adapter) in h.all_adapters() {
                if !adapter.supports_limit_polling() {
                    continue;
                }
                let state = polls.entry(account.id.clone()).or_insert(PollState { last: None, backoff: None });
                // Codex account reports include activity in other clients. A
                // quiet harness does not imply that this account is idle.
                let active = account.kind == magpie_core::ProviderKind::CodexCli
                    || h.last_activity.read().get(&account.id).map(|t| t.elapsed() < ACTIVE_WINDOW).unwrap_or(false);
                let interval = state.backoff.unwrap_or(if active { ACTIVE_INTERVAL } else { IDLE_INTERVAL });
                let poked = pending.get(&account.id).map(|due| *due <= now).unwrap_or(false);
                let due = match state.last {
                    None => true,
                    Some(last) => last.elapsed() >= interval || (poked && last.elapsed() >= MIN_GAP),
                };
                if !due {
                    continue;
                }
                pending.remove(&account.id);
                state.last = Some(now);
                match h.refresh_limits(&account.id).await {
                    Ok(_) => state.backoff = None,
                    Err(e) => {
                        let next = state.backoff.map(|b| (b * 2).min(MAX_BACKOFF)).unwrap_or(Duration::from_secs(240));
                        tracing::debug!(account = %account.id, error = %e, backoff_secs = next.as_secs(), "limit poll failed");
                        state.backoff = Some(next);
                    }
                }
            }
            polls.retain(|id, _| h.accounts.read().contains_key(id));
        }
    });
}
