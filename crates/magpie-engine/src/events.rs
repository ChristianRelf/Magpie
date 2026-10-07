use magpie_core::*;
use serde::Serialize;

/// Harness-wide events broadcast to the desktop app and `/v1/events`
/// subscribers. These never contain request or response content.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HarnessEvent {
    ExecutionStarted { execution: ExecutionSummary },
    ExecutionFinished { execution: ExecutionSummary },
    AccountUpdated { account: Account },
    AccountRemoved { account_id: String },
    ModelsUpdated { account_id: Option<String> },
    LimitsUpdated { account_id: String },
    Notification { notification: Notification },
    SettingsUpdated,
    RoutingUpdated,
}

/// Content-free summary of an execution for live views.
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionSummary {
    pub id: String,
    pub created_at: Timestamp,
    pub client: String,
    pub status: ExecutionStatus,
    pub task: TaskClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<SelectedModel>,
    pub usage: TokenUsage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<HarnessError>,
    pub attempts: usize,
}

impl From<&ExecutionRecord> for ExecutionSummary {
    fn from(r: &ExecutionRecord) -> Self {
        Self {
            id: r.id.clone(),
            created_at: r.created_at,
            client: r.client.clone(),
            status: r.status,
            task: r.task,
            model: r.model.clone(),
            usage: r.usage.clone(),
            cost: r.cost,
            duration_ms: r.duration_ms,
            error: r.error.clone(),
            attempts: r.attempts.len(),
        }
    }
}
