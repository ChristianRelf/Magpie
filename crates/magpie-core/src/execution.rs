use serde::{Deserialize, Serialize};

use crate::{Cost, HarnessError, LimitWindow, ProviderKind, RoutingPreset, TaskClass, Timestamp, TokenUsage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    /// Image as base64 data or a URL. Only routed to vision-capable models.
    Image {
        #[serde(skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON-encoded arguments, exactly as produced by the model.
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    #[serde(default)]
    pub content: Vec<ContentPart>,
    /// For `Role::Tool` messages: the call being answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// For `Role::Assistant` messages: calls the model made.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

impl Message {
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self { role, content: vec![ContentPart::Text { text: text.into() }], tool_call_id: None, tool_calls: vec![] }
    }

    /// Concatenated text content.
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        for part in &self.content {
            if let ContentPart::Text { text } = part {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
        }
        out
    }

    pub fn has_images(&self) -> bool {
        self.content.iter().any(|p| matches!(p, ContentPart::Image { .. }))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the tool arguments.
    #[serde(default = "empty_object_schema")]
    pub parameters: serde_json::Value,
}

fn empty_object_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {}})
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Tool { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    Text,
    JsonObject,
    JsonSchema {
        name: String,
        schema: serde_json::Value,
        #[serde(default)]
        strict: bool,
    },
}

/// Options for agentic models (provider CLIs) that run their own tool loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentOptions {
    /// Working directory the agent operates in.
    pub working_dir: String,
    /// Allow the agent to modify files. Read-only by default.
    #[serde(default)]
    pub allow_writes: bool,
}

/// Per-request routing preferences. Unset fields inherit the user's
/// configured defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RequestPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "priority")]
    pub preset: Option<RoutingPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallback: Option<bool>,
    /// Permit routing to metered/billable accounts for this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_billable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cost_usd: Option<f64>,
    /// Restrict routing to these providers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<ProviderKind>,
    /// Never route to these providers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_providers: Vec<ProviderKind>,
}

/// The unified execution request accepted by the harness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecRequest {
    /// `auto`, a model key (`<account>/<model>`), a public id
    /// (`<provider>/<model>`) or a bare model id.
    #[serde(default = "auto_model")]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<TaskClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Provider-neutral reasoning effort: low, medium, high (passed through
    /// where supported).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub preferences: RequestPreferences,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentOptions>,
    /// Client-supplied metadata stored with the execution record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

fn auto_model() -> String {
    "auto".into()
}

impl ExecRequest {
    pub fn simple(prompt: impl Into<String>) -> Self {
        Self {
            model: auto_model(),
            task_type: None,
            system: None,
            messages: vec![Message::text(Role::User, prompt)],
            tools: vec![],
            tool_choice: None,
            response_format: None,
            max_output_tokens: None,
            temperature: None,
            reasoning_effort: None,
            stream: false,
            preferences: RequestPreferences::default(),
            agent: None,
            metadata: None,
        }
    }

    /// All text in the request (system + messages) for classification and
    /// token estimation.
    pub fn all_text(&self) -> String {
        let mut out = self.system.clone().unwrap_or_default();
        for m in &self.messages {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&m.text_content());
            for tc in &m.tool_calls {
                out.push_str(&tc.arguments);
            }
        }
        out
    }

    pub fn last_user_text(&self) -> String {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.text_content())
            .unwrap_or_default()
    }

    pub fn has_images(&self) -> bool {
        self.messages.iter().any(|m| m.has_images())
    }

    pub fn validate(&self) -> Result<(), HarnessError> {
        if self.messages.is_empty() {
            return Err(HarnessError::invalid("Request must contain at least one message"));
        }
        if !self.messages.iter().any(|m| m.role == Role::User || m.role == Role::Tool) {
            return Err(HarnessError::invalid("Request must contain a user message"));
        }
        if let Some(t) = self.temperature {
            if !(0.0..=2.0).contains(&t) {
                return Err(HarnessError::invalid("temperature must be between 0 and 2"));
            }
        }
        if let Some(c) = self.preferences.max_cost_usd {
            if c < 0.0 {
                return Err(HarnessError::invalid("max_cost_usd must be non-negative"));
            }
        }
        for tool in &self.tools {
            if tool.name.is_empty() || tool.name.len() > 128 {
                return Err(HarnessError::invalid("Tool names must be 1-128 characters"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Refusal,
    Cancelled,
    Error,
}

/// Events emitted by provider adapters while executing.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderEvent {
    TextDelta(String),
    ReasoningDelta(String),
    ToolCall(ToolCall),
    Usage(TokenUsage),
    /// The concrete model that served the request, if the provider reports it.
    ResolvedModel(String),
}

/// Final adapter result for a successful attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderOutcome {
    pub finish_reason: FinishReason,
    pub usage: TokenUsage,
    /// Provider-reported cost, if any.
    pub cost: Option<Cost>,
    /// Limit information observed during the request (headers, events).
    pub limits: Vec<LimitWindow>,
    pub resolved_model: Option<String>,
}

impl Default for ProviderOutcome {
    fn default() -> Self {
        Self {
            finish_reason: FinishReason::Stop,
            usage: TokenUsage::default(),
            cost: None,
            limits: vec![],
            resolved_model: None,
        }
    }
}

/// Identifies the model an execution ran on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectedModel {
    pub key: String,
    pub account_id: String,
    pub provider: ProviderKind,
    pub model_id: String,
    pub display_name: String,
}

/// Events streamed to API clients for one execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecEvent {
    Started {
        execution_id: String,
        model: SelectedModel,
        task: TaskClass,
        reasons: Vec<String>,
    },
    /// The harness moved the request to a different model.
    RoutingChanged {
        from: SelectedModel,
        to: SelectedModel,
        reason: String,
    },
    TextDelta {
        text: String,
    },
    ReasoningDelta {
        text: String,
    },
    ToolCall {
        call: ToolCall,
    },
    Completed {
        result: ExecResult,
    },
    Failed {
        execution_id: String,
        error: HarnessError,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecResult {
    pub execution_id: String,
    pub model: SelectedModel,
    pub output_text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
    pub usage: TokenUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<u64>,
    pub attempts: u32,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl ExecutionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ExecutionStatus::Running => "running",
            ExecutionStatus::Succeeded => "succeeded",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "cancelled" => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

/// One attempt within an execution (initial + fallbacks).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub model: SelectedModel,
    pub started_at: Timestamp,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HarnessError>,
}

/// Persisted execution metadata. Request/response content is only present
/// when the user enabled content retention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub created_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
    pub client: String,
    pub status: ExecutionStatus,
    pub task: TaskClass,
    pub complexity: crate::Complexity,
    pub preset: RoutingPreset,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<SelectedModel>,
    pub usage: TokenUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HarnessError>,
    pub stream: bool,
    pub attempts: Vec<AttemptRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<crate::RoutingDecision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_deserialises_with_defaults() {
        let r: ExecRequest = serde_json::from_value(serde_json::json!({
            "messages": [{"role": "user", "content": [{"type": "text", "text": "hi"}]}],
            "preferences": {"priority": "best_quality", "allow_fallback": true}
        }))
        .unwrap();
        assert_eq!(r.model, "auto");
        assert_eq!(r.preferences.preset, Some(RoutingPreset::BestQuality));
        assert!(r.validate().is_ok());
    }

    #[test]
    fn validation_rejects_empty() {
        let mut r = ExecRequest::simple("x");
        r.messages.clear();
        assert!(r.validate().is_err());
        let mut r = ExecRequest::simple("x");
        r.temperature = Some(5.0);
        assert!(r.validate().is_err());
    }
}
