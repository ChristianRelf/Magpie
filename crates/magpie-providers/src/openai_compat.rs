//! Adapter for the OpenAI chat-completions protocol. Serves OpenAI itself
//! and every compatible provider; per-provider differences are captured in
//! [`Profile`].

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use magpie_core::*;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::http::{self, error_from_response, transport_error};
use crate::{catalog, emit, AdapterRequest, EventSender, ProviderAdapter};

#[derive(Debug, Clone)]
struct Profile {
    vendor: &'static str,
    /// Send `stream_options.include_usage`.
    stream_usage: bool,
    /// Use `max_completion_tokens` instead of `max_tokens`.
    max_completion_tokens: bool,
    /// Pass `reasoning_effort`.
    reasoning_effort: bool,
    requires_key: bool,
}

fn profile(kind: ProviderKind) -> Profile {
    use ProviderKind::*;
    let p = Profile { vendor: "generic", stream_usage: true, max_completion_tokens: false, reasoning_effort: false, requires_key: true };
    match kind {
        OpenAi => Profile { vendor: "openai", max_completion_tokens: true, reasoning_effort: true, ..p },
        OpenRouter => Profile { vendor: "openrouter", reasoning_effort: true, ..p },
        Groq => Profile { vendor: "groq", ..p },
        Mistral => Profile { vendor: "mistral", stream_usage: false, ..p },
        DeepSeek => Profile { vendor: "deepseek", ..p },
        Ollama => Profile { vendor: "ollama", requires_key: false, ..p },
        LmStudio => Profile { vendor: "lmstudio", stream_usage: false, requires_key: false, ..p },
        _ => Profile { stream_usage: false, requires_key: false, ..p },
    }
}

pub struct OpenAiCompatAdapter {
    account: Account,
    key: Option<String>,
    base_url: String,
    profile: Profile,
    http: reqwest::Client,
}

impl OpenAiCompatAdapter {
    pub fn new(account: Account, key: Option<String>) -> HarnessResult<Self> {
        let profile = profile(account.kind);
        let base_url = account.base_url().ok_or_else(|| HarnessError::invalid("A base URL is required for this provider"))?;
        if profile.requires_key && key.as_deref().map(str::is_empty).unwrap_or(true) {
            return Err(HarnessError::new(ErrorKind::Authentication, "An API key is required"));
        }
        Ok(Self { base_url: http::base(&base_url), account, key, profile, http: http::client() })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut rb = self.http.request(method, format!("{}{}", self.base_url, path));
        if let Some(k) = self.key.as_deref().filter(|k| !k.is_empty()) {
            rb = rb.bearer_auth(k);
        }
        if self.account.kind == ProviderKind::OpenRouter {
            rb = rb.header("HTTP-Referer", "https://github.com/christianrelf/magpie").header("X-Title", PRODUCT_NAME);
        }
        if let Some(headers) = self.account.options.get("headers").and_then(|h| h.as_object()) {
            for (k, v) in headers {
                if let Some(v) = v.as_str() {
                    rb = rb.header(k.as_str(), v);
                }
            }
        }
        rb
    }

    pub(crate) fn build_body(&self, req: &AdapterRequest) -> Value {
        let r = &req.request;
        let mut messages = Vec::new();
        if let Some(sys) = &r.system {
            messages.push(json!({"role": "system", "content": sys}));
        }
        for m in &r.messages {
            messages.push(message_json(m));
        }
        let mut body = json!({
            "model": req.model_id,
            "messages": messages,
            "stream": true,
        });
        let obj = body.as_object_mut().unwrap();
        if self.profile.stream_usage {
            obj.insert("stream_options".into(), json!({"include_usage": true}));
        }
        if let Some(max) = r.max_output_tokens {
            let key = if self.profile.max_completion_tokens { "max_completion_tokens" } else { "max_tokens" };
            obj.insert(key.into(), json!(max));
        }
        if let Some(t) = r.temperature {
            obj.insert("temperature".into(), json!(t));
        }
        if let (true, Some(effort)) = (self.profile.reasoning_effort, &r.reasoning_effort) {
            if self.account.kind == ProviderKind::OpenRouter {
                obj.insert("reasoning".into(), json!({"effort": effort}));
            } else {
                obj.insert("reasoning_effort".into(), json!(effort));
            }
        }
        if !r.tools.is_empty() {
            let tools: Vec<Value> = r
                .tools
                .iter()
                .map(|t| {
                    json!({"type": "function", "function": {
                        "name": t.name,
                        "description": t.description.clone().unwrap_or_default(),
                        "parameters": t.parameters,
                    }})
                })
                .collect();
            obj.insert("tools".into(), Value::Array(tools));
            if let Some(tc) = &r.tool_choice {
                obj.insert(
                    "tool_choice".into(),
                    match tc {
                        ToolChoice::Auto => json!("auto"),
                        ToolChoice::None => json!("none"),
                        ToolChoice::Required => json!("required"),
                        ToolChoice::Tool { name } => json!({"type": "function", "function": {"name": name}}),
                    },
                );
            }
        }
        match &r.response_format {
            Some(ResponseFormat::JsonObject) => {
                obj.insert("response_format".into(), json!({"type": "json_object"}));
            }
            Some(ResponseFormat::JsonSchema { name, schema, strict }) => {
                obj.insert(
                    "response_format".into(),
                    json!({"type": "json_schema", "json_schema": {"name": name, "schema": schema, "strict": strict}}),
                );
            }
            _ => {}
        }
        body
    }

    async fn openrouter_credits(&self) -> HarnessResult<Vec<LimitWindow>> {
        let resp = self.request(reqwest::Method::GET, "/key").send().await.map_err(transport_error)?;
        if !resp.status().is_success() {
            return Err(error_from_response(resp).await);
        }
        let v: Value = resp.json().await.map_err(transport_error)?;
        let d = &v["data"];
        let limit = d["limit"].as_f64();
        let remaining = d["limit_remaining"].as_f64();
        let usage = d["usage"].as_f64();
        let mut w = LimitWindow::new(&self.account.id, "credits", "Key credit limit", LimitMetric::Credits, Provenance::Reported);
        if limit.is_none() && remaining.is_none() {
            w.key = "spend".into();
            w.label = "Key spend".into();
            w.metric = LimitMetric::Spend;
        }
        w.limit = limit;
        w.remaining = remaining;
        w.used = usage;
        w.exhausted = remaining.map(|r| r <= 0.0).unwrap_or(false);
        let out = vec![w];
        Ok(out)
    }

    async fn deepseek_balance(&self) -> HarnessResult<Vec<LimitWindow>> {
        let url = format!("{}/user/balance", self.base_url.trim_end_matches("/v1"));
        let mut rb = self.http.get(url);
        if let Some(k) = &self.key {
            rb = rb.bearer_auth(k);
        }
        let resp = rb.send().await.map_err(transport_error)?;
        if !resp.status().is_success() {
            return Err(error_from_response(resp).await);
        }
        let v: Value = resp.json().await.map_err(transport_error)?;
        let usd = v["balance_infos"]
            .as_array()
            .and_then(|a| a.iter().find(|b| b["currency"] == "USD").or_else(|| a.first()))
            .and_then(|b| b["total_balance"].as_str().and_then(|s| s.parse::<f64>().ok()));
        Ok(usd
            .map(|bal| {
                let mut w = LimitWindow::new(&self.account.id, "credits", "Account balance", LimitMetric::Credits, Provenance::Reported);
                w.remaining = Some(bal);
                w.exhausted = bal <= 0.0 || v["is_available"] == Value::Bool(false);
                vec![w]
            })
            .unwrap_or_default())
    }
}

fn message_json(m: &Message) -> Value {
    match m.role {
        Role::Tool => json!({
            "role": "tool",
            "tool_call_id": m.tool_call_id.clone().unwrap_or_default(),
            "content": m.text_content(),
        }),
        Role::Assistant => {
            let mut v = json!({"role": "assistant", "content": m.text_content()});
            if !m.tool_calls.is_empty() {
                v["tool_calls"] = Value::Array(
                    m.tool_calls
                        .iter()
                        .map(|c| json!({"id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}))
                        .collect(),
                );
                if m.text_content().is_empty() {
                    v["content"] = Value::Null;
                }
            }
            v
        }
        Role::System | Role::User => {
            let role = if m.role == Role::System { "system" } else { "user" };
            if !m.has_images() {
                return json!({"role": role, "content": m.text_content()});
            }
            let parts: Vec<Value> = m
                .content
                .iter()
                .map(|p| match p {
                    ContentPart::Text { text } => json!({"type": "text", "text": text}),
                    ContentPart::Image { media_type, data, url } => {
                        let url = match (data, url) {
                            (Some(d), _) => format!("data:{};base64,{}", media_type.as_deref().unwrap_or("image/png"), d),
                            (None, Some(u)) => u.clone(),
                            _ => String::new(),
                        };
                        json!({"type": "image_url", "image_url": {"url": url}})
                    }
                })
                .collect();
            json!({"role": role, "content": parts})
        }
    }
}

fn parse_usage(u: &Value) -> Option<TokenUsage> {
    if !u.is_object() {
        return None;
    }
    let get = |v: &Value| v.as_u64();
    Some(TokenUsage {
        input_tokens: get(&u["prompt_tokens"]),
        output_tokens: get(&u["completion_tokens"]),
        cached_input_tokens: get(&u["prompt_tokens_details"]["cached_tokens"]).or_else(|| get(&u["prompt_cache_hit_tokens"])),
        cache_write_tokens: None,
        reasoning_tokens: get(&u["completion_tokens_details"]["reasoning_tokens"]),
        provenance: Provenance::Reported,
    })
}

#[derive(Default)]
struct PartialCall {
    id: String,
    name: String,
    args: String,
}

#[async_trait]
impl ProviderAdapter for OpenAiCompatAdapter {
    fn account(&self) -> &Account {
        &self.account
    }

    async fn verify(&self) -> HarnessResult<VerifiedIdentity> {
        let resp = self.request(reqwest::Method::GET, "/models").timeout(Duration::from_secs(20)).send().await.map_err(|e| {
            let mut err = transport_error(e);
            if self.account.descriptor().is_local {
                err.kind = ErrorKind::LocalDependency;
                err.message = format!("{} is not reachable at {}. Is it running?", self.account.descriptor().name, self.base_url);
            }
            err
        })?;
        if !resp.status().is_success() {
            return Err(error_from_response(resp).await);
        }
        let mut id = VerifiedIdentity::default();
        if self.account.kind == ProviderKind::OpenRouter {
            if let Ok(resp) = self.request(reqwest::Method::GET, "/key").send().await {
                if let Ok(v) = resp.json::<Value>().await {
                    id.identity = v["data"]["label"].as_str().map(str::to_string);
                    if v["data"]["is_free_tier"].as_bool() == Some(true) {
                        id.plan = Some("Free tier".into());
                    }
                    id.billing_mode = Some(BillingMode::Credits);
                }
            }
        }
        Ok(id)
    }

    async fn discover_models(&self) -> HarnessResult<Vec<DiscoveredModel>> {
        let resp = self.request(reqwest::Method::GET, "/models").timeout(Duration::from_secs(30)).send().await.map_err(transport_error)?;
        if !resp.status().is_success() {
            return Err(error_from_response(resp).await);
        }
        let v: Value = resp.json().await.map_err(transport_error)?;
        let empty = vec![];
        let data = v["data"].as_array().or_else(|| v["models"].as_array()).unwrap_or(&empty);
        let mut out = Vec::new();
        for m in data {
            let Some(id) = m["id"].as_str() else { continue };
            if !catalog::is_text_model(id) {
                continue;
            }
            // OpenAI Codex models require Responses, not Chat Completions.
            if self.account.kind == ProviderKind::OpenAi && id.contains("codex") {
                continue;
            }
            if m["active"] == Value::Bool(false) {
                continue;
            }
            if self.account.kind == ProviderKind::Mistral && m["capabilities"]["completion_chat"] == Value::Bool(false) {
                continue;
            }
            let mut d = catalog::discovered_from_catalog(self.profile.vendor, id, m["name"].as_str().map(str::to_string));
            d.description = m["description"].as_str().map(|s| s.chars().take(300).collect());
            let reported_ctx =
                m["context_length"].as_u64().or_else(|| m["context_window"].as_u64()).or_else(|| m["max_context_length"].as_u64());
            if let Some(ctx) = reported_ctx {
                d.context_window = Some(ctx);
                d.metadata_provenance = Provenance::Reported;
            }
            if let Some(max_out) = m["top_provider"]["max_completion_tokens"].as_u64() {
                d.max_output_tokens = Some(max_out);
            }
            // OpenRouter reports per-token USD prices and supported features.
            if let Some(p) = m.get("pricing") {
                let parse = |v: &Value| v.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| v.as_f64());
                if let (Some(i), Some(o)) = (parse(&p["prompt"]), parse(&p["completion"])) {
                    if i >= 0.0 && o >= 0.0 {
                        d.pricing = Some(Pricing {
                            input_per_mtok: i * 1e6,
                            output_per_mtok: o * 1e6,
                            cached_input_per_mtok: parse(&p["input_cache_read"]).map(|c| c * 1e6),
                            provenance: Provenance::Reported,
                        });
                    }
                }
            }
            if let Some(params) = m["supported_parameters"].as_array() {
                let has = |s: &str| params.iter().any(|p| p.as_str() == Some(s));
                d.capabilities.tools = has("tools");
                d.capabilities.structured_output = has("structured_outputs") || has("response_format");
                d.capabilities.reasoning = has("reasoning") || has("include_reasoning");
            }
            if let Some(mods) = m["architecture"]["input_modalities"].as_array() {
                d.capabilities.vision = mods.iter().any(|x| x.as_str() == Some("image"));
            }
            if let Some(caps) = m.get("capabilities").filter(|c| c.is_object()) {
                if let Some(b) = caps["function_calling"].as_bool() {
                    d.capabilities.tools = b;
                }
                if let Some(b) = caps["vision"].as_bool() {
                    d.capabilities.vision = b;
                }
            }
            out.push(d);
        }
        for extra in crate::extra_models(&self.account) {
            if !out.iter().any(|m| m.model_id == extra) {
                out.push(catalog::discovered_from_catalog(self.profile.vendor, &extra, None));
            }
        }
        out.sort_by(|a, b| a.model_id.cmp(&b.model_id));
        Ok(out)
    }

    async fn execute(&self, req: &AdapterRequest, events: EventSender, cancel: CancellationToken) -> HarnessResult<ProviderOutcome> {
        let body = self.build_body(req);
        let send = self.request(reqwest::Method::POST, "/chat/completions").json(&body).send();
        let resp = tokio::select! {
            r = tokio::time::timeout(req.timeout, send) => r
                .map_err(|_| HarnessError::new(ErrorKind::Timeout, "Provider did not respond in time"))?
                .map_err(transport_error)?,
            _ = cancel.cancelled() => return Err(HarnessError::cancelled()),
        };
        let mut limits = http::openai_rate_limits(&self.account.id, resp.headers());
        if !resp.status().is_success() {
            let mut err = error_from_response(resp).await;
            if err.kind == ErrorKind::RateLimited || err.kind == ErrorKind::QuotaExhausted {
                err.resets_at = limits.iter().filter_map(|l| l.resets_at).min();
            }
            return Err(err);
        }

        let started = Instant::now();
        let mut stream = Box::pin(http::sse_stream(resp, Duration::from_secs(180)));
        let mut outcome = ProviderOutcome::default();
        let mut calls: BTreeMap<u64, PartialCall> = BTreeMap::new();
        let mut finish: Option<String> = None;
        loop {
            let next = tokio::select! {
                n = stream.next() => n,
                _ = cancel.cancelled() => return Err(HarnessError::cancelled()),
            };
            let Some(ev) = next else { break };
            let ev = ev?;
            let data = ev.data.trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                break;
            }
            let chunk: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(err) = chunk.get("error").filter(|e| !e.is_null()) {
                let msg = err["message"].as_str().unwrap_or("Provider stream error");
                let code = err["code"].as_u64().unwrap_or(500) as u16;
                return Err(HarnessError::from_http_status(code, magpie_security::redact(msg)));
            }
            if outcome.resolved_model.is_none() {
                if let Some(m) = chunk["model"].as_str() {
                    outcome.resolved_model = Some(m.to_string());
                    emit(&events, ProviderEvent::ResolvedModel(m.to_string())).await?;
                }
            }
            if let Some(u) = parse_usage(&chunk["usage"]).or_else(|| parse_usage(&chunk["x_groq"]["usage"])) {
                outcome.usage.merge(&u);
                if let Some(cost) = chunk["usage"]["cost"].as_f64() {
                    outcome.cost = Some(Cost { usd: cost, provenance: Provenance::Reported, api_equivalent: false });
                }
            }
            let Some(choice) = chunk["choices"].as_array().and_then(|c| c.first()) else { continue };
            let delta = &choice["delta"];
            if let Some(t) = delta["content"].as_str().filter(|t| !t.is_empty()) {
                emit(&events, ProviderEvent::TextDelta(t.to_string())).await?;
            }
            for key in ["reasoning_content", "reasoning"] {
                if let Some(t) = delta[key].as_str().filter(|t| !t.is_empty()) {
                    emit(&events, ProviderEvent::ReasoningDelta(t.to_string())).await?;
                }
            }
            if let Some(tcs) = delta["tool_calls"].as_array() {
                for (pos, tc) in tcs.iter().enumerate() {
                    let idx = tc["index"].as_u64().unwrap_or(pos as u64);
                    let entry = calls.entry(idx).or_default();
                    if let Some(id) = tc["id"].as_str() {
                        entry.id = id.to_string();
                    }
                    if let Some(n) = tc["function"]["name"].as_str() {
                        entry.name.push_str(n);
                    }
                    if let Some(a) = tc["function"]["arguments"].as_str() {
                        entry.args.push_str(a);
                    }
                }
            }
            if let Some(f) = choice["finish_reason"].as_str() {
                finish = Some(f.to_string());
            }
        }
        for (i, c) in calls {
            let call = ToolCall {
                id: if c.id.is_empty() { format!("call_{i}") } else { c.id },
                name: c.name,
                arguments: if c.args.is_empty() { "{}".into() } else { c.args },
            };
            emit(&events, ProviderEvent::ToolCall(call)).await?;
        }
        outcome.finish_reason = match finish.as_deref() {
            Some("length") => FinishReason::Length,
            Some("tool_calls") | Some("function_call") => FinishReason::ToolCalls,
            Some("content_filter") => FinishReason::Refusal,
            _ => FinishReason::Stop,
        };
        tracing::debug!(elapsed_ms = started.elapsed().as_millis() as u64, "openai-compatible stream finished");
        outcome.limits.append(&mut limits);
        Ok(outcome)
    }

    async fn fetch_limits(&self) -> HarnessResult<Vec<LimitWindow>> {
        match self.account.kind {
            ProviderKind::OpenRouter => self.openrouter_credits().await,
            ProviderKind::DeepSeek => self.deepseek_balance().await,
            _ => Ok(vec![]),
        }
    }

    fn supports_limit_polling(&self) -> bool {
        matches!(self.account.kind, ProviderKind::OpenRouter | ProviderKind::DeepSeek)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(kind: ProviderKind) -> Account {
        Account {
            id: "acc".into(),
            kind,
            label: "t".into(),
            auth_method: AuthMethod::ApiKey,
            billing_mode: BillingMode::Metered,
            billing_reported: false,
            base_url: Some("http://localhost:1".into()),
            identity: None,
            plan: None,
            status: ConnectionStatus::Connected,
            status_message: None,
            enabled: true,
            last_verified_at: None,
            created_at: now(),
            has_secret: true,
            secret_store: None,
            options: json!({}),
        }
    }

    #[test]
    fn body_maps_tools_images_and_format() {
        let a = OpenAiCompatAdapter::new(account(ProviderKind::OpenAi), Some("sk-test".into())).unwrap();
        let mut r = ExecRequest::simple("hello");
        r.system = Some("be brief".into());
        r.max_output_tokens = Some(100);
        r.messages[0].content.push(ContentPart::Image { media_type: Some("image/png".into()), data: Some("AAAA".into()), url: None });
        r.tools.push(ToolDefinition { name: "lookup".into(), description: None, parameters: json!({"type":"object"}) });
        r.tool_choice = Some(ToolChoice::Required);
        r.response_format = Some(ResponseFormat::JsonSchema { name: "x".into(), schema: json!({"type":"object"}), strict: true });
        r.messages.push(Message {
            role: Role::Assistant,
            content: vec![],
            tool_call_id: None,
            tool_calls: vec![ToolCall { id: "c1".into(), name: "lookup".into(), arguments: "{}".into() }],
        });
        r.messages.push(Message {
            role: Role::Tool,
            content: vec![ContentPart::Text { text: "42".into() }],
            tool_call_id: Some("c1".into()),
            tool_calls: vec![],
        });
        let req = AdapterRequest {
            model_id: "gpt-x".into(),
            request: r,
            default_max_output: 1000,
            timeout: Duration::from_secs(5),
            scratch_dir: std::env::temp_dir(),
        };
        let b = a.build_body(&req);
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][1]["content"][1]["image_url"]["url"], "data:image/png;base64,AAAA");
        assert_eq!(b["max_completion_tokens"], 100);
        assert_eq!(b["tool_choice"], "required");
        assert_eq!(b["response_format"]["type"], "json_schema");
        assert_eq!(b["messages"][2]["tool_calls"][0]["id"], "c1");
        assert!(b["messages"][2]["content"].is_null());
        assert_eq!(b["messages"][3]["tool_call_id"], "c1");
        assert_eq!(b["stream_options"]["include_usage"], true);
    }

    #[test]
    fn requires_key_for_cloud_providers() {
        assert!(OpenAiCompatAdapter::new(account(ProviderKind::Groq), None).is_err());
        assert!(OpenAiCompatAdapter::new(account(ProviderKind::Ollama), None).is_ok());
    }
}
