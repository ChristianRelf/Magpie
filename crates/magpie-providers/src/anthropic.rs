//! Anthropic Messages API adapter (API-key authentication).

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use magpie_core::*;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::http::{self, error_from_response, transport_error};
use crate::{catalog, emit, AdapterRequest, EventSender, ProviderAdapter};

const API_VERSION: &str = "2023-06-01";

pub struct AnthropicAdapter {
    account: Account,
    key: String,
    base_url: String,
    http: reqwest::Client,
}

/// Models that reject sampling parameters (temperature/top_p).
fn rejects_sampling(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    ["fable", "mythos", "opus-5", "sonnet-5", "opus-4-7", "opus-4-8"].iter().any(|p| m.contains(p))
}

/// Models that reject forced tool choice (`any` / `tool`).
fn rejects_forced_tools(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    ["fable-5-1", "mythos-5-1", "opus-5-5"].iter().any(|p| m.contains(p))
}

/// Models that accept `output_config.effort`.
fn supports_effort(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    ["fable", "mythos", "opus-5", "sonnet-5", "opus-4-5", "opus-4-6", "opus-4-7", "opus-4-8", "sonnet-4-6"]
        .iter()
        .any(|p| m.contains(p))
}

impl AnthropicAdapter {
    pub fn new(account: Account, key: Option<String>) -> HarnessResult<Self> {
        let key = key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| HarnessError::new(ErrorKind::Authentication, "An Anthropic API key is required"))?;
        let base_url = http::base(&account.base_url().unwrap_or_else(|| "https://api.anthropic.com".into()));
        Ok(Self { account, key, base_url, http: http::client() })
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .header("x-api-key", &self.key)
            .header("anthropic-version", API_VERSION)
    }

    pub(crate) fn build_body(&self, req: &AdapterRequest) -> Value {
        let r = &req.request;
        let model = req.model_id.as_str();
        let mut messages: Vec<Value> = Vec::new();
        let mut system_parts: Vec<String> = r.system.iter().cloned().collect();

        for m in &r.messages {
            match m.role {
                Role::System => system_parts.push(m.text_content()),
                Role::User => push_merged(&mut messages, "user", user_blocks(m)),
                Role::Assistant => {
                    let mut blocks = Vec::new();
                    let text = m.text_content();
                    if !text.is_empty() {
                        blocks.push(json!({"type": "text", "text": text}));
                    }
                    for c in &m.tool_calls {
                        let input: Value = serde_json::from_str(&c.arguments).unwrap_or_else(|_| json!({}));
                        blocks.push(json!({"type": "tool_use", "id": c.id, "name": c.name, "input": input}));
                    }
                    if !blocks.is_empty() {
                        push_merged(&mut messages, "assistant", blocks);
                    }
                }
                Role::Tool => push_merged(
                    &mut messages,
                    "user",
                    vec![json!({
                        "type": "tool_result",
                        "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                        "content": m.text_content(),
                    })],
                ),
            }
        }

        let max_tokens = r.max_output_tokens.unwrap_or(req.default_max_output);
        let mut body = json!({
            "model": model,
            "max_tokens": max_tokens,
            "messages": messages,
            "stream": true,
        });
        let obj = body.as_object_mut().unwrap();
        if !system_parts.is_empty() {
            obj.insert("system".into(), json!(system_parts.join("\n\n")));
        }
        if let Some(t) = r.temperature {
            if !rejects_sampling(model) {
                obj.insert("temperature".into(), json!(t.min(1.0)));
            }
        }
        let mut output_config = serde_json::Map::new();
        if let Some(effort) = &r.reasoning_effort {
            if supports_effort(model) {
                output_config.insert("effort".into(), json!(effort));
            }
        }
        if let Some(ResponseFormat::JsonSchema { schema, .. }) = &r.response_format {
            output_config.insert("format".into(), json!({"type": "json_schema", "schema": schema}));
        }
        if !output_config.is_empty() {
            obj.insert("output_config".into(), Value::Object(output_config));
        }
        if !r.tools.is_empty() {
            obj.insert(
                "tools".into(),
                Value::Array(
                    r.tools
                        .iter()
                        .map(|t| json!({"name": t.name, "description": t.description.clone().unwrap_or_default(), "input_schema": t.parameters}))
                        .collect(),
                ),
            );
            if let Some(tc) = &r.tool_choice {
                let forced_ok = !rejects_forced_tools(model);
                let choice = match tc {
                    ToolChoice::Auto => json!({"type": "auto"}),
                    ToolChoice::None => json!({"type": "none"}),
                    ToolChoice::Required if forced_ok => json!({"type": "any"}),
                    ToolChoice::Tool { name } if forced_ok => json!({"type": "tool", "name": name}),
                    // Forced tool use is rejected by these models; fall back
                    // to automatic selection rather than failing the request.
                    _ => json!({"type": "auto"}),
                };
                obj.insert("tool_choice".into(), choice);
            }
        }
        body
    }
}

fn user_blocks(m: &Message) -> Vec<Value> {
    m.content
        .iter()
        .map(|p| match p {
            ContentPart::Text { text } => json!({"type": "text", "text": text}),
            ContentPart::Image { media_type, data, url } => match (data, url) {
                (Some(d), _) => json!({"type": "image", "source": {
                    "type": "base64", "media_type": media_type.clone().unwrap_or_else(|| "image/png".into()), "data": d}}),
                (None, Some(u)) => json!({"type": "image", "source": {"type": "url", "url": u}}),
                _ => json!({"type": "text", "text": ""}),
            },
        })
        .collect()
}

/// Append content blocks, merging consecutive messages of the same role
/// (the Messages API requires alternation; tool results share a user turn).
fn push_merged(messages: &mut Vec<Value>, role: &str, blocks: Vec<Value>) {
    if let Some(last) = messages.last_mut() {
        if last["role"] == role {
            if let Some(arr) = last["content"].as_array_mut() {
                arr.extend(blocks);
                return;
            }
        }
    }
    messages.push(json!({"role": role, "content": blocks}));
}

fn usage_from(u: &Value) -> TokenUsage {
    let input = u["input_tokens"].as_u64();
    let cache_read = u["cache_read_input_tokens"].as_u64();
    let cache_write = u["cache_creation_input_tokens"].as_u64();
    // `input_tokens` excludes cache reads/writes; report total input.
    let total_input = input.map(|i| i + cache_read.unwrap_or(0) + cache_write.unwrap_or(0));
    TokenUsage {
        input_tokens: total_input,
        output_tokens: u["output_tokens"].as_u64(),
        cached_input_tokens: cache_read,
        cache_write_tokens: cache_write,
        reasoning_tokens: None,
        provenance: Provenance::Reported,
    }
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    fn account(&self) -> &Account {
        &self.account
    }

    async fn verify(&self) -> HarnessResult<VerifiedIdentity> {
        let resp = self
            .req(reqwest::Method::GET, "/v1/models?limit=1")
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(transport_error)?;
        if !resp.status().is_success() {
            return Err(error_from_response(resp).await);
        }
        Ok(VerifiedIdentity { billing_mode: Some(BillingMode::Metered), ..Default::default() })
    }

    async fn discover_models(&self) -> HarnessResult<Vec<DiscoveredModel>> {
        let mut out = Vec::new();
        let mut after: Option<String> = None;
        for _ in 0..10 {
            let mut path = "/v1/models?limit=100".to_string();
            if let Some(a) = &after {
                path.push_str(&format!("&after_id={a}"));
            }
            let resp = self.req(reqwest::Method::GET, &path).send().await.map_err(transport_error)?;
            if !resp.status().is_success() {
                return Err(error_from_response(resp).await);
            }
            let v: Value = resp.json().await.map_err(transport_error)?;
            for m in v["data"].as_array().cloned().unwrap_or_default() {
                let Some(id) = m["id"].as_str() else { continue };
                let mut d = catalog::discovered_from_catalog("anthropic", id, m["display_name"].as_str().map(str::to_string));
                if let Some(ctx) = m["max_input_tokens"].as_u64() {
                    d.context_window = Some(ctx);
                    d.metadata_provenance = Provenance::Reported;
                }
                if let Some(mx) = m["max_tokens"].as_u64() {
                    d.max_output_tokens = Some(mx);
                }
                let caps = &m["capabilities"];
                if caps.is_object() {
                    let sup = |path: &[&str]| {
                        let mut c = caps;
                        for p in path {
                            c = &c[*p];
                        }
                        c["supported"].as_bool()
                    };
                    if let Some(b) = sup(&["image_input"]) {
                        d.capabilities.vision = b;
                    }
                    if let Some(b) = sup(&["structured_outputs"]) {
                        d.capabilities.structured_output = b;
                    }
                    if let Some(b) = sup(&["thinking"]) {
                        d.capabilities.reasoning = b;
                    }
                    if let Some(effort) = caps.get("effort").and_then(|e| e.as_object()) {
                        d.reasoning_efforts = effort
                            .iter()
                            .filter(|(k, v)| *k != "supported" && v["supported"].as_bool() == Some(true))
                            .map(|(k, _)| k.clone())
                            .collect();
                    }
                }
                out.push(d);
            }
            if v["has_more"].as_bool() == Some(true) {
                after = v["last_id"].as_str().map(str::to_string);
                if after.is_none() {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(out)
    }

    async fn execute(&self, req: &AdapterRequest, events: EventSender, cancel: CancellationToken) -> HarnessResult<ProviderOutcome> {
        let body = self.build_body(req);
        let send = self.req(reqwest::Method::POST, "/v1/messages").json(&body).send();
        let resp = tokio::select! {
            r = tokio::time::timeout(req.timeout, send) => r
                .map_err(|_| HarnessError::new(ErrorKind::Timeout, "Provider did not respond in time"))?
                .map_err(transport_error)?,
            _ = cancel.cancelled() => return Err(HarnessError::cancelled()),
        };
        let limits = http::anthropic_rate_limits(&self.account.id, &req.model_id, resp.headers());
        if !resp.status().is_success() {
            let mut err = error_from_response(resp).await;
            if err.kind == ErrorKind::RateLimited {
                err.resets_at = limits.iter().filter(|l| l.remaining == Some(0.0)).filter_map(|l| l.resets_at).min();
            }
            return Err(err);
        }

        let mut outcome = ProviderOutcome { limits, ..Default::default() };
        let mut stream = Box::pin(http::sse_stream(resp, Duration::from_secs(300)));
        // index -> (id, name, partial json)
        let mut tool_blocks: HashMap<u64, (String, String, String)> = HashMap::new();
        let mut stop_reason: Option<String> = None;
        loop {
            let next = tokio::select! {
                n = stream.next() => n,
                _ = cancel.cancelled() => return Err(HarnessError::cancelled()),
            };
            let Some(ev) = next else { break };
            let ev = ev?;
            let v: Value = match serde_json::from_str(&ev.data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            match v["type"].as_str().unwrap_or_default() {
                "message_start" => {
                    let msg = &v["message"];
                    if let Some(m) = msg["model"].as_str() {
                        outcome.resolved_model = Some(m.to_string());
                        emit(&events, ProviderEvent::ResolvedModel(m.to_string())).await?;
                    }
                    outcome.usage.merge(&usage_from(&msg["usage"]));
                }
                "content_block_start" => {
                    let idx = v["index"].as_u64().unwrap_or(0);
                    let block = &v["content_block"];
                    if block["type"] == "tool_use" {
                        tool_blocks.insert(
                            idx,
                            (block["id"].as_str().unwrap_or_default().into(), block["name"].as_str().unwrap_or_default().into(), String::new()),
                        );
                    }
                }
                "content_block_delta" => {
                    let idx = v["index"].as_u64().unwrap_or(0);
                    let d = &v["delta"];
                    match d["type"].as_str().unwrap_or_default() {
                        "text_delta" => {
                            if let Some(t) = d["text"].as_str() {
                                emit(&events, ProviderEvent::TextDelta(t.to_string())).await?;
                            }
                        }
                        "thinking_delta" => {
                            if let Some(t) = d["thinking"].as_str().filter(|t| !t.is_empty()) {
                                emit(&events, ProviderEvent::ReasoningDelta(t.to_string())).await?;
                            }
                        }
                        "input_json_delta" => {
                            if let (Some(entry), Some(p)) = (tool_blocks.get_mut(&idx), d["partial_json"].as_str()) {
                                entry.2.push_str(p);
                            }
                        }
                        _ => {}
                    }
                }
                "content_block_stop" => {
                    let idx = v["index"].as_u64().unwrap_or(0);
                    if let Some((id, name, args)) = tool_blocks.remove(&idx) {
                        let arguments = if args.trim().is_empty() { "{}".to_string() } else { args };
                        emit(&events, ProviderEvent::ToolCall(ToolCall { id, name, arguments })).await?;
                    }
                }
                "message_delta" => {
                    if let Some(s) = v["delta"]["stop_reason"].as_str() {
                        stop_reason = Some(s.to_string());
                    }
                    if let Some(out) = v["usage"]["output_tokens"].as_u64() {
                        outcome.usage.output_tokens = Some(out);
                        outcome.usage.provenance = Provenance::Reported;
                    }
                }
                "error" => {
                    let e = &v["error"];
                    let ty = e["type"].as_str().unwrap_or_default();
                    let msg = magpie_security::redact(e["message"].as_str().unwrap_or("Stream error"));
                    let kind = match ty {
                        "overloaded_error" | "api_error" => ErrorKind::ProviderUnavailable,
                        "rate_limit_error" => ErrorKind::RateLimited,
                        "authentication_error" => ErrorKind::Authentication,
                        "permission_error" => ErrorKind::PermissionDenied,
                        "invalid_request_error" => ErrorKind::InvalidRequest,
                        _ => ErrorKind::ProviderUnavailable,
                    };
                    return Err(HarnessError::new(kind, msg));
                }
                "message_stop" => break,
                _ => {}
            }
        }
        outcome.finish_reason = match stop_reason.as_deref() {
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            Some("refusal") => FinishReason::Refusal,
            _ => FinishReason::Stop,
        };
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> AnthropicAdapter {
        let account = Account {
            id: "acc".into(),
            kind: ProviderKind::Anthropic,
            label: "t".into(),
            auth_method: AuthMethod::ApiKey,
            billing_mode: BillingMode::Metered,
            billing_reported: false,
            base_url: None,
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
        };
        AnthropicAdapter::new(account, Some("sk-ant-test".into())).unwrap()
    }

    fn areq(model: &str, r: ExecRequest) -> AdapterRequest {
        AdapterRequest { model_id: model.into(), request: r, default_max_output: 16000, timeout: Duration::from_secs(5), scratch_dir: std::env::temp_dir() }
    }

    #[test]
    fn merges_tool_results_into_single_user_turn() {
        let mut r = ExecRequest::simple("weather?");
        r.messages.push(Message {
            role: Role::Assistant,
            content: vec![],
            tool_call_id: None,
            tool_calls: vec![
                ToolCall { id: "t1".into(), name: "w".into(), arguments: r#"{"city":"Paris"}"#.into() },
                ToolCall { id: "t2".into(), name: "w".into(), arguments: r#"{"city":"Rome"}"#.into() },
            ],
        });
        r.messages.push(Message { role: Role::Tool, content: vec![ContentPart::Text { text: "sunny".into() }], tool_call_id: Some("t1".into()), tool_calls: vec![] });
        r.messages.push(Message { role: Role::Tool, content: vec![ContentPart::Text { text: "rain".into() }], tool_call_id: Some("t2".into()), tool_calls: vec![] });
        let b = adapter().build_body(&areq("claude-sonnet-5", r));
        let msgs = b["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["content"][0]["input"]["city"], "Paris");
        assert_eq!(msgs[2]["content"].as_array().unwrap().len(), 2);
        assert_eq!(msgs[2]["content"][1]["tool_use_id"], "t2");
    }

    #[test]
    fn negotiates_model_specific_parameters() {
        let mut r = ExecRequest::simple("x");
        r.temperature = Some(0.2);
        r.reasoning_effort = Some("low".into());
        r.tools.push(ToolDefinition { name: "f".into(), description: None, parameters: json!({"type":"object"}) });
        r.tool_choice = Some(ToolChoice::Required);
        let b = adapter().build_body(&areq("claude-opus-5-5", r.clone()));
        assert!(b.get("temperature").is_none());
        assert_eq!(b["tool_choice"]["type"], "auto");
        assert_eq!(b["output_config"]["effort"], "low");

        let b = adapter().build_body(&areq("claude-haiku-4-5", r));
        assert_eq!(b["temperature"], json!(0.2f32));
        assert_eq!(b["tool_choice"]["type"], "any");
        assert!(b.get("output_config").is_none());
    }

    #[test]
    fn usage_includes_cache_tokens() {
        let u = usage_from(&json!({"input_tokens": 10, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 5, "output_tokens": 3}));
        assert_eq!(u.input_tokens, Some(115));
        assert_eq!(u.cached_input_tokens, Some(100));
    }
}
