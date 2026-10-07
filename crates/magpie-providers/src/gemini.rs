//! Google Gemini API adapter (AI Studio API keys).

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use magpie_core::*;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::http::{self, error_from_response, transport_error};
use crate::{catalog, emit, AdapterRequest, EventSender, ProviderAdapter};

pub struct GeminiAdapter {
    account: Account,
    key: String,
    base_url: String,
    http: reqwest::Client,
}

impl GeminiAdapter {
    pub fn new(account: Account, key: Option<String>) -> HarnessResult<Self> {
        let key = key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| HarnessError::new(ErrorKind::Authentication, "A Gemini API key is required"))?;
        let base_url = http::base(&account.base_url().unwrap_or_else(|| "https://generativelanguage.googleapis.com".into()));
        Ok(Self { account, key, base_url, http: http::client() })
    }

    fn req(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http.request(method, format!("{}{}", self.base_url, path)).header("x-goog-api-key", &self.key)
    }

    pub(crate) fn build_body(&self, req: &AdapterRequest) -> Value {
        let r = &req.request;
        // Gemini function responses are keyed by function name, so map call ids.
        let mut call_names: HashMap<String, String> = HashMap::new();
        let mut contents: Vec<Value> = Vec::new();
        let mut system_parts: Vec<String> = r.system.iter().cloned().collect();
        for m in &r.messages {
            match m.role {
                Role::System => system_parts.push(m.text_content()),
                Role::User => push(&mut contents, "user", parts_for(m)),
                Role::Assistant => {
                    let mut parts = Vec::new();
                    let t = m.text_content();
                    if !t.is_empty() {
                        parts.push(json!({"text": t}));
                    }
                    for c in &m.tool_calls {
                        call_names.insert(c.id.clone(), c.name.clone());
                        let args: Value = serde_json::from_str(&c.arguments).unwrap_or_else(|_| json!({}));
                        parts.push(json!({"functionCall": {"name": c.name, "args": args}}));
                    }
                    if !parts.is_empty() {
                        push(&mut contents, "model", parts);
                    }
                }
                Role::Tool => {
                    let id = m.tool_call_id.clone().unwrap_or_default();
                    let name = call_names.get(&id).cloned().unwrap_or_else(|| id.clone());
                    let text = m.text_content();
                    let response = serde_json::from_str::<Value>(&text)
                        .ok()
                        .filter(|v| v.is_object())
                        .unwrap_or_else(|| json!({"result": text}));
                    push(&mut contents, "user", vec![json!({"functionResponse": {"name": name, "response": response}})]);
                }
            }
        }
        let mut body = json!({"contents": contents});
        let obj = body.as_object_mut().unwrap();
        if !system_parts.is_empty() {
            obj.insert("systemInstruction".into(), json!({"parts": [{"text": system_parts.join("\n\n")}]}));
        }
        let mut gen = serde_json::Map::new();
        if let Some(m) = r.max_output_tokens {
            gen.insert("maxOutputTokens".into(), json!(m));
        }
        if let Some(t) = r.temperature {
            gen.insert("temperature".into(), json!(t));
        }
        match &r.response_format {
            Some(ResponseFormat::JsonObject) => {
                gen.insert("responseMimeType".into(), json!("application/json"));
            }
            Some(ResponseFormat::JsonSchema { schema, .. }) => {
                gen.insert("responseMimeType".into(), json!("application/json"));
                gen.insert("responseJsonSchema".into(), schema.clone());
            }
            _ => {}
        }
        if !gen.is_empty() {
            obj.insert("generationConfig".into(), Value::Object(gen));
        }
        if !r.tools.is_empty() {
            let decls: Vec<Value> = r
                .tools
                .iter()
                .map(|t| json!({"name": t.name, "description": t.description.clone().unwrap_or_default(), "parametersJsonSchema": t.parameters}))
                .collect();
            obj.insert("tools".into(), json!([{"functionDeclarations": decls}]));
            if let Some(tc) = &r.tool_choice {
                let cfg = match tc {
                    ToolChoice::Auto => json!({"mode": "AUTO"}),
                    ToolChoice::None => json!({"mode": "NONE"}),
                    ToolChoice::Required => json!({"mode": "ANY"}),
                    ToolChoice::Tool { name } => json!({"mode": "ANY", "allowedFunctionNames": [name]}),
                };
                obj.insert("toolConfig".into(), json!({"functionCallingConfig": cfg}));
            }
        }
        body
    }
}

fn parts_for(m: &Message) -> Vec<Value> {
    m.content
        .iter()
        .map(|p| match p {
            ContentPart::Text { text } => json!({"text": text}),
            ContentPart::Image { media_type, data, url } => match (data, url) {
                (Some(d), _) => json!({"inlineData": {"mimeType": media_type.clone().unwrap_or_else(|| "image/png".into()), "data": d}}),
                (None, Some(u)) => json!({"fileData": {"mimeType": media_type.clone().unwrap_or_else(|| "image/png".into()), "fileUri": u}}),
                _ => json!({"text": ""}),
            },
        })
        .collect()
}

fn push(contents: &mut Vec<Value>, role: &str, parts: Vec<Value>) {
    if let Some(last) = contents.last_mut() {
        if last["role"] == role {
            if let Some(arr) = last["parts"].as_array_mut() {
                arr.extend(parts);
                return;
            }
        }
    }
    contents.push(json!({"role": role, "parts": parts}));
}

fn usage_from(u: &Value) -> Option<TokenUsage> {
    if !u.is_object() {
        return None;
    }
    let thoughts = u["thoughtsTokenCount"].as_u64();
    Some(TokenUsage {
        input_tokens: u["promptTokenCount"].as_u64(),
        // Gemini reports thinking tokens separately from candidates.
        output_tokens: u["candidatesTokenCount"].as_u64().map(|c| c + thoughts.unwrap_or(0)).or(thoughts),
        cached_input_tokens: u["cachedContentTokenCount"].as_u64(),
        cache_write_tokens: None,
        reasoning_tokens: thoughts,
        provenance: Provenance::Reported,
    })
}

#[async_trait]
impl ProviderAdapter for GeminiAdapter {
    fn account(&self) -> &Account {
        &self.account
    }

    async fn verify(&self) -> HarnessResult<VerifiedIdentity> {
        let resp = self
            .req(reqwest::Method::GET, "/v1beta/models?pageSize=1")
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(transport_error)?;
        if !resp.status().is_success() {
            return Err(error_from_response(resp).await);
        }
        Ok(VerifiedIdentity::default())
    }

    async fn discover_models(&self) -> HarnessResult<Vec<DiscoveredModel>> {
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        for _ in 0..10 {
            let mut path = "/v1beta/models?pageSize=1000".to_string();
            if let Some(p) = &page {
                path.push_str(&format!("&pageToken={p}"));
            }
            let resp = self.req(reqwest::Method::GET, &path).send().await.map_err(transport_error)?;
            if !resp.status().is_success() {
                return Err(error_from_response(resp).await);
            }
            let v: Value = resp.json().await.map_err(transport_error)?;
            for m in v["models"].as_array().cloned().unwrap_or_default() {
                let Some(name) = m["name"].as_str() else { continue };
                let id = name.trim_start_matches("models/");
                let methods = m["supportedGenerationMethods"].as_array().cloned().unwrap_or_default();
                if !methods.iter().any(|x| x == "generateContent") || !catalog::is_text_model(id) || id.contains("aqa") {
                    continue;
                }
                let mut d = catalog::discovered_from_catalog("google", id, m["displayName"].as_str().map(str::to_string));
                d.description = m["description"].as_str().map(|s| s.chars().take(300).collect());
                if let Some(c) = m["inputTokenLimit"].as_u64() {
                    d.context_window = Some(c);
                    d.metadata_provenance = Provenance::Reported;
                }
                if let Some(o) = m["outputTokenLimit"].as_u64() {
                    d.max_output_tokens = Some(o);
                }
                if let Some(t) = m["thinking"].as_bool() {
                    d.capabilities.reasoning = t;
                }
                out.push(d);
            }
            page = v["nextPageToken"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
            if page.is_none() {
                break;
            }
        }
        Ok(out)
    }

    async fn execute(&self, req: &AdapterRequest, events: EventSender, cancel: CancellationToken) -> HarnessResult<ProviderOutcome> {
        let body = self.build_body(req);
        let path = format!("/v1beta/models/{}:streamGenerateContent?alt=sse", req.model_id);
        let send = self.req(reqwest::Method::POST, &path).json(&body).send();
        let resp = tokio::select! {
            r = tokio::time::timeout(req.timeout, send) => r
                .map_err(|_| HarnessError::new(ErrorKind::Timeout, "Provider did not respond in time"))?
                .map_err(transport_error)?,
            _ = cancel.cancelled() => return Err(HarnessError::cancelled()),
        };
        if !resp.status().is_success() {
            return Err(error_from_response(resp).await);
        }
        let mut outcome = ProviderOutcome::default();
        let mut stream = Box::pin(http::sse_stream(resp, Duration::from_secs(300)));
        let mut finish: Option<String> = None;
        let mut call_n = 0;
        let mut saw_call = false;
        loop {
            let next = tokio::select! {
                n = stream.next() => n,
                _ = cancel.cancelled() => return Err(HarnessError::cancelled()),
            };
            let Some(ev) = next else { break };
            let v: Value = match serde_json::from_str(&ev?.data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(e) = v.get("error") {
                let code = e["code"].as_u64().unwrap_or(500) as u16;
                return Err(HarnessError::from_http_status(code, magpie_security::redact(e["message"].as_str().unwrap_or("Stream error"))));
            }
            if outcome.resolved_model.is_none() {
                if let Some(m) = v["modelVersion"].as_str() {
                    outcome.resolved_model = Some(m.to_string());
                    emit(&events, ProviderEvent::ResolvedModel(m.to_string())).await?;
                }
            }
            if let Some(u) = usage_from(&v["usageMetadata"]) {
                outcome.usage.merge(&u);
            }
            if let Some(reason) = v["promptFeedback"]["blockReason"].as_str() {
                return Err(HarnessError::new(ErrorKind::Refused, format!("Prompt blocked by provider: {reason}")));
            }
            let Some(cand) = v["candidates"].as_array().and_then(|c| c.first()) else { continue };
            for part in cand["content"]["parts"].as_array().cloned().unwrap_or_default() {
                if let Some(t) = part["text"].as_str() {
                    if part["thought"].as_bool() == Some(true) {
                        emit(&events, ProviderEvent::ReasoningDelta(t.to_string())).await?;
                    } else if !t.is_empty() {
                        emit(&events, ProviderEvent::TextDelta(t.to_string())).await?;
                    }
                }
                if let Some(fc) = part.get("functionCall") {
                    call_n += 1;
                    saw_call = true;
                    let id = fc["id"].as_str().map(str::to_string).unwrap_or_else(|| format!("call_{call_n}"));
                    let call = ToolCall {
                        id,
                        name: fc["name"].as_str().unwrap_or_default().to_string(),
                        arguments: fc["args"].to_string(),
                    };
                    emit(&events, ProviderEvent::ToolCall(call)).await?;
                }
            }
            if let Some(f) = cand["finishReason"].as_str() {
                finish = Some(f.to_string());
            }
        }
        outcome.finish_reason = match finish.as_deref() {
            _ if saw_call => FinishReason::ToolCalls,
            Some("MAX_TOKENS") => FinishReason::Length,
            Some("SAFETY") | Some("RECITATION") | Some("BLOCKLIST") | Some("PROHIBITED_CONTENT") | Some("SPII") => FinishReason::Refusal,
            _ => FinishReason::Stop,
        };
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_function_responses_by_name() {
        let account = Account {
            id: "acc".into(),
            kind: ProviderKind::Gemini,
            label: "t".into(),
            auth_method: AuthMethod::ApiKey,
            billing_mode: BillingMode::Unknown,
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
        let a = GeminiAdapter::new(account, Some("AIza-test".into())).unwrap();
        let mut r = ExecRequest::simple("weather?");
        r.system = Some("sys".into());
        r.messages.push(Message {
            role: Role::Assistant,
            content: vec![],
            tool_call_id: None,
            tool_calls: vec![ToolCall { id: "c9".into(), name: "get_weather".into(), arguments: r#"{"city":"Oslo"}"#.into() }],
        });
        r.messages.push(Message { role: Role::Tool, content: vec![ContentPart::Text { text: "cold".into() }], tool_call_id: Some("c9".into()), tool_calls: vec![] });
        r.response_format = Some(ResponseFormat::JsonObject);
        let req = AdapterRequest { model_id: "gemini-x".into(), request: r, default_max_output: 1000, timeout: Duration::from_secs(5), scratch_dir: std::env::temp_dir() };
        let b = a.build_body(&req);
        assert_eq!(b["systemInstruction"]["parts"][0]["text"], "sys");
        assert_eq!(b["contents"][1]["role"], "model");
        assert_eq!(b["contents"][2]["parts"][0]["functionResponse"]["name"], "get_weather");
        assert_eq!(b["contents"][2]["parts"][0]["functionResponse"]["response"]["result"], "cold");
        assert_eq!(b["generationConfig"]["responseMimeType"], "application/json");
    }

    #[test]
    fn usage_counts_thoughts_as_output() {
        let u = usage_from(&json!({"promptTokenCount": 10, "candidatesTokenCount": 5, "thoughtsTokenCount": 7})).unwrap();
        assert_eq!(u.output_tokens, Some(12));
        assert_eq!(u.reasoning_tokens, Some(7));
    }
}
