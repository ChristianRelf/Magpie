//! Google account access through the official Gemini CLI's headless mode
//! (`gemini -p ... --output-format stream-json`).
//!
//! Status: implemented against the documented headless interface but not
//! verified on a live installation by the Magpie maintainers. Event parsing
//! is deliberately tolerant.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use magpie_core::*;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::{classify_cli_failure, command, find_binary, flatten_prompt, JsonlProcess};
use crate::{catalog, emit, AdapterRequest, EventSender, ProviderAdapter};

const MAX_ARG_PROMPT: usize = 100_000;

pub struct GeminiCliAdapter {
    account: Account,
}

impl GeminiCliAdapter {
    pub fn new(account: Account) -> Self {
        Self { account }
    }

    fn binary(&self) -> HarnessResult<PathBuf> {
        let override_path = self.account.options.get("cli_path").and_then(|v| v.as_str());
        find_binary("gemini", override_path).ok_or_else(|| {
            HarnessError::new(ErrorKind::LocalDependency, "Gemini CLI is not installed. Install it with: npm install -g @google/gemini-cli")
        })
    }

    fn gemini_home() -> Option<PathBuf> {
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(|h| PathBuf::from(h).join(".gemini"))
    }

    /// Determine the configured auth type without reading credential
    /// contents: only file presence and the non-secret settings field.
    fn auth_state() -> (bool, Option<String>) {
        let Some(home) = Self::gemini_home() else { return (false, None) };
        let settings: Option<Value> = std::fs::read(home.join("settings.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let selected = settings.as_ref().and_then(|s| {
            s.pointer("/security/auth/selectedType").or_else(|| s.get("selectedAuthType")).and_then(|v| v.as_str()).map(str::to_string)
        });
        let has_oauth = home.join("oauth_creds.json").exists();
        let has_env = std::env::var_os("GEMINI_API_KEY").is_some() || std::env::var_os("GOOGLE_API_KEY").is_some();
        (has_oauth || has_env || selected.is_some(), selected)
    }
}

#[async_trait]
impl ProviderAdapter for GeminiCliAdapter {
    fn account(&self) -> &Account {
        &self.account
    }

    async fn verify(&self) -> HarnessResult<VerifiedIdentity> {
        self.binary()?;
        let (configured, selected) = Self::auth_state();
        if !configured {
            return Err(HarnessError::new(
                ErrorKind::Authentication,
                "Gemini CLI is not signed in. Run `gemini` once in a terminal and choose \"Login with Google\".",
            ));
        }
        let billing = match selected.as_deref() {
            Some("oauth-personal") => BillingMode::Subscription,
            Some("gemini-api-key") | Some("vertex-ai") => BillingMode::Metered,
            _ => BillingMode::Unknown,
        };
        Ok(VerifiedIdentity {
            identity: None,
            plan: None,
            billing_mode: Some(billing),
            detail: selected.map(|s| format!("Auth type: {s}")),
        })
    }

    async fn discover_models(&self) -> HarnessResult<Vec<DiscoveredModel>> {
        let mut ids: Vec<String> = vec!["gemini-2.5-pro".into(), "gemini-2.5-flash".into(), "gemini-2.5-flash-lite".into()];
        if let Some(home) = Self::gemini_home() {
            if let Some(name) = std::fs::read(home.join("settings.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .and_then(|s| s.pointer("/model/name").and_then(|v| v.as_str()).map(str::to_string))
            {
                if !ids.contains(&name) {
                    ids.insert(0, name);
                }
            }
        }
        for extra in crate::extra_models(&self.account) {
            if !ids.contains(&extra) {
                ids.push(extra);
            }
        }
        Ok(ids
            .iter()
            .map(|id| {
                let mut d = catalog::discovered_from_catalog("google", id, None);
                d.pricing = None;
                d.capabilities.tools = false;
                d.capabilities.vision = false;
                d.capabilities.structured_output = false;
                d.capabilities.agentic = true;
                d
            })
            .collect())
    }

    async fn execute(&self, req: &AdapterRequest, events: EventSender, cancel: CancellationToken) -> HarnessResult<ProviderOutcome> {
        let bin = self.binary()?;
        let r = &req.request;
        let cwd = match &r.agent {
            Some(a) => PathBuf::from(&a.working_dir),
            None => super::ensure_scratch(&req.scratch_dir),
        };
        let prompt = flatten_prompt(r, true);
        let mut args: Vec<String> = vec!["--output-format".into(), "stream-json".into(), "-m".into(), req.model_id.clone()];
        match &r.agent {
            Some(a) if a.allow_writes => args.extend(["--approval-mode".into(), "auto_edit".into()]),
            _ => {}
        }
        let stdin = if prompt.len() <= MAX_ARG_PROMPT {
            args.extend(["-p".into(), prompt]);
            None
        } else {
            args.extend(["-p".into(), "Follow the instructions and conversation provided above.".into()]);
            Some(prompt)
        };
        let mut cmd = command(&bin, &[]);
        cmd.args(&args).current_dir(&cwd);
        let mut proc = JsonlProcess::spawn(cmd, stdin).await?;
        let mut outcome = ProviderOutcome::default();
        let mut got_text = false;
        let mut failure: Option<HarnessError> = None;
        while let Some(v) = proc.next(&cancel, req.timeout.min(Duration::from_secs(600))).await? {
            match v["type"].as_str().unwrap_or_default() {
                "init" => {
                    if let Some(m) = v["model"].as_str() {
                        outcome.resolved_model = Some(m.to_string());
                        emit(&events, ProviderEvent::ResolvedModel(m.to_string())).await?;
                    }
                }
                "message" if v["role"] == "assistant" => {
                    if let Some(t) = v["content"].as_str() {
                        got_text = true;
                        emit(&events, ProviderEvent::TextDelta(t.to_string())).await?;
                    }
                }
                "error" => {
                    if v["severity"].as_str() != Some("warning") {
                        failure = Some(classify_cli_failure(v["message"].as_str().unwrap_or("Gemini CLI error"), None));
                    }
                }
                "result" => {
                    let stats = &v["stats"];
                    let input = stats["input_tokens"].as_u64().or_else(|| stats["input"].as_u64());
                    let output = stats["output_tokens"].as_u64().or_else(|| stats["output"].as_u64());
                    if input.is_some() || output.is_some() {
                        outcome.usage = TokenUsage {
                            input_tokens: input,
                            output_tokens: output,
                            cached_input_tokens: stats["cached"].as_u64(),
                            cache_write_tokens: None,
                            reasoning_tokens: None,
                            provenance: Provenance::Reported,
                        };
                    }
                    if v["status"].as_str() == Some("error") {
                        let msg = v["error"]["message"].as_str().unwrap_or("Gemini CLI request failed");
                        failure = Some(classify_cli_failure(msg, None));
                    } else if !got_text {
                        if let Some(t) = v["response"].as_str() {
                            got_text = true;
                            emit(&events, ProviderEvent::TextDelta(t.to_string())).await?;
                        }
                    }
                }
                _ => {}
            }
        }
        let (code, stderr) = proc.finish().await;
        if let Some(e) = failure {
            return Err(e);
        }
        if !got_text && code != Some(0) {
            return Err(classify_cli_failure(&stderr, code));
        }
        Ok(outcome)
    }
}
