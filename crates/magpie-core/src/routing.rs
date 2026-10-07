use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Capabilities, ProviderKind, SelectedModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    SimpleQuestion,
    CodeGeneration,
    Debugging,
    RepositoryAnalysis,
    MathReasoning,
    Planning,
    Summarisation,
    ToolExecution,
    LargeContext,
    DataExtraction,
    General,
}

impl TaskClass {
    pub const ALL: [TaskClass; 11] = [
        TaskClass::SimpleQuestion,
        TaskClass::CodeGeneration,
        TaskClass::Debugging,
        TaskClass::RepositoryAnalysis,
        TaskClass::MathReasoning,
        TaskClass::Planning,
        TaskClass::Summarisation,
        TaskClass::ToolExecution,
        TaskClass::LargeContext,
        TaskClass::DataExtraction,
        TaskClass::General,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TaskClass::SimpleQuestion => "Simple question",
            TaskClass::CodeGeneration => "Code generation",
            TaskClass::Debugging => "Debugging",
            TaskClass::RepositoryAnalysis => "Repository analysis",
            TaskClass::MathReasoning => "Mathematical reasoning",
            TaskClass::Planning => "Complex planning",
            TaskClass::Summarisation => "Summarisation",
            TaskClass::ToolExecution => "Tool execution",
            TaskClass::LargeContext => "Large-context processing",
            TaskClass::DataExtraction => "Structured extraction",
            TaskClass::General => "General",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TaskClass::SimpleQuestion => "simple_question",
            TaskClass::CodeGeneration => "code_generation",
            TaskClass::Debugging => "debugging",
            TaskClass::RepositoryAnalysis => "repository_analysis",
            TaskClass::MathReasoning => "math_reasoning",
            TaskClass::Planning => "planning",
            TaskClass::Summarisation => "summarisation",
            TaskClass::ToolExecution => "tool_execution",
            TaskClass::LargeContext => "large_context",
            TaskClass::DataExtraction => "data_extraction",
            TaskClass::General => "general",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|t| t.as_str() == s).or(match s.as_str() {
            "code" | "coding" => Some(TaskClass::CodeGeneration),
            "debug" => Some(TaskClass::Debugging),
            "repo" | "repository" => Some(TaskClass::RepositoryAnalysis),
            "math" | "reasoning" => Some(TaskClass::MathReasoning),
            "plan" => Some(TaskClass::Planning),
            "summary" | "summarize" | "summarization" => Some(TaskClass::Summarisation),
            "tools" | "tool" => Some(TaskClass::ToolExecution),
            "extraction" | "extract" => Some(TaskClass::DataExtraction),
            "simple" | "question" | "qa" => Some(TaskClass::SimpleQuestion),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingPreset {
    #[serde(alias = "auto", alias = "balanced")]
    Automatic,
    #[serde(alias = "quality")]
    BestQuality,
    #[serde(alias = "speed", alias = "fast")]
    Fastest,
    #[serde(alias = "cost", alias = "cheap")]
    Economical,
    #[serde(alias = "preserve")]
    PreserveLimits,
    Manual,
}

impl RoutingPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            RoutingPreset::Automatic => "automatic",
            RoutingPreset::BestQuality => "best_quality",
            RoutingPreset::Fastest => "fastest",
            RoutingPreset::Economical => "economical",
            RoutingPreset::PreserveLimits => "preserve_limits",
            RoutingPreset::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
    }
}

/// A user-defined rule mapping a task class to preferred models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRule {
    pub task: TaskClass,
    /// Model keys in preference order.
    pub models: Vec<String>,
    /// Optional preset override for this task class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<RoutingPreset>,
}

/// User routing configuration, edited in the Routing screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    pub preset: RoutingPreset,
    /// Model key used by the Manual preset.
    pub manual_model: Option<String>,
    /// Providers in preference order (earlier = preferred).
    pub provider_order: Vec<ProviderKind>,
    /// Model keys in fallback order.
    pub fallback_order: Vec<String>,
    pub task_rules: Vec<TaskRule>,
    pub allow_fallback: bool,
    /// Allow routing from subscription-backed execution to metered API
    /// accounts. Off by default: requires explicit opt-in.
    pub allow_subscription_to_api: bool,
    /// Allow metered accounts to be used at all for automatic routing.
    pub allow_metered: bool,
    /// Maximum estimated cost per request in USD.
    pub max_cost_per_request_usd: Option<f64>,
    /// Keep this percentage of known allowances on premium models reserved
    /// for high-complexity tasks.
    pub preserve_premium_percent: Option<f64>,
    /// Quality bias in [-1, 1]: negative favours cheaper, positive favours
    /// stronger models (used by Automatic).
    pub quality_bias: f64,
    /// Use the history of latency/success when ranking.
    pub learn_from_history: bool,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            preset: RoutingPreset::Automatic,
            manual_model: None,
            provider_order: vec![],
            fallback_order: vec![],
            task_rules: vec![],
            allow_fallback: true,
            allow_subscription_to_api: false,
            allow_metered: true,
            max_cost_per_request_usd: None,
            preserve_premium_percent: None,
            quality_bias: 0.0,
            learn_from_history: true,
        }
    }
}

/// Classification output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classification {
    pub task: TaskClass,
    pub complexity: Complexity,
    /// Capabilities the request requires.
    pub required: Capabilities,
    pub estimated_input_tokens: u64,
    pub estimated_output_tokens: u64,
    /// Whether the task type came from the client rather than heuristics.
    pub explicit: bool,
    pub signals: Vec<String>,
}

/// A ranked routing candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub model: SelectedModel,
    pub score: f64,
    /// Score components for transparency.
    pub components: BTreeMap<String, f64>,
    pub reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<f64>,
    pub billable: bool,
}

/// A model that was considered and rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejection {
    pub model_key: String,
    pub display_name: String,
    pub reason: String,
}

/// The full, inspectable outcome of model selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub preset: RoutingPreset,
    pub classification: Classification,
    /// Candidates in ranked order; the first is selected.
    pub candidates: Vec<Candidate>,
    pub rejected: Vec<Rejection>,
    pub allow_fallback: bool,
}

impl RoutingDecision {
    pub fn selected(&self) -> Option<&Candidate> {
        self.candidates.first()
    }
}
