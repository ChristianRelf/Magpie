use magpie_core::*;
use magpie_providers::SharedAdapter;
use magpie_security::SecretBackend;
use serde::Deserialize;

use crate::{AccountEntry, Harness, HarnessEvent};

#[derive(Clone, Deserialize)]
pub struct ConnectRequest {
    pub kind: ProviderKind,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub oauth_token: Option<String>,
    #[serde(default)]
    pub auth_mode: CliAuthMode,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub options: Option<serde_json::Value>,
    /// User-declared billing mode for providers that cannot report it.
    #[serde(default)]
    pub billing_mode: Option<BillingMode>,
}

#[derive(Clone, Default, Deserialize)]
pub struct AccountPatch {
    pub label: Option<String>,
    pub enabled: Option<bool>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub oauth_token: Option<String>,
    pub options: Option<serde_json::Value>,
    pub billing_mode: Option<BillingMode>,
}

fn validate_base_url(url: &str) -> HarnessResult<()> {
    let ok = (url.starts_with("http://") || url.starts_with("https://")) && url.len() < 2048 && !url.contains(char::is_whitespace);
    if !ok {
        return Err(HarnessError::invalid("Base URL must be an http(s) URL"));
    }
    Ok(())
}

impl Harness {
    pub(crate) async fn load_accounts(&self) -> HarnessResult<()> {
        let accounts = self.store.list_accounts()?;
        for account in accounts {
            let adapter = self.build_adapter(&account).await.ok();
            self.accounts.write().insert(account.id.clone(), AccountEntry { account, adapter });
        }
        Ok(())
    }

    async fn read_secret(&self, account: &Account) -> HarnessResult<Option<String>> {
        if !account.has_secret {
            return Ok(None);
        }
        let secrets = self.secrets.clone();
        let id = account.id.clone();
        let backend = account.secret_store.as_deref().and_then(SecretBackend::parse);
        tokio::task::spawn_blocking(move || secrets.get(&id, backend))
            .await
            .map_err(|e| HarnessError::internal(e.to_string()))?
            .map_err(|e| HarnessError::new(ErrorKind::Authentication, format!("Could not read credential: {e}")))
    }

    async fn write_secret(&self, id: &str, secret: &str) -> HarnessResult<SecretBackend> {
        let secrets = self.secrets.clone();
        let id = id.to_string();
        let secret = secret.to_string();
        tokio::task::spawn_blocking(move || secrets.set(&id, &secret))
            .await
            .map_err(|e| HarnessError::internal(e.to_string()))?
            .map_err(|e| HarnessError::internal(format!("Could not store credential: {e}")))
    }

    async fn build_adapter(&self, account: &Account) -> HarnessResult<SharedAdapter> {
        let secret = self.read_secret(account).await?;
        self.adapter_with_secret(account, secret)
    }

    fn adapter_with_secret(&self, account: &Account, secret: Option<String>) -> HarnessResult<SharedAdapter> {
        let mut adapter_account = account.clone();
        if account.has_managed_profile() {
            // The client cannot choose a path or redirect another profile.
            let dir = self.paths.root.join("auth-profiles").join(&account.id);
            std::fs::create_dir_all(&dir).map_err(|e| HarnessError::internal(format!("Cannot create authentication profile: {e}")))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                    .map_err(|e| HarnessError::internal(format!("Cannot protect authentication profile: {e}")))?;
            }
            adapter_account.options["_profile_home"] = serde_json::json!(dir);
        }
        (self.factory)(adapter_account, secret)
    }

    pub(crate) fn adapter_for(&self, account_id: &str) -> Option<SharedAdapter> {
        self.accounts.read().get(account_id).and_then(|e| e.adapter.clone())
    }

    pub fn list_accounts(&self) -> Vec<Account> {
        let mut v: Vec<Account> = self.accounts.read().values().map(|e| e.account.clone()).collect();
        v.sort_by_key(|a| a.created_at);
        v
    }

    pub fn get_account(&self, id: &str) -> Option<Account> {
        self.accounts.read().get(id).map(|e| e.account.clone())
    }

    fn save_account(&self, account: &Account) -> HarnessResult<()> {
        let mut accounts = self.accounts.write();
        let entry = accounts.get_mut(&account.id).ok_or_else(|| HarnessError::invalid("Account was disconnected"))?;
        self.store.upsert_account(account)?;
        entry.account = account.clone();
        drop(accounts);
        self.emit(HarnessEvent::AccountUpdated { account: account.clone() });
        Ok(())
    }

    /// Create a connection, verify it and discover its models.
    pub async fn connect(&self, req: ConnectRequest) -> HarnessResult<Account> {
        let _guard = self.connect_lock.lock().await;
        let d = req.kind.descriptor();
        if !d.allow_multiple && self.accounts.read().values().any(|e| e.account.kind == req.kind) {
            return Err(HarnessError::invalid(format!("{} is already connected", d.name)));
        }
        if let Some(u) = &req.base_url {
            validate_base_url(u)?;
        }
        if req.kind == ProviderKind::OpenAiCompatible && req.base_url.is_none() {
            return Err(HarnessError::invalid("A base URL is required for OpenAI-compatible endpoints"));
        }
        match (req.kind, req.auth_mode) {
            (_, CliAuthMode::Existing) => {}
            (ProviderKind::CodexCli, CliAuthMode::Isolated) => {}
            (ProviderKind::ClaudeCode, CliAuthMode::SavedToken) => {}
            _ => return Err(HarnessError::invalid("This provider does not support the selected authentication mode")),
        }
        if d.auth_method == AuthMethod::CliDelegated
            && req.auth_mode == CliAuthMode::Existing
            && self.accounts.read().values().any(|e| e.account.kind == req.kind && e.account.cli_auth_mode() == CliAuthMode::Existing)
        {
            return Err(HarnessError::invalid("The shared CLI login is already connected. Add a separate sign-in or saved token instead."));
        }
        if req.oauth_token.is_some() && req.auth_mode != CliAuthMode::SavedToken {
            return Err(HarnessError::invalid("OAuth tokens are only supported with Claude Code saved-token authentication"));
        }
        if req.api_key.is_some() && d.auth_method == AuthMethod::CliDelegated {
            return Err(HarnessError::invalid("Use the API provider to connect an API key"));
        }
        let key = req.oauth_token.as_deref().or(req.api_key.as_deref()).map(str::trim).filter(|k| !k.is_empty()).map(str::to_string);
        if req.auth_mode == CliAuthMode::SavedToken && key.is_none() {
            return Err(HarnessError::invalid("A token generated by claude setup-token is required"));
        }
        if d.auth_method == AuthMethod::ApiKey && key.is_none() && req.kind != ProviderKind::OpenAiCompatible {
            return Err(HarnessError::new(ErrorKind::Authentication, "An API key is required"));
        }
        if let Some(opts) = &req.options {
            validate_options(opts)?;
        }
        let mut options = req.options.unwrap_or_else(|| serde_json::json!({}));
        if req.auth_mode != CliAuthMode::Existing {
            options["auth_mode"] = serde_json::json!(req.auth_mode);
        }
        let id = new_id("acc");
        let mut account = Account {
            id: id.clone(),
            kind: req.kind,
            label: req.label.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| d.name.to_string()),
            auth_method: if req.auth_mode == CliAuthMode::SavedToken { AuthMethod::CliToken } else { d.auth_method },
            billing_mode: req.billing_mode.unwrap_or(d.default_billing),
            billing_reported: false,
            base_url: req.base_url,
            identity: None,
            plan: None,
            status: ConnectionStatus::Pending,
            status_message: None,
            enabled: true,
            last_verified_at: None,
            created_at: now(),
            has_secret: false,
            secret_store: None,
            options,
        };
        let adapter = self.adapter_with_secret(&account, key.clone())?;
        if req.auth_mode == CliAuthMode::Isolated {
            account.status = ConnectionStatus::NeedsAuth;
            account.status_message = Some("Sign in to this separate profile with your ChatGPT account.".into());
        } else {
            // Verify keys before persisting; never make a paid test request.
            let identity = adapter.verify().await?;
            apply_identity(&mut account, identity);
        }
        if let Some(k) = &key {
            let backend = self.write_secret(&id, k).await?;
            account.has_secret = true;
            account.secret_store = Some(backend.as_str().to_string());
        }
        self.store.upsert_account(&account)?;
        // Rebuild with the final account (billing mode may have changed).
        adapter.shutdown().await;
        let adapter = self.adapter_with_secret(&account, key)?;
        self.accounts.write().insert(id.clone(), AccountEntry { account: account.clone(), adapter: Some(adapter) });
        self.emit(HarnessEvent::AccountUpdated { account: account.clone() });
        if account.status == ConnectionStatus::Connected {
            if let Err(e) = self.refresh_models(&id).await {
                tracing::warn!(account = %id, error = %e, "model discovery failed after connect");
            }
            let _ = self.poke_limits.send(id.clone());
        }
        Ok(self.get_account(&id).unwrap_or(account))
    }

    pub async fn begin_account_login(self: &std::sync::Arc<Self>, id: &str) -> HarnessResult<LoginChallenge> {
        let _guard = self.connect_lock.lock().await;
        let account = self.get_account(id).ok_or_else(|| HarnessError::invalid("Account not found"))?;
        if account.kind != ProviderKind::CodexCli || account.cli_auth_mode() != CliAuthMode::Isolated {
            return Err(HarnessError::invalid("Browser sign-in requires a separate Codex profile"));
        }
        if account.status == ConnectionStatus::Connected {
            return Err(HarnessError::invalid("This profile is already connected. Add another profile for a different account."));
        }
        if let Some(previous) = self.login_tasks.write().remove(id) {
            previous.cancel();
        }
        let challenge = self
            .adapter_for(id)
            .ok_or_else(|| HarnessError::new(ErrorKind::LocalDependency, "Provider adapter unavailable"))?
            .begin_login()
            .await?;
        let cancelled = self.shutdown.child_token();
        self.login_tasks.write().insert(id.to_string(), cancelled.clone());
        let harness = self.clone();
        let id = id.to_string();
        tokio::spawn(async move {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
            while std::time::Instant::now() < deadline {
                tokio::select! {
                    _ = cancelled.cancelled() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
                }
                if harness.get_account(&id).is_none() {
                    break;
                }
                if let Ok(account) = harness.verify_account(&id).await {
                    if account.status == ConnectionStatus::Connected {
                        let _ = harness.poke_limits.send(id.clone());
                        break;
                    }
                }
            }
            cancelled.cancel();
            harness.login_tasks.write().retain(|_, task| !task.is_cancelled());
        });
        Ok(challenge)
    }

    pub async fn update_account(&self, id: &str, patch: AccountPatch) -> HarnessResult<Account> {
        let mut account = self.get_account(id).ok_or_else(|| HarnessError::new(ErrorKind::ModelNotFound, "Account not found"))?;
        let mut rebuild = false;
        if let Some(l) = patch.label.filter(|l| !l.trim().is_empty()) {
            account.label = l.trim().chars().take(80).collect();
        }
        if let Some(e) = patch.enabled {
            account.enabled = e;
            if !e {
                account.status = ConnectionStatus::Disabled;
            } else if account.status == ConnectionStatus::Disabled {
                account.status = ConnectionStatus::Pending;
            }
        }
        if let Some(u) = patch.base_url {
            if u.is_empty() {
                account.base_url = None;
            } else {
                validate_base_url(&u)?;
                account.base_url = Some(u);
            }
            rebuild = true;
        }
        if let Some(o) = patch.options {
            validate_options(&o)?;
            let auth_mode = account.cli_auth_mode();
            account.options = o;
            if auth_mode != CliAuthMode::Existing {
                account.options["auth_mode"] = serde_json::json!(auth_mode);
            }
            rebuild = true;
        }
        if let Some(b) = patch.billing_mode {
            if !account.billing_reported {
                account.billing_mode = b;
                rebuild = true;
            }
        }
        if patch.oauth_token.is_some() && account.auth_method != AuthMethod::CliToken {
            return Err(HarnessError::invalid("This connection does not use a saved CLI token"));
        }
        if patch.api_key.is_some() && account.auth_method != AuthMethod::ApiKey {
            return Err(HarnessError::invalid("This connection does not use an API key"));
        }
        if let Some(k) = patch.oauth_token.or(patch.api_key).map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
            // A failed replacement must leave the previous credential intact.
            let candidate = self.adapter_with_secret(&account, Some(k.clone()))?;
            let verified = candidate.verify().await;
            candidate.shutdown().await;
            apply_identity(&mut account, verified?);
            let backend = self.write_secret(id, &k).await?;
            account.has_secret = true;
            account.secret_store = Some(backend.as_str().to_string());
            rebuild = true;
        }
        self.save_account(&account)?;
        if rebuild {
            let adapter = self.build_adapter(&account).await.ok();
            if let Some(e) = self.accounts.write().get_mut(id) {
                e.adapter = adapter;
            }
        }
        if account.enabled && (rebuild || account.status == ConnectionStatus::Pending) {
            return self.verify_account(id).await;
        }
        self.emit(HarnessEvent::ModelsUpdated { account_id: Some(id.to_string()) });
        Ok(account)
    }

    /// Remove an account and revoke its stored credential.
    pub async fn delete_account(&self, id: &str) -> HarnessResult<()> {
        let _guard = self.connect_lock.lock().await;
        let account = self.get_account(id).ok_or_else(|| HarnessError::new(ErrorKind::ModelNotFound, "Account not found"))?;
        if self.active.read().values().any(|e| e.summary.model.as_ref().is_some_and(|m| m.account_id == id)) {
            return Err(HarnessError::invalid("Cancel active executions on this account before disconnecting it"));
        }
        if let Some(login) = self.login_tasks.write().remove(id) {
            login.cancel();
        }
        if let Some(adapter) = self.adapter_for(id) {
            adapter.revoke_auth().await?;
        }
        if account.has_secret {
            let secrets = self.secrets.clone();
            let owned_id = id.to_string();
            let backend = account.secret_store.as_deref().and_then(SecretBackend::parse);
            tokio::task::spawn_blocking(move || secrets.delete(&owned_id, backend))
                .await
                .map_err(|e| HarnessError::internal(e.to_string()))?
                .map_err(|e| HarnessError::internal(format!("Could not remove credential: {e}")))?;
        }
        let entry = self.accounts.write().remove(id);
        let Some(entry) = entry else {
            return Err(HarnessError::new(ErrorKind::ModelNotFound, "Account not found"));
        };
        if let Some(a) = entry.adapter {
            a.shutdown().await;
        }
        self.store.delete_account(id)?;
        self.models.write().retain(|(a, _, _)| a != id);
        self.limits.write().remove(id);
        self.usage_reports.write().remove(id);
        self.prefs.write().retain(|k, _| !k.starts_with(&format!("{id}/")));
        self.emit(HarnessEvent::AccountRemoved { account_id: id.to_string() });
        self.emit(HarnessEvent::ModelsUpdated { account_id: Some(id.to_string()) });
        if account.has_managed_profile() {
            let _ = std::fs::remove_dir_all(self.paths.root.join("auth-profiles").join(id));
        }
        Ok(())
    }

    /// Re-check a connection. Updates status, identity and plan.
    pub async fn verify_account(&self, id: &str) -> HarnessResult<Account> {
        let mut account = self.get_account(id).ok_or_else(|| HarnessError::new(ErrorKind::ModelNotFound, "Account not found"))?;
        if !account.enabled {
            return Ok(account);
        }
        // A setup-token has no documented, allowance-free remote validation
        // endpoint. Local `auth status` must never revive a credential that
        // Claude has already rejected. Replacing the token explicitly verifies
        // its CLI configuration in update_account before clearing this state.
        if account.auth_method == AuthMethod::CliToken && account.status == ConnectionStatus::NeedsAuth {
            return Ok(account);
        }
        let adapter = match self.adapter_for(id) {
            Some(a) => Ok(a),
            None => self.build_adapter(&account).await,
        };
        let was_connected = account.status == ConnectionStatus::Connected;
        match adapter {
            Ok(adapter) => match adapter.verify().await {
                Ok(identity) => {
                    let billing_before = account.billing_mode;
                    apply_identity(&mut account, identity);
                    self.save_account(&account)?;
                    if billing_before != account.billing_mode {
                        if let Ok(a) = self.build_adapter(&account).await {
                            if let Some(e) = self.accounts.write().get_mut(id) {
                                e.adapter = Some(a);
                            }
                        }
                    } else if let Some(e) = self.accounts.write().get_mut(id) {
                        e.adapter.get_or_insert(adapter);
                    }
                    let has_models = self.models.read().iter().any(|(a, _, _)| a == id);
                    if !has_models {
                        let _ = self.refresh_models(id).await;
                    }
                }
                Err(e) => {
                    self.set_failure(&mut account, &e, was_connected)?;
                }
            },
            Err(e) => self.set_failure(&mut account, &e, was_connected)?,
        }
        self.emit(HarnessEvent::ModelsUpdated { account_id: Some(id.to_string()) });
        Ok(account)
    }

    pub(crate) fn set_failure(&self, account: &mut Account, e: &HarnessError, was_connected: bool) -> HarnessResult<()> {
        account.status = match e.kind {
            ErrorKind::Authentication | ErrorKind::PermissionDenied => ConnectionStatus::NeedsAuth,
            ErrorKind::LocalDependency | ErrorKind::Network => ConnectionStatus::Unavailable,
            _ => ConnectionStatus::Error,
        };
        account.status_message = Some(e.message.clone());
        self.save_account(account)?;
        if was_connected {
            let (kind, title) = if account.status == ConnectionStatus::NeedsAuth {
                (NotificationKind::AuthExpired, format!("{} needs to sign in again", account.label))
            } else {
                (NotificationKind::ProviderDisconnected, format!("{} is unavailable", account.label))
            };
            self.notify(kind, title, e.message.clone(), Some(account.id.clone()));
        }
        Ok(())
    }

    /// Re-run model discovery for an account.
    pub async fn refresh_models(&self, id: &str) -> HarnessResult<usize> {
        let adapter = self.adapter_for(id).ok_or_else(|| HarnessError::new(ErrorKind::Authentication, "Account is not connected"))?;
        let models = adapter.discover_models().await?;
        self.store.replace_models(id, &models)?;
        let ts = now();
        {
            let mut all = self.models.write();
            all.retain(|(a, _, _)| a != id);
            all.extend(models.iter().cloned().map(|m| (id.to_string(), m, ts)));
        }
        self.blocked_models.write().retain(|k, _| !k.starts_with(&format!("{id}/")));
        self.emit(HarnessEvent::ModelsUpdated { account_id: Some(id.to_string()) });
        Ok(models.len())
    }

    /// All models joined with account state and preferences.
    pub fn list_models(&self) -> Vec<ModelInfo> {
        let accounts = self.accounts.read();
        let prefs = self.prefs.read();
        let blocked = self.blocked_models.read();
        let now = now();
        let mut out: Vec<ModelInfo> = self
            .models
            .read()
            .iter()
            .filter_map(|(account_id, m, discovered_at)| {
                let entry = accounts.get(account_id)?;
                let key = ModelInfo::make_key(account_id, &m.model_id);
                let account = &entry.account;
                let block = blocked.get(&key).filter(|(until, _)| *until > now);
                let (available, reason) = if !account.enabled {
                    (false, Some("Account disabled".to_string()))
                } else if account.status != ConnectionStatus::Connected {
                    (false, Some(account.status_message.clone().unwrap_or_else(|| "Account not connected".into())))
                } else if entry.adapter.is_none() {
                    (false, Some("Adapter unavailable".to_string()))
                } else if let Some((_, why)) = block {
                    (false, Some(why.clone()))
                } else {
                    (true, None)
                };
                Some(ModelInfo {
                    key: key.clone(),
                    account_id: account_id.clone(),
                    account_label: account.label.clone(),
                    provider: account.kind,
                    billing_mode: account.billing_mode,
                    model: m.clone(),
                    preference: prefs.get(&key).cloned().unwrap_or_default(),
                    available,
                    unavailable_reason: reason,
                    discovered_at: *discovered_at,
                })
            })
            .collect();
        out.sort_by(|a, b| {
            a.account_label.cmp(&b.account_label).then(b.model.tier.cmp(&a.model.tier)).then(a.model.model_id.cmp(&b.model.model_id))
        });
        out
    }

    pub fn set_model_preference(&self, key: &str, pref: ModelPreference) -> HarnessResult<ModelPreference> {
        if !self.models.read().iter().any(|(a, m, _)| ModelInfo::make_key(a, &m.model_id) == key) {
            return Err(HarnessError::new(ErrorKind::ModelNotFound, "Model not found"));
        }
        if let Some(r) = pref.reserve_percent {
            if !(0.0..=90.0).contains(&r) {
                return Err(HarnessError::invalid("Reserve must be between 0% and 90%"));
            }
        }
        if let Some(p) = &pref.price_override {
            if p.input_per_mtok < 0.0 || p.output_per_mtok < 0.0 {
                return Err(HarnessError::invalid("Prices must be non-negative"));
            }
        }
        let mut pref = pref;
        if let Some(p) = pref.price_override.as_mut() {
            p.provenance = Provenance::Calculated;
        }
        pref.priority = pref.priority.clamp(-10, 10);
        self.store.set_model_pref(key, &pref)?;
        self.prefs.write().insert(key.to_string(), pref.clone());
        self.emit(HarnessEvent::ModelsUpdated { account_id: key.split('/').next().map(str::to_string) });
        Ok(pref)
    }

    /// Detect official provider CLIs installed on this machine.
    pub async fn detect_clis(&self) -> Vec<magpie_providers::cli::CliStatus> {
        let mut out = Vec::new();
        for kind in [ProviderKind::ClaudeCode, ProviderKind::CodexCli, ProviderKind::GeminiCli] {
            out.push(magpie_providers::cli::detect(kind).await);
        }
        out
    }

    pub fn provider_usage_reports(&self) -> Vec<ProviderUsageReport> {
        self.usage_reports.read().values().cloned().collect()
    }

    pub(crate) fn mark_activity(&self, account_id: &str) {
        self.last_activity.write().insert(account_id.to_string(), std::time::Instant::now());
    }

    pub(crate) fn block_model(&self, key: &str, reason: String) {
        self.blocked_models.write().insert(key.to_string(), (now() + chrono::Duration::hours(1), reason));
        self.emit(HarnessEvent::ModelsUpdated { account_id: key.split('/').next().map(str::to_string) });
    }

    pub(crate) fn all_adapters(&self) -> Vec<(Account, SharedAdapter)> {
        self.accounts
            .read()
            .values()
            .filter(|e| e.account.is_usable())
            .filter_map(|e| e.adapter.clone().map(|a| (e.account.clone(), a)))
            .collect()
    }
}

fn validate_options(options: &serde_json::Value) -> HarnessResult<()> {
    if !options.is_object() {
        return Err(HarnessError::invalid("options must be an object"));
    }
    if options.get("auth_mode").is_some() || options.get("_profile_home").is_some() {
        return Err(HarnessError::invalid("Authentication profile options are managed by Magpie"));
    }
    Ok(())
}

fn apply_identity(account: &mut Account, id: VerifiedIdentity) {
    account.status = ConnectionStatus::Connected;
    account.status_message = id.detail;
    account.last_verified_at = Some(now());
    if id.identity.is_some() {
        account.identity = id.identity;
    }
    if id.plan.is_some() {
        account.plan = id.plan;
    }
    if let Some(b) = id.billing_mode {
        if b != BillingMode::Unknown {
            account.billing_mode = b;
            account.billing_reported = true;
        }
    }
}
