//! Local API authentication: one admin token for the desktop app and CLI
//! (stored in an owner-only file), plus revocable client keys with scopes.

use magpie_core::*;
use magpie_security::{generate_token, hash_token, token_prefix, verify_token};
use magpie_store::{paths::Paths, ApiClient};
use serde::Serialize;

use crate::Harness;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Run executions.
    Execute,
    /// Read models, usage, limits and status.
    Read,
    /// Permit local CLI agent filesystem workflows.
    Agent,
    /// Manage providers, settings and keys.
    Admin,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Execute => "execute",
            Scope::Read => "read",
            Scope::Admin => "admin",
            Scope::Agent => "agent",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "execute" => Some(Scope::Execute),
            "read" => Some(Scope::Read),
            "admin" => Some(Scope::Admin),
            "agent" => Some(Scope::Agent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Principal {
    Admin,
    Client(ApiClient),
}

impl Principal {
    pub fn name(&self) -> String {
        match self {
            Principal::Admin => "magpie".into(),
            Principal::Client(c) => c.name.clone(),
        }
    }

    pub fn has(&self, scope: Scope) -> bool {
        match self {
            Principal::Admin => true,
            Principal::Client(c) => c.scopes.iter().any(|s| s == scope.as_str() || s == "admin"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedClient {
    pub client: ApiClient,
    /// The plaintext key. Shown once; only its hash is stored.
    pub token: String,
}

pub(crate) fn ensure_admin_token(paths: &Paths) -> HarnessResult<String> {
    let path = paths.admin_token();
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let t = existing.trim();
        if t.starts_with("mga_") && t.len() > 20 {
            return Ok(hash_token(t));
        }
    }
    let token = generate_token("mga");
    magpie_security::secrets::write_private(&path, token.as_bytes())
        .map_err(|e| HarnessError::internal(format!("cannot write admin token: {e}")))?;
    Ok(hash_token(&token))
}

impl Harness {
    /// Authenticate a bearer token.
    pub fn authenticate(&self, token: &str) -> Option<Principal> {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }
        if token.starts_with("mga_") {
            return verify_token(token, &self.admin_token_hash).then_some(Principal::Admin);
        }
        if !token.starts_with("mgp_") {
            return None;
        }
        let client = self.store.client_by_hash(&hash_token(token)).ok().flatten()?;
        let _ = self.store.touch_client(&client.id);
        Some(Principal::Client(client))
    }

    pub fn create_client(&self, name: &str, scopes: &[String]) -> HarnessResult<CreatedClient> {
        let name = name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(HarnessError::invalid("Key name must be 1-80 characters"));
        }
        let mut parsed = Vec::new();
        for s in scopes {
            let sc = Scope::parse(s).ok_or_else(|| HarnessError::invalid(format!("Unknown scope: {s}")))?;
            if !parsed.contains(&sc) {
                parsed.push(sc);
            }
        }
        if parsed.is_empty() {
            return Err(HarnessError::invalid("Choose at least one scope"));
        }
        let token = generate_token("mgp");
        let client = ApiClient {
            id: new_id("key"),
            name: name.to_string(),
            prefix: token_prefix(&token),
            scopes: parsed.iter().map(|s| s.as_str().to_string()).collect(),
            created_at: now(),
            last_used_at: None,
            revoked_at: None,
        };
        self.store.insert_client(&client, &hash_token(&token))?;
        Ok(CreatedClient { client, token })
    }

    pub fn list_clients(&self) -> HarnessResult<Vec<ApiClient>> {
        Ok(self.store.list_clients()?)
    }

    pub fn revoke_client(&self, id: &str) -> HarnessResult<bool> {
        Ok(self.store.revoke_client(id)?)
    }
}
