//! OpenAI-compatible surface: `POST /v1/chat/completions` and
//! `GET /v1/models`. Tools that let users set a custom base URL and API key
//! (most OpenAI SDK based tools) can use the harness unchanged; `"auto"`
//! selects a model through the router.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use magpie_core::*;
use magpie_engine::Scope;
use serde_json::{json, Value};

use crate::auth::Auth;
use crate::convert::{parse_message, parse_response_format, parse_tool_choice, parse_tools, task_from, MagpieExt};
use crate::error::{ApiError, ApiResult};
use crate::AppState;

pub async fn list_models(State(s): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    auth.require(Scope::Read)?;
    let models = s.harness.list_models();
    let mut data = vec![json!({
        "id": "auto",
        "object": "model",
        "created": s.harness.started_at.timestamp(),
        "owned_by": "magpie",
        "magpie": {"description": "Routed automatically by Magpie"},
    })];
    for m in &models {
        data.push(json!({
            "id": m.public_id(),
            "object": "model",
            "created": m.discovered_at.timestamp(),
            "owned_by": m.provider.as_str(),
            "magpie": m,
        }));
    }
    Ok(Json(json!({"object": "list", "data": data})))
}

fn finish_str(f: FinishReason) -> &'static str {
    match f {
        FinishReason::Stop | FinishReason::Cancelled | FinishReason::Error => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::Refusal => "content_filter",
    }
}

fn usage_json(u: &TokenUsage) -> Value {
    let input = u.input_tokens.unwrap_or(0);
    let output = u.output_tokens.unwrap_or(0);
    json!({
        "prompt_tokens": input,
        "completion_tokens": output,
        "total_tokens": input + output,
        "prompt_tokens_details": {"cached_tokens": u.cached_input_tokens.unwrap_or(0)},
        "completion_tokens_details": {"reasoning_tokens": u.reasoning_tokens.unwrap_or(0)},
        "magpie_provenance": u.provenance,
    })
}

fn tool_calls_json(calls: &[ToolCall]) -> Value {
    Value::Array(
        calls
            .iter()
            .enumerate()
            .map(|(i, c)| json!({"index": i, "id": c.id, "type": "function", "function": {"name": c.name, "arguments": c.arguments}}))
            .collect(),
    )
}

pub async fn chat_completions(State(s): State<AppState>, auth: Auth, Json(body): Json<Value>) -> ApiResult<Response> {
    auth.require(Scope::Execute)?;
    let messages = body["messages"].as_array().ok_or_else(|| ApiError::bad_request("`messages` is required"))?;
    let parsed: Vec<Message> = messages.iter().map(parse_message).collect::<Result<_, _>>()?;
    // Leading system messages become the system prompt.
    let mut system_parts = Vec::new();
    let mut rest = Vec::new();
    for m in parsed {
        if m.role == Role::System && rest.is_empty() {
            system_parts.push(m.text_content());
        } else {
            rest.push(m);
        }
    }
    let ext: MagpieExt = serde_json::from_value(body.get("magpie").cloned().unwrap_or(Value::Null)).unwrap_or_default();
    let stream = body["stream"].as_bool().unwrap_or(false);
    let include_usage = body["stream_options"]["include_usage"].as_bool().unwrap_or(false);
    let req = ExecRequest {
        model: body["model"].as_str().unwrap_or("auto").to_string(),
        task_type: task_from(ext.task_type.as_deref())?,
        system: (!system_parts.is_empty()).then(|| system_parts.join("\n\n")),
        messages: rest,
        tools: parse_tools(&body["tools"])?,
        tool_choice: parse_tool_choice(&body["tool_choice"]),
        response_format: parse_response_format(&body["response_format"]),
        max_output_tokens: body["max_completion_tokens"]
            .as_u64()
            .or_else(|| body["max_tokens"].as_u64())
            .map(|v| v.min(u32::MAX as u64) as u32),
        temperature: body["temperature"].as_f64().map(|t| t as f32),
        reasoning_effort: body["reasoning_effort"].as_str().map(str::to_string),
        stream,
        preferences: ext.preferences.unwrap_or_default(),
        agent: ext.agent,
        metadata: body.get("metadata").cloned(),
    };
    if req.agent.is_some() {
        auth.require(Scope::Agent)?;
    }
    let client = auth.0.name();
    let created = now().timestamp();

    if !stream {
        let (r, notable) = s.harness.execute_collect(req, client).await?;
        let mut message = json!({"role": "assistant", "content": r.output_text});
        if !r.tool_calls.is_empty() {
            message["tool_calls"] = tool_calls_json(&r.tool_calls);
        }
        let reasons = notable.iter().find_map(|e| match e {
            ExecEvent::Started { reasons, task, .. } => Some(json!({"task": task, "reasons": reasons})),
            _ => None,
        });
        return Ok(Json(json!({
            "id": format!("chatcmpl-{}", r.execution_id),
            "object": "chat.completion",
            "created": created,
            "model": format!("{}/{}", r.model.provider.as_str(), r.model.model_id),
            "choices": [{"index": 0, "message": message, "finish_reason": finish_str(r.finish_reason)}],
            "usage": usage_json(&r.usage),
            "magpie": {"execution_id": r.execution_id, "model": r.model, "cost": r.cost, "routing": reasons, "attempts": r.attempts},
        }))
        .into_response());
    }

    let handle = s.harness.execute(req, client).await?;
    let id = format!("chatcmpl-{}", handle.id);
    let mut model_name = String::from("auto");
    let mut tool_index = 0usize;
    let events = tokio_stream::wrappers::ReceiverStream::new(handle.events).flat_map(move |ev| {
        let chunk = |model: &str, delta: Value, finish: Option<&str>| {
            json!({"id": id, "object": "chat.completion.chunk", "created": created, "model": model,
                   "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]})
        };
        let mut out: Vec<String> = Vec::new();
        match ev {
            ExecEvent::Started { model, .. } => {
                model_name = format!("{}/{}", model.provider.as_str(), model.model_id);
                out.push(chunk(&model_name, json!({"role": "assistant", "content": ""}), None).to_string());
            }
            ExecEvent::RoutingChanged { to, .. } => {
                model_name = format!("{}/{}", to.provider.as_str(), to.model_id);
            }
            ExecEvent::TextDelta { text } => out.push(chunk(&model_name, json!({"content": text}), None).to_string()),
            ExecEvent::ReasoningDelta { text } => out.push(chunk(&model_name, json!({"reasoning_content": text}), None).to_string()),
            ExecEvent::ToolCall { call } => {
                let delta = json!({"tool_calls": [{"index": tool_index, "id": call.id, "type": "function",
                    "function": {"name": call.name, "arguments": call.arguments}}]});
                tool_index += 1;
                out.push(chunk(&model_name, delta, None).to_string());
            }
            ExecEvent::Completed { result } => {
                out.push(chunk(&model_name, json!({}), Some(finish_str(result.finish_reason))).to_string());
                if include_usage {
                    let mut u = json!({"id": id, "object": "chat.completion.chunk", "created": created, "model": model_name,
                        "choices": [], "usage": usage_json(&result.usage)});
                    u["magpie"] = json!({"execution_id": result.execution_id, "cost": result.cost});
                    out.push(u.to_string());
                }
                out.push("[DONE]".into());
            }
            ExecEvent::Failed { error, .. } => {
                let api = ApiError::from(error);
                out.push(json!({"error": {"message": api.message, "type": api.kind, "code": api.kind}}).to_string());
                out.push("[DONE]".into());
            }
        }
        futures::stream::iter(out.into_iter().map(|d| Ok::<_, Infallible>(Event::default().data(d))))
    });
    Ok(Sse::new(events).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))).into_response())
}
