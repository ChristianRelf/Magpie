//! Scriptable adapter for automated tests. Only compiled with the `testing`
//! feature; never available in release builds.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use magpie_core::*;
use tokio_util::sync::CancellationToken;

use crate::{emit, AdapterRequest, EventSender, ProviderAdapter};

#[derive(Debug, Clone)]
pub enum Script {
    /// Stream these chunks then succeed with the given usage.
    Reply { chunks: Vec<String>, usage: TokenUsage, tool_calls: Vec<ToolCall> },
    /// Fail with this error.
    Fail(HarnessError),
    /// Emit one chunk, then fail (tests mid-stream failure handling).
    FailAfterOutput(String, HarnessError),
    /// Wait for the duration (or cancellation) before replying.
    Delay(Duration, Box<Script>),
}

impl Script {
    pub fn text(t: &str) -> Self {
        Script::Reply {
            chunks: vec![t.to_string()],
            usage: TokenUsage { input_tokens: Some(10), output_tokens: Some(5), provenance: Provenance::Reported, ..Default::default() },
            tool_calls: vec![],
        }
    }

    pub fn fail(kind: ErrorKind) -> Self {
        Script::Fail(HarnessError::new(kind, format!("scripted {}", kind.as_str())))
    }
}

pub struct MockAdapter {
    account: Account,
    models: Vec<DiscoveredModel>,
    scripts: Mutex<HashMap<String, Vec<Script>>>,
    pub calls: Arc<AtomicU32>,
    pub limits: Mutex<Vec<LimitWindow>>,
}

impl MockAdapter {
    pub fn new(account: Account, models: Vec<DiscoveredModel>) -> Self {
        Self { account, models, scripts: Mutex::new(HashMap::new()), calls: Arc::new(AtomicU32::new(0)), limits: Mutex::new(vec![]) }
    }

    /// Queue scripted responses for a model; the last script repeats.
    pub fn script(&self, model_id: &str, scripts: Vec<Script>) {
        self.scripts.lock().unwrap().insert(model_id.to_string(), scripts);
    }

    fn next_script(&self, model_id: &str) -> Script {
        let mut map = self.scripts.lock().unwrap();
        match map.get_mut(model_id) {
            Some(list) if list.len() > 1 => list.remove(0),
            Some(list) if list.len() == 1 => list[0].clone(),
            _ => Script::text("ok"),
        }
    }
}

pub fn model(id: &str, tier: QualityTier, speed: SpeedClass, caps: Capabilities, pricing: Option<Pricing>) -> DiscoveredModel {
    DiscoveredModel {
        model_id: id.to_string(),
        display_name: id.to_string(),
        description: None,
        context_window: Some(200_000),
        max_output_tokens: Some(32_000),
        capabilities: caps,
        tier,
        speed,
        pricing,
        metadata_provenance: Provenance::Reported,
        is_default: false,
        reasoning_efforts: vec![],
    }
}

#[async_trait]
impl ProviderAdapter for MockAdapter {
    fn account(&self) -> &Account {
        &self.account
    }

    async fn verify(&self) -> HarnessResult<VerifiedIdentity> {
        Ok(VerifiedIdentity { identity: Some("test@example.com".into()), plan: Some("Test".into()), billing_mode: None, detail: None })
    }

    async fn discover_models(&self) -> HarnessResult<Vec<DiscoveredModel>> {
        Ok(self.models.clone())
    }

    async fn execute(&self, req: &AdapterRequest, events: EventSender, cancel: CancellationToken) -> HarnessResult<ProviderOutcome> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut script = self.next_script(&req.model_id);
        loop {
            match script {
                Script::Delay(d, inner) => {
                    tokio::select! {
                        _ = tokio::time::sleep(d) => {}
                        _ = cancel.cancelled() => return Err(HarnessError::cancelled()),
                    }
                    script = *inner;
                }
                Script::Fail(e) => return Err(e),
                Script::FailAfterOutput(t, e) => {
                    emit(&events, ProviderEvent::TextDelta(t)).await?;
                    return Err(e);
                }
                Script::Reply { chunks, usage, tool_calls } => {
                    for c in chunks {
                        emit(&events, ProviderEvent::TextDelta(c)).await?;
                    }
                    let finish = if tool_calls.is_empty() { FinishReason::Stop } else { FinishReason::ToolCalls };
                    for tc in tool_calls {
                        emit(&events, ProviderEvent::ToolCall(tc)).await?;
                    }
                    return Ok(ProviderOutcome {
                        finish_reason: finish,
                        usage,
                        cost: None,
                        limits: self.limits.lock().unwrap().clone(),
                        resolved_model: Some(req.model_id.clone()),
                    });
                }
            }
        }
    }

    async fn fetch_limits(&self) -> HarnessResult<Vec<LimitWindow>> {
        Ok(self.limits.lock().unwrap().clone())
    }
}
