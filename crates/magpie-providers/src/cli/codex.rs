//! ChatGPT-plan access through the official Codex CLI's `app-server`
//! JSON-RPC interface (the same interface used by Codex IDE integrations).
//!
//! One app-server process is shared by all requests on the account and is
//! stopped after a period of inactivity.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::TimeZone;
use magpie_core::*;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_util::sync::CancellationToken;

use super::{classify_cli_failure, command, find_binary, flatten_prompt};
use crate::{catalog, emit, AdapterRequest, EventSender, ProviderAdapter};

const IDLE_SHUTDOWN: Duration = Duration::from_secs(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

type Notification = (String, Value);

struct AppServer {
    child: Mutex<Child>,
    stdin: Mutex<ChildStdin>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, oneshot::Sender<Result<Value, HarnessError>>>>,
    threads: Mutex<HashMap<String, mpsc::UnboundedSender<Notification>>>,
    rate_limits: Mutex<Option<Value>>,
    alive: AtomicBool,
    last_used: std::sync::Mutex<Instant>,
    active: AtomicU64,
}

impl AppServer {
    async fn spawn(bin: &std::path::Path) -> HarnessResult<Arc<Self>> {
        let mut cmd = command(bin, &["OPENAI_API_KEY"]);
        cmd.arg("app-server").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd.spawn().map_err(|e| super::spawn_error(bin, e))?;
        let stdin = child.stdin.take().ok_or_else(|| HarnessError::internal("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| HarnessError::internal("no stdout"))?;
        let server = Arc::new(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            threads: Mutex::new(HashMap::new()),
            rate_limits: Mutex::new(None),
            alive: AtomicBool::new(true),
            last_used: std::sync::Mutex::new(Instant::now()),
            active: AtomicU64::new(0),
        });
        let weak = Arc::downgrade(&server);
        tokio::spawn(read_loop(weak, BufReader::new(stdout)));
        server
            .request(
                "initialize",
                json!({"clientInfo": {"name": "magpie", "title": PRODUCT_NAME, "version": VERSION}, "capabilities": null}),
            )
            .await?;
        server.notify("initialized", Value::Null).await?;
        Ok(server)
    }

    async fn write(&self, msg: Value) -> HarnessResult<()> {
        let mut line = msg.to_string();
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await.map_err(|e| {
            self.alive.store(false, Ordering::SeqCst);
            HarnessError::new(ErrorKind::LocalDependency, format!("Codex app-server unavailable: {e}"))
        })?;
        stdin.flush().await.map_err(|e| HarnessError::new(ErrorKind::LocalDependency, e.to_string()))
    }

    async fn notify(&self, method: &str, params: Value) -> HarnessResult<()> {
        let mut msg = json!({"jsonrpc": "2.0", "method": method});
        if !params.is_null() {
            msg["params"] = params;
        }
        self.write(msg).await
    }

    async fn request(&self, method: &str, params: Value) -> HarnessResult<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        *self.last_used.lock().unwrap() = Instant::now();
        self.write(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).await?;
        match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(HarnessError::new(ErrorKind::LocalDependency, "Codex app-server exited")),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(HarnessError::new(ErrorKind::Timeout, format!("Codex did not answer `{method}` in time")))
            }
        }
    }

    async fn subscribe(&self, thread_id: &str) -> mpsc::UnboundedReceiver<Notification> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.threads.lock().await.insert(thread_id.to_string(), tx);
        rx
    }

    async fn unsubscribe(&self, thread_id: &str) {
        self.threads.lock().await.remove(thread_id);
    }

    async fn kill(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let _ = self.child.lock().await.start_kill();
    }
}

async fn read_loop(server: Weak<AppServer>, stdout: BufReader<tokio::process::ChildStdout>) {
    let mut lines = stdout.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let Some(server) = server.upgrade() else { return };
        let Ok(msg) = serde_json::from_str::<Value>(line.trim()) else { continue };
        let id = msg.get("id").and_then(|v| v.as_u64());
        let method = msg.get("method").and_then(|m| m.as_str()).map(str::to_string);
        match (id, method) {
            (Some(id), None) => {
                if let Some(tx) = server.pending.lock().await.remove(&id) {
                    let res = if let Some(err) = msg.get("error") {
                        let text = err["message"].as_str().unwrap_or("Codex request failed");
                        Err(classify_cli_failure(text, None))
                    } else {
                        Ok(msg.get("result").cloned().unwrap_or(Value::Null))
                    };
                    let _ = tx.send(res);
                }
            }
            (Some(id), Some(method)) => {
                // Server-initiated request. Magpie never grants approvals;
                // the sandbox and approval policy are set so none are needed.
                let response = if method.ends_with("requestApproval") || method.ends_with("Approval") {
                    json!({"jsonrpc": "2.0", "id": id, "result": {"decision": "decline"}})
                } else {
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Not supported by Magpie"}})
                };
                let _ = server.write(response).await;
            }
            (None, Some(method)) => {
                let params = msg.get("params").cloned().unwrap_or(Value::Null);
                if method == "account/rateLimits/updated" {
                    *server.rate_limits.lock().await = params.get("rateLimits").cloned().or(Some(params.clone()));
                    continue;
                }
                if let Some(tid) = params.get("threadId").and_then(|t| t.as_str()) {
                    if let Some(tx) = server.threads.lock().await.get(tid) {
                        let _ = tx.send((method, params));
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(server) = server.upgrade() {
        server.alive.store(false, Ordering::SeqCst);
        for (_, tx) in server.pending.lock().await.drain() {
            let _ = tx.send(Err(HarnessError::new(ErrorKind::LocalDependency, "Codex app-server exited")));
        }
        server.threads.lock().await.clear();
    }
}

pub struct CodexAdapter {
    account: Account,
    server: Arc<Mutex<Option<Arc<AppServer>>>>,
}

impl CodexAdapter {
    pub fn new(account: Account) -> Self {
        Self { account, server: Arc::new(Mutex::new(None)) }
    }

    fn binary(&self) -> HarnessResult<PathBuf> {
        let override_path = self.account.options.get("cli_path").and_then(|v| v.as_str());
        find_binary("codex", override_path).ok_or_else(|| {
            HarnessError::new(ErrorKind::LocalDependency, "Codex CLI is not installed. Install it with: npm install -g @openai/codex")
        })
    }

    async fn server(&self) -> HarnessResult<Arc<AppServer>> {
        let mut guard = self.server.lock().await;
        if let Some(s) = guard.as_ref() {
            if s.alive.load(Ordering::SeqCst) {
                *s.last_used.lock().unwrap() = Instant::now();
                return Ok(s.clone());
            }
        }
        let s = AppServer::spawn(&self.binary()?).await?;
        *guard = Some(s.clone());
        // Reaper: stop the app-server when idle to keep resource use low.
        let slot = Arc::downgrade(&self.server);
        let weak = Arc::downgrade(&s);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let Some(server) = weak.upgrade() else { return };
                if !server.alive.load(Ordering::SeqCst) {
                    return;
                }
                let idle = server.last_used.lock().unwrap().elapsed();
                if idle > IDLE_SHUTDOWN && server.active.load(Ordering::SeqCst) == 0 {
                    server.kill().await;
                    if let Some(slot) = slot.upgrade() {
                        let mut g = slot.lock().await;
                        if g.as_ref().map(|x| Arc::ptr_eq(x, &server)).unwrap_or(false) {
                            *g = None;
                        }
                    }
                    return;
                }
            }
        });
        Ok(s)
    }

    fn limits_from_snapshot(&self, snap: &Value) -> Vec<LimitWindow> {
        let mut out = Vec::new();
        let by_id = snap.get("rateLimitsByLimitId").and_then(|v| v.as_object()).cloned();
        let snapshots: Vec<(String, Value)> = match by_id {
            Some(map) if !map.is_empty() => map.into_iter().collect(),
            _ => {
                let s = snap.get("rateLimits").cloned().unwrap_or_else(|| snap.clone());
                vec![(s["limitId"].as_str().unwrap_or("codex").to_string(), s)]
            }
        };
        for (limit_id, s) in snapshots {
            let name = s["limitName"].as_str().map(str::to_string);
            for slot in ["primary", "secondary"] {
                let w = &s[slot];
                if !w.is_object() {
                    continue;
                }
                let mins = w["windowDurationMins"].as_u64();
                let label = match mins {
                    Some(300) => "5-hour window".to_string(),
                    Some(10080) => "Weekly window".to_string(),
                    Some(1440) => "Daily window".to_string(),
                    Some(m) if m % 60 == 0 => format!("{}-hour window", m / 60),
                    Some(m) => format!("{m}-minute window"),
                    None => format!("{slot} window"),
                };
                let label = match &name {
                    Some(n) => format!("{n} · {label}"),
                    None if limit_id != "codex" => format!("{limit_id} · {label}"),
                    None => label,
                };
                let mut lw = LimitWindow::new(
                    &self.account.id,
                    &format!("codex.{limit_id}.{slot}"),
                    &label,
                    LimitMetric::UsagePercent,
                    Provenance::Reported,
                );
                lw.used_percent = w["usedPercent"].as_f64();
                lw.window_secs = mins.map(|m| m * 60);
                lw.resets_at = w["resetsAt"].as_i64().and_then(|t| chrono::Utc.timestamp_opt(t, 0).single());
                lw.exhausted = lw.used_percent.map(|p| p >= 100.0).unwrap_or(false);
                lw.model_scope = s["normalModelSlug"].as_str().map(str::to_string);
                out.push(lw);
            }
            let credits = &s["credits"];
            if credits["hasCredits"].as_bool() == Some(true) && credits["unlimited"].as_bool() != Some(true) {
                let mut lw = LimitWindow::new(
                    &self.account.id,
                    &format!("codex.{limit_id}.credits"),
                    "Credits",
                    LimitMetric::Credits,
                    Provenance::Reported,
                );
                lw.remaining = credits["balance"].as_str().and_then(|b| b.parse().ok()).or_else(|| credits["balance"].as_f64());
                out.push(lw);
            }
        }
        out
    }

    fn build_input(&self, r: &ExecRequest) -> Vec<Value> {
        let mut input = vec![json!({"type": "text", "text": flatten_prompt(r, false), "text_elements": []})];
        if let Some(last_user) = r.messages.iter().rev().find(|m| m.role == Role::User) {
            for p in &last_user.content {
                if let ContentPart::Image { media_type, data, url } = p {
                    let url = match (data, url) {
                        (Some(d), _) => format!("data:{};base64,{d}", media_type.as_deref().unwrap_or("image/png")),
                        (None, Some(u)) => u.clone(),
                        _ => continue,
                    };
                    input.push(json!({"type": "image", "url": url}));
                }
            }
        }
        input
    }
}

fn usage_from(tu: &Value) -> Option<TokenUsage> {
    let t = tu.get("total").or_else(|| tu.get("last"))?;
    Some(TokenUsage {
        input_tokens: t["inputTokens"].as_u64(),
        output_tokens: t["outputTokens"].as_u64(),
        cached_input_tokens: t["cachedInputTokens"].as_u64(),
        cache_write_tokens: t["cacheWriteInputTokens"].as_u64().filter(|v| *v > 0),
        reasoning_tokens: t["reasoningOutputTokens"].as_u64(),
        provenance: Provenance::Reported,
    })
}

#[async_trait]
impl ProviderAdapter for CodexAdapter {
    fn account(&self) -> &Account {
        &self.account
    }

    async fn verify(&self) -> HarnessResult<VerifiedIdentity> {
        let server = self.server().await?;
        let v = server.request("account/read", json!({})).await?;
        let account = &v["account"];
        if account.is_null() {
            return Err(HarnessError::new(ErrorKind::Authentication, "Codex is not signed in. Run `codex login`."));
        }
        let ty = account["type"].as_str().unwrap_or_default();
        let billing = match ty {
            "chatgpt" => BillingMode::Subscription,
            "apiKey" | "api_key" => BillingMode::Metered,
            _ => BillingMode::Unknown,
        };
        let plan = account["planType"].as_str().map(|p| match p {
            "prolite" => "Pro Lite".to_string(),
            other => {
                let mut c = other.chars();
                c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
            }
        });
        Ok(VerifiedIdentity {
            identity: account["email"].as_str().map(str::to_string),
            plan,
            billing_mode: Some(billing),
            detail: Some(format!("Signed in with {}", if ty == "chatgpt" { "ChatGPT" } else { "an API key" })),
        })
    }

    async fn discover_models(&self) -> HarnessResult<Vec<DiscoveredModel>> {
        let server = self.server().await?;
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..5 {
            let params = match &cursor {
                Some(c) => json!({"cursor": c}),
                None => json!({}),
            };
            let v = server.request("model/list", params).await?;
            for m in v["data"].as_array().cloned().unwrap_or_default() {
                if m["hidden"].as_bool() == Some(true) {
                    continue;
                }
                let Some(id) = m["model"].as_str().or_else(|| m["id"].as_str()) else { continue };
                let mut d = catalog::discovered_from_catalog("openai", id, m["displayName"].as_str().map(str::to_string));
                let desc = m["description"].as_str().unwrap_or_default().to_string();
                let lower = desc.to_ascii_lowercase();
                if lower.contains("frontier") || lower.contains("most demanding") {
                    d.tier = QualityTier::Frontier;
                    d.speed = SpeedClass::Medium;
                } else if lower.contains("fast") || lower.contains("affordable") || lower.contains("efficient") {
                    d.tier = if lower.contains("older") { QualityTier::Light } else { QualityTier::Standard };
                    d.speed = SpeedClass::Fast;
                } else if lower.contains("workhorse") || lower.contains("balanced") {
                    d.tier = QualityTier::High;
                }
                if lower.contains("older") || lower.contains("previous") {
                    // Prefer current generations when quality is otherwise equal.
                    d.tier = match d.tier {
                        QualityTier::Frontier => QualityTier::High,
                        QualityTier::High => QualityTier::Standard,
                        t => t,
                    };
                }
                d.description = Some(desc);
                d.pricing = None;
                d.capabilities.vision = m["inputModalities"].as_array().map(|a| a.iter().any(|x| x == "image")).unwrap_or(false);
                d.capabilities.tools = false;
                d.capabilities.reasoning = m["supportedReasoningEfforts"].as_array().map(|a| !a.is_empty()).unwrap_or(false);
                d.capabilities.structured_output = true;
                d.capabilities.agentic = true;
                d.is_default = m["isDefault"].as_bool() == Some(true);
                d.reasoning_efforts = m["supportedReasoningEfforts"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|e| e["reasoningEffort"].as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                d.metadata_provenance = Provenance::Reported;
                out.push(d);
            }
            cursor = v["nextCursor"].as_str().map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    async fn execute(&self, req: &AdapterRequest, events: EventSender, cancel: CancellationToken) -> HarnessResult<ProviderOutcome> {
        let r = &req.request;
        if r.agent.is_none() {
            return Err(HarnessError::invalid(
                "Codex execution requires explicit agent options. Use an API provider for isolated text generation.",
            ));
        }
        let server = self.server().await?;
        server.active.fetch_add(1, Ordering::SeqCst);
        struct Active(Arc<AppServer>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.active.fetch_sub(1, Ordering::SeqCst);
                *self.0.last_used.lock().unwrap() = Instant::now();
            }
        }
        let _active = Active(server.clone());

        let (cwd, sandbox) = match &r.agent {
            Some(a) => (PathBuf::from(&a.working_dir), if a.allow_writes { "workspace-write" } else { "read-only" }),
            None => (super::ensure_scratch(&req.scratch_dir), "read-only"),
        };
        let mut system: Vec<String> = r.system.iter().cloned().collect();
        system.extend(r.messages.iter().filter(|m| m.role == Role::System).map(|m| m.text_content()));
        let mut params = json!({
            "model": req.model_id,
            "cwd": cwd.display().to_string(),
            "approvalPolicy": "never",
            "sandbox": sandbox,
            "ephemeral": true,
            "serviceName": PRODUCT_NAME,
        });
        if r.agent.is_some() {
            if !system.is_empty() {
                params["developerInstructions"] = json!(system.join("\n\n"));
            }
        } else {
            // Plain generation: replace the coding-agent base prompt.
            params["baseInstructions"] = json!(if system.is_empty() {
                "You are a helpful assistant. Respond directly to the user's request.".to_string()
            } else {
                system.join("\n\n")
            });
        }
        let started = server.request("thread/start", params).await?;
        let thread_id = started["thread"]["id"]
            .as_str()
            .ok_or_else(|| HarnessError::new(ErrorKind::LocalDependency, "Codex did not return a thread id"))?
            .to_string();
        let mut rx = server.subscribe(&thread_id).await;
        struct Unsub(Arc<AppServer>, String);
        impl Drop for Unsub {
            fn drop(&mut self) {
                let s = self.0.clone();
                let t = self.1.clone();
                tokio::spawn(async move { s.unsubscribe(&t).await });
            }
        }
        let _unsub = Unsub(server.clone(), thread_id.clone());

        let mut outcome = ProviderOutcome { resolved_model: started["model"].as_str().map(str::to_string), ..Default::default() };
        if let Some(m) = &outcome.resolved_model {
            emit(&events, ProviderEvent::ResolvedModel(m.clone())).await?;
        }
        let mut turn_params = json!({"threadId": thread_id, "input": self.build_input(r)});
        if let Some(e) = &r.reasoning_effort {
            turn_params["effort"] = json!(e);
        }
        if let Some(ResponseFormat::JsonSchema { schema, .. }) = &r.response_format {
            turn_params["outputSchema"] = schema.clone();
        }
        let turn = server.request("turn/start", turn_params).await?;
        let turn_id = turn["turn"]["id"].as_str().unwrap_or_default().to_string();

        let mut streamed_items: std::collections::HashSet<String> = Default::default();
        let idle = req.timeout.min(Duration::from_secs(900));
        loop {
            let next = tokio::select! {
                n = tokio::time::timeout(idle, rx.recv()) => n,
                _ = cancel.cancelled() => {
                    let _ = server.request("turn/interrupt", json!({"threadId": thread_id, "turnId": turn_id})).await;
                    return Err(HarnessError::cancelled());
                }
            };
            let (method, p) = match next {
                Ok(Some(n)) => n,
                Ok(None) => return Err(HarnessError::new(ErrorKind::LocalDependency, "Codex app-server exited during the request")),
                Err(_) => {
                    let _ = server.request("turn/interrupt", json!({"threadId": thread_id, "turnId": turn_id})).await;
                    return Err(HarnessError::new(ErrorKind::Timeout, "Codex produced no output in time"));
                }
            };
            match method.as_str() {
                "item/agentMessage/delta" => {
                    if let Some(d) = p["delta"].as_str() {
                        if let Some(item) = p["itemId"].as_str() {
                            streamed_items.insert(item.to_string());
                        }
                        emit(&events, ProviderEvent::TextDelta(d.to_string())).await?;
                    }
                }
                "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                    if let Some(d) = p["delta"].as_str() {
                        emit(&events, ProviderEvent::ReasoningDelta(d.to_string())).await?;
                    }
                }
                "item/completed" => {
                    let item = &p["item"];
                    if item["type"] == "agentMessage" {
                        let id = item["id"].as_str().unwrap_or_default();
                        if !streamed_items.contains(id) {
                            if let Some(t) = item["text"].as_str() {
                                emit(&events, ProviderEvent::TextDelta(t.to_string())).await?;
                            }
                        }
                    }
                }
                "thread/tokenUsage/updated" => {
                    if let Some(u) = usage_from(&p["tokenUsage"]) {
                        outcome.usage = u;
                    }
                }
                "model/rerouted" => {
                    if let Some(m) = p["toModel"].as_str().or_else(|| p["model"].as_str()) {
                        outcome.resolved_model = Some(m.to_string());
                        emit(&events, ProviderEvent::ResolvedModel(m.to_string())).await?;
                    }
                }
                "error" => {
                    if p["willRetry"].as_bool() != Some(true) {
                        let msg = p["error"]["message"].as_str().unwrap_or("Codex reported an error");
                        return Err(classify_cli_failure(msg, None));
                    }
                }
                "turn/completed" => {
                    let t = &p["turn"];
                    match t["status"].as_str().unwrap_or("completed") {
                        "failed" => {
                            let msg = t["error"]["message"].as_str().unwrap_or("Codex turn failed");
                            let mut e = classify_cli_failure(msg, None);
                            if matches!(e.kind, ErrorKind::QuotaExhausted | ErrorKind::RateLimited) {
                                if let Some(snap) = server.rate_limits.lock().await.clone() {
                                    e.resets_at =
                                        self.limits_from_snapshot(&snap).iter().filter(|l| l.exhausted).filter_map(|l| l.resets_at).min();
                                }
                            }
                            return Err(e);
                        }
                        "interrupted" => return Err(HarnessError::cancelled()),
                        _ => break,
                    }
                }
                _ => {}
            }
        }
        if let Some(snap) = server.rate_limits.lock().await.clone() {
            outcome.limits = self.limits_from_snapshot(&json!({"rateLimits": snap}));
        }
        Ok(outcome)
    }

    async fn fetch_limits(&self) -> HarnessResult<Vec<LimitWindow>> {
        let server = self.server().await?;
        let v = server.request("account/rateLimits/read", Value::Null).await?;
        Ok(self.limits_from_snapshot(&v))
    }

    fn supports_limit_polling(&self) -> bool {
        true
    }

    async fn fetch_usage_report(&self) -> HarnessResult<Option<ProviderUsageReport>> {
        let server = self.server().await?;
        let v = server.request("account/usage/read", json!({})).await?;
        let num = |x: &Value| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()));
        let daily = v["dailyUsageBuckets"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|b| Some(DailyTokens { date: b["startDate"].as_str()?.to_string(), tokens: num(&b["tokens"])? }))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Some(ProviderUsageReport {
            account_id: self.account.id.clone(),
            lifetime_tokens: num(&v["summary"]["lifetimeTokens"]),
            peak_daily_tokens: num(&v["summary"]["peakDailyTokens"]),
            daily,
            provenance: Provenance::Reported,
            observed_at: now(),
        }))
    }

    async fn shutdown(&self) {
        if let Some(s) = self.server.lock().await.take() {
            s.kill().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> CodexAdapter {
        CodexAdapter::new(Account {
            id: "cx".into(),
            kind: ProviderKind::CodexCli,
            label: "Codex".into(),
            auth_method: AuthMethod::CliDelegated,
            billing_mode: BillingMode::Subscription,
            billing_reported: true,
            base_url: None,
            identity: None,
            plan: None,
            status: ConnectionStatus::Connected,
            status_message: None,
            enabled: true,
            last_verified_at: None,
            created_at: now(),
            has_secret: false,
            secret_store: None,
            options: json!({}),
        })
    }

    #[test]
    fn maps_rate_limit_snapshot() {
        // Shape captured from `account/rateLimits/read` (codex-cli 0.156).
        let v = json!({"ordinaryUsageAllowed":true,"rateLimits":{"limitId":"codex","primary":{"usedPercent":0,"windowDurationMins":10080,"resetsAt":1791991248},"secondary":null,"credits":{"hasCredits":false,"unlimited":false,"balance":"0"}},
            "rateLimitsByLimitId":{"codex":{"limitId":"codex","limitName":null,"primary":{"usedPercent":42.5,"windowDurationMins":300,"resetsAt":1791991248},"secondary":{"usedPercent":100,"windowDurationMins":10080,"resetsAt":1791991248},"credits":{"hasCredits":false}}}});
        let w = adapter().limits_from_snapshot(&v);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].label, "5-hour window");
        assert_eq!(w[0].used_percent, Some(42.5));
        assert_eq!(w[0].window_secs, Some(18000));
        assert!(w[1].exhausted);
        assert_eq!(w[1].resets_at.unwrap().timestamp(), 1791991248);
    }

    #[test]
    fn maps_token_usage() {
        let u = usage_from(&json!({"total":{"totalTokens":120,"inputTokens":100,"cachedInputTokens":40,"cacheWriteInputTokens":0,"outputTokens":20,"reasoningOutputTokens":5}})).unwrap();
        assert_eq!(u.input_tokens, Some(100));
        assert_eq!(u.cached_input_tokens, Some(40));
        assert_eq!(u.cache_write_tokens, None);
        assert_eq!(u.reasoning_tokens, Some(5));
    }
}
