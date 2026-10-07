//! Conversion from wire formats (OpenAI chat messages, Responses-style
//! `input`) to the unified [`ExecRequest`].

use magpie_core::*;
use serde::Deserialize;
use serde_json::Value;

use crate::error::ApiError;

fn parse_role(s: &str) -> Result<Role, ApiError> {
    match s {
        "user" => Ok(Role::User),
        "assistant" | "model" => Ok(Role::Assistant),
        "system" | "developer" => Ok(Role::System),
        "tool" | "function" => Ok(Role::Tool),
        other => Err(ApiError::bad_request(format!("Unsupported message role: {other}"))),
    }
}

fn parse_image_url(url: &str) -> ContentPart {
    if let Some(rest) = url.strip_prefix("data:") {
        if let Some((meta, data)) = rest.split_once(',') {
            let media_type = meta.split(';').next().map(str::to_string);
            return ContentPart::Image { media_type, data: Some(data.to_string()), url: None };
        }
    }
    ContentPart::Image { media_type: None, data: None, url: Some(url.to_string()) }
}

/// Parse message content: a string or an array of parts.
pub fn parse_content(v: &Value) -> Result<Vec<ContentPart>, ApiError> {
    match v {
        Value::Null => Ok(vec![]),
        Value::String(s) => Ok(vec![ContentPart::Text { text: s.clone() }]),
        Value::Array(parts) => {
            let mut out = Vec::new();
            for p in parts {
                if let Some(s) = p.as_str() {
                    out.push(ContentPart::Text { text: s.to_string() });
                    continue;
                }
                let ty = p["type"].as_str().unwrap_or("text");
                match ty {
                    "text" | "input_text" | "output_text" => {
                        out.push(ContentPart::Text { text: p["text"].as_str().unwrap_or_default().to_string() });
                    }
                    "image_url" | "input_image" | "image" => {
                        let url = p["image_url"]["url"].as_str().or_else(|| p["image_url"].as_str()).or_else(|| p["url"].as_str());
                        if let Some(u) = url {
                            out.push(parse_image_url(u));
                        } else if let Some(d) = p["data"].as_str().or_else(|| p["source"]["data"].as_str()) {
                            let mt = p["media_type"].as_str().or_else(|| p["source"]["media_type"].as_str()).map(str::to_string);
                            out.push(ContentPart::Image { media_type: mt, data: Some(d.to_string()), url: None });
                        } else {
                            return Err(ApiError::bad_request("Image part requires a url or data"));
                        }
                    }
                    other => return Err(ApiError::bad_request(format!("Unsupported content part type: {other}"))),
                }
            }
            Ok(out)
        }
        _ => Err(ApiError::bad_request("content must be a string or an array")),
    }
}

/// Parse an OpenAI-style message object.
pub fn parse_message(v: &Value) -> Result<Message, ApiError> {
    let role = parse_role(v["role"].as_str().unwrap_or("user"))?;
    let content = parse_content(&v["content"])?;
    let tool_calls = v["tool_calls"]
        .as_array()
        .map(|calls| {
            calls
                .iter()
                .map(|c| ToolCall {
                    id: c["id"].as_str().unwrap_or_default().to_string(),
                    name: c["function"]["name"].as_str().or_else(|| c["name"].as_str()).unwrap_or_default().to_string(),
                    arguments: match &c["function"]["arguments"] {
                        Value::String(s) => s.clone(),
                        Value::Null => c["arguments"].as_str().map(str::to_string).unwrap_or_else(|| "{}".into()),
                        other => other.to_string(),
                    },
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Message { role, content, tool_call_id: v["tool_call_id"].as_str().map(str::to_string), tool_calls })
}

pub fn parse_tools(v: &Value) -> Result<Vec<ToolDefinition>, ApiError> {
    let Some(arr) = v.as_array() else { return Ok(vec![]) };
    arr.iter()
        .map(|t| {
            let f = if t.get("function").is_some() { &t["function"] } else { t };
            let name = f["name"].as_str().ok_or_else(|| ApiError::bad_request("Each tool needs a name"))?;
            Ok(ToolDefinition {
                name: name.to_string(),
                description: f["description"].as_str().map(str::to_string),
                parameters: f
                    .get("parameters")
                    .or_else(|| f.get("input_schema"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}})),
            })
        })
        .collect()
}

pub fn parse_tool_choice(v: &Value) -> Option<ToolChoice> {
    match v {
        Value::String(s) => match s.as_str() {
            "auto" => Some(ToolChoice::Auto),
            "none" => Some(ToolChoice::None),
            "required" | "any" => Some(ToolChoice::Required),
            _ => None,
        },
        Value::Object(_) => v["function"]["name"].as_str().or_else(|| v["name"].as_str()).map(|n| ToolChoice::Tool { name: n.to_string() }),
        _ => None,
    }
}

pub fn parse_response_format(v: &Value) -> Option<ResponseFormat> {
    match v["type"].as_str()? {
        "json_object" => Some(ResponseFormat::JsonObject),
        "json_schema" => {
            let js = if v.get("json_schema").is_some() { &v["json_schema"] } else { v };
            Some(ResponseFormat::JsonSchema {
                name: js["name"].as_str().unwrap_or("response").to_string(),
                schema: js.get("schema").cloned().unwrap_or_else(|| serde_json::json!({"type": "object"})),
                strict: js["strict"].as_bool().unwrap_or(false),
            })
        }
        _ => Some(ResponseFormat::Text),
    }
}

/// Magpie extension object accepted on both endpoints.
#[derive(Debug, Default, Deserialize)]
pub struct MagpieExt {
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub preferences: Option<RequestPreferences>,
    #[serde(default)]
    pub agent: Option<AgentOptions>,
}

/// Body of `POST /v1/responses`.
#[derive(Debug, Deserialize)]
pub struct ResponsesBody {
    #[serde(default = "auto")]
    pub model: String,
    #[serde(default)]
    pub task_type: Option<String>,
    /// String prompt or a list of messages.
    #[serde(default)]
    pub input: Value,
    #[serde(default, alias = "system")]
    pub instructions: Option<String>,
    #[serde(default)]
    pub tools: Value,
    #[serde(default)]
    pub tool_choice: Value,
    #[serde(default, alias = "response_format")]
    pub text_format: Value,
    #[serde(default, alias = "max_tokens")]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub preferences: Option<RequestPreferences>,
    #[serde(default)]
    pub agent: Option<AgentOptions>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

fn auto() -> String {
    "auto".into()
}

pub fn task_from(s: Option<&str>) -> Result<Option<TaskClass>, ApiError> {
    match s {
        None => Ok(None),
        Some("auto") | Some("") => Ok(None),
        Some(t) => TaskClass::parse(t).map(Some).ok_or_else(|| ApiError::bad_request(format!("Unknown task_type: {t}"))),
    }
}

impl ResponsesBody {
    pub fn into_request(self) -> Result<ExecRequest, ApiError> {
        let messages = match &self.input {
            Value::String(s) => vec![Message::text(Role::User, s.clone())],
            Value::Array(items) => items.iter().map(parse_message).collect::<Result<Vec<_>, _>>()?,
            Value::Null => return Err(ApiError::bad_request("`input` is required")),
            _ => return Err(ApiError::bad_request("`input` must be a string or an array of messages")),
        };
        Ok(ExecRequest {
            model: self.model,
            task_type: task_from(self.task_type.as_deref())?,
            system: self.instructions,
            messages,
            tools: parse_tools(&self.tools)?,
            tool_choice: parse_tool_choice(&self.tool_choice),
            response_format: parse_response_format(&self.text_format),
            max_output_tokens: self.max_output_tokens,
            temperature: self.temperature,
            reasoning_effort: self.reasoning_effort,
            stream: self.stream,
            preferences: self.preferences.unwrap_or_default(),
            agent: self.agent,
            metadata: self.metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn responses_body_with_string_input() {
        let b: ResponsesBody = serde_json::from_value(json!({
            "model": "auto", "task_type": "code", "input": "Inspect this function for bugs",
            "preferences": {"priority": "quality", "allow_fallback": true}
        }))
        .unwrap();
        let r = b.into_request().unwrap();
        assert_eq!(r.task_type, Some(TaskClass::CodeGeneration));
        assert_eq!(r.preferences.preset, Some(RoutingPreset::BestQuality));
        assert_eq!(r.messages[0].text_content(), "Inspect this function for bugs");
    }

    #[test]
    fn parses_openai_messages_with_images_and_tools() {
        let m = parse_message(&json!({"role": "user", "content": [
            {"type": "text", "text": "what is this"},
            {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,QUJD"}}
        ]}))
        .unwrap();
        assert!(matches!(&m.content[1], ContentPart::Image { media_type: Some(t), data: Some(d), .. } if t == "image/jpeg" && d == "QUJD"));
        let a = parse_message(&json!({"role": "assistant", "content": null, "tool_calls": [
            {"id": "c1", "type": "function", "function": {"name": "f", "arguments": "{\"x\":1}"}}
        ]}))
        .unwrap();
        assert_eq!(a.tool_calls[0].arguments, "{\"x\":1}");
        assert!(parse_message(&json!({"role": "wizard", "content": "x"})).is_err());
    }

    #[test]
    fn response_formats() {
        assert_eq!(parse_response_format(&json!({"type": "json_object"})), Some(ResponseFormat::JsonObject));
        let f = parse_response_format(
            &json!({"type": "json_schema", "json_schema": {"name": "x", "schema": {"type": "object"}, "strict": true}}),
        );
        assert!(matches!(f, Some(ResponseFormat::JsonSchema { strict: true, .. })));
        assert_eq!(parse_tool_choice(&json!({"type": "function", "function": {"name": "f"}})), Some(ToolChoice::Tool { name: "f".into() }));
    }
}
