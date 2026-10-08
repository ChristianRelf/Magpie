use magpie_core::*;

use crate::{Harness, HarnessEvent};

impl Harness {
    /// Read the opt-in local Claude status-line report. This never calls a
    /// provider or consumes allowance, including while provider polling is off.
    pub(crate) fn import_claude_usage(&self, account_id: &str) {
        if self.get_account(account_id).is_none_or(|a| a.kind != ProviderKind::ClaudeCode || !a.enabled) {
            return;
        }
        let Some(snapshot) = magpie_providers::cli::claude_usage::read_snapshot(&self.paths.root, account_id) else { return };
        let windows: Vec<_> = snapshot
            .limits()
            .into_iter()
            .filter(|w| {
                !self
                    .limits
                    .read()
                    .get(account_id)
                    .is_some_and(|existing| existing.iter().any(|e| e.key == w.key && e.observed_at >= w.observed_at))
            })
            .collect();
        self.apply_limits(account_id, windows);
    }

    /// Current limit windows for all accounts.
    pub fn limit_windows(&self) -> Vec<LimitWindow> {
        self.limits.read().values().flatten().cloned().collect()
    }

    /// Aggregated limit status per account (every connected account is
    /// listed, with `Unknown` when nothing is reported).
    pub fn account_limits(&self) -> Vec<AccountLimits> {
        let now = now();
        let limits = self.limits.read();
        self.list_accounts()
            .into_iter()
            .map(|a| AccountLimits::from_windows(a.id.clone(), limits.get(&a.id).cloned().unwrap_or_default(), now))
            .collect()
    }

    /// Merge windows into an account's limit state (upsert by key), persist
    /// them and raise notifications for state changes.
    pub(crate) fn apply_limits(&self, account_id: &str, windows: Vec<LimitWindow>) {
        if windows.is_empty() {
            return;
        }
        {
            let mut all = self.limits.write();
            let entry = all.entry(account_id.to_string()).or_default();
            for w in &windows {
                match entry.iter_mut().find(|e| e.key == w.key) {
                    Some(existing) => *existing = w.clone(),
                    None => entry.push(w.clone()),
                }
            }
            // Fresh provider data supersedes observed (inferred) blocks.
            if windows.iter().any(|w| !w.key.starts_with("observed.") && w.provenance == Provenance::Reported && !w.exhausted) {
                let any_exhausted = windows.iter().any(|w| w.exhausted);
                if !any_exhausted {
                    entry.retain(|e| !(e.key.starts_with("observed.") && e.model_scope.is_none()));
                }
            }
        }
        if let Err(e) = self.store.upsert_limits(&windows) {
            tracing::warn!(error = %e, "could not persist limits");
        }
        self.evaluate_limit_notifications(account_id, &windows);
        self.emit(HarnessEvent::LimitsUpdated { account_id: account_id.to_string() });
    }

    fn evaluate_limit_notifications(&self, account_id: &str, windows: &[LimitWindow]) {
        let label = self.get_account(account_id).map(|a| a.label).unwrap_or_else(|| account_id.to_string());
        let now = now();
        for w in windows {
            let period = w.resets_at.map(|r| r.timestamp().to_string()).unwrap_or_default();
            let reset_text = w.resets_at.map(|r| format!(" Resets {}.", r.format("%a %H:%M UTC"))).unwrap_or_default();
            match w.state_at(now, 0.8) {
                LimitState::Approaching | LimitState::Limited if w.provenance != Provenance::Unavailable => {
                    let pct = w.used_fraction().map(|u| format!("{:.0}%", u * 100.0)).unwrap_or_default();
                    self.notify_once(
                        format!("approach:{account_id}:{}:{period}", w.key),
                        NotificationKind::LimitApproaching,
                        format!("{label}: {} at {pct}", w.label),
                        format!("Approaching the {} limit.{reset_text}", w.label.to_lowercase()),
                        Some(account_id.to_string()),
                    );
                }
                LimitState::Exhausted => {
                    self.notify_once(
                        format!("exhaust:{account_id}:{}:{period}", w.key),
                        NotificationKind::AllowanceExhausted,
                        format!("{label}: {} reached", w.label),
                        format!("Requests will be routed elsewhere where possible.{reset_text}"),
                        Some(account_id.to_string()),
                    );
                }
                _ => {}
            }
        }
    }

    /// Record a limit the provider just reported through an error.
    pub(crate) fn observe_limit_error(&self, model: &SelectedModel, e: &HarnessError) {
        let now = now();
        let quota = e.kind == ErrorKind::QuotaExhausted;
        let (resets_at, provenance) = match (e.resets_at, e.retry_after_secs) {
            (Some(r), _) => (r, Provenance::Reported),
            (None, Some(s)) => (now + chrono::Duration::seconds(s.max(1) as i64), Provenance::Reported),
            (None, None) if quota => (now + chrono::Duration::hours(1), Provenance::Estimated),
            (None, None) => (now + chrono::Duration::seconds(60), Provenance::Estimated),
        };
        let mut w = LimitWindow::new(
            &model.account_id,
            if quota { "observed.quota" } else { "observed.rate" },
            if quota { "Usage allowance" } else { "Rate limit" },
            LimitMetric::UsagePercent,
            provenance,
        );
        if !quota {
            w.key = format!("observed.rate.{}", model.model_id);
            w.model_scope = Some(model.model_id.clone());
        }
        w.exhausted = true;
        w.resets_at = Some(resets_at);
        self.apply_limits(&model.account_id, vec![w]);
    }

    /// Drop expired observed windows and announce resets. Returns true when
    /// anything changed.
    pub(crate) fn sweep_limits(&self) -> bool {
        let now = now();
        let mut reset: Vec<(String, LimitWindow)> = Vec::new();
        {
            let mut all = self.limits.write();
            for (account, windows) in all.iter_mut() {
                for w in windows.iter() {
                    if w.exhausted && w.resets_at.map(|r| r <= now).unwrap_or(false) {
                        reset.push((account.clone(), w.clone()));
                    }
                }
                windows.retain(|w| !(w.key.starts_with("observed.") && w.resets_at.map(|r| r <= now).unwrap_or(true)));
                for w in windows.iter_mut() {
                    if w.exhausted && w.resets_at.map(|r| r <= now).unwrap_or(false) {
                        // The provider has not confirmed yet; state becomes
                        // reset-pending until the next poll or request.
                        w.exhausted = false;
                        w.used_percent = None;
                        w.remaining = None;
                        w.provenance = Provenance::Estimated;
                    }
                }
            }
        }
        for (account, w) in &reset {
            let label = self.get_account(account).map(|a| a.label).unwrap_or_default();
            self.notify_once(
                format!("reset:{account}:{}:{}", w.key, w.resets_at.map(|r| r.timestamp()).unwrap_or_default()),
                NotificationKind::AllowanceReset,
                format!("{label}: {} reset", w.label),
                "The allowance window has reset.",
                Some(account.clone()),
            );
            let current = self.limits.read().get(account).cloned().unwrap_or_default();
            let _ = self.store.clear_limits(account);
            let _ = self.store.upsert_limits(&current);
            self.emit(HarnessEvent::LimitsUpdated { account_id: account.clone() });
        }
        !reset.is_empty()
    }

    /// Poll a provider's limit endpoint now.
    pub async fn refresh_limits(&self, account_id: &str) -> HarnessResult<Vec<LimitWindow>> {
        self.import_claude_usage(account_id);
        let adapter =
            self.adapter_for(account_id).ok_or_else(|| HarnessError::new(ErrorKind::Authentication, "Account is not connected"))?;
        let limit_result = if adapter.supports_limit_polling() {
            adapter.fetch_limits().await.map(|windows| self.apply_limits(account_id, windows))
        } else {
            Ok(())
        };
        // Usage history is independent of the current quota endpoint.
        if let Ok(Some(report)) = adapter.fetch_usage_report().await {
            self.usage_reports.write().insert(account_id.to_string(), report);
            self.emit(HarnessEvent::LimitsUpdated { account_id: account_id.to_string() });
        }
        limit_result?;
        Ok(self.limits.read().get(account_id).cloned().unwrap_or_default())
    }
}
