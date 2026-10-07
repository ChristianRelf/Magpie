// Types mirroring the harness API (crates/magpie-core). Keep in sync with
// the Rust definitions; field names match the JSON wire format.

export type ProviderKind =
  | "openai"
  | "anthropic"
  | "gemini"
  | "open_router"
  | "groq"
  | "mistral"
  | "deep_seek"
  | "ollama"
  | "lm_studio"
  | "open_ai_compatible"
  | "claude_code"
  | "codex_cli"
  | "gemini_cli";

export type AuthMethod = "api_key" | "cli_delegated" | "none";
export type BillingMode = "subscription" | "metered" | "credits" | "local" | "unknown";
export type ConnectionStatus = "connected" | "needs_auth" | "unavailable" | "error" | "disabled" | "pending";
export type Provenance = "reported" | "calculated" | "estimated" | "unavailable";
export type QualityTier = "light" | "standard" | "high" | "frontier";
export type SpeedClass = "slow" | "medium" | "fast";
export type LimitState = "unknown" | "available" | "approaching" | "limited" | "exhausted" | "reset_pending";
export type LimitMetric = "requests" | "tokens" | "input_tokens" | "output_tokens" | "usage_percent" | "credits" | "spend";
export type RoutingPreset = "automatic" | "best_quality" | "fastest" | "economical" | "preserve_limits" | "manual";
export type TaskClass =
  | "simple_question"
  | "code_generation"
  | "debugging"
  | "repository_analysis"
  | "math_reasoning"
  | "planning"
  | "summarisation"
  | "tool_execution"
  | "large_context"
  | "data_extraction"
  | "general";
export type Complexity = "low" | "medium" | "high";
export type ExecutionStatus = "running" | "succeeded" | "failed" | "cancelled";
export type FinishReason = "stop" | "length" | "tool_calls" | "refusal" | "cancelled" | "error";
export type ErrorKind =
  | "authentication"
  | "permission_denied"
  | "rate_limited"
  | "quota_exhausted"
  | "invalid_request"
  | "context_length"
  | "unsupported"
  | "model_not_found"
  | "provider_unavailable"
  | "network"
  | "timeout"
  | "cancelled"
  | "local_dependency"
  | "refused"
  | "no_eligible_model"
  | "internal";

export interface ProviderDescriptor {
  kind: ProviderKind;
  name: string;
  vendor: string;
  auth_method: AuthMethod;
  default_billing: BillingMode;
  default_base_url: string | null;
  cli_binary: string | null;
  cli_install: string | null;
  cli_login: string | null;
  key_url: string | null;
  docs_url: string;
  is_local: boolean;
  allow_multiple: boolean;
  summary: string;
}

export interface Account {
  id: string;
  kind: ProviderKind;
  label: string;
  auth_method: AuthMethod;
  billing_mode: BillingMode;
  billing_reported: boolean;
  base_url?: string;
  identity?: string;
  plan?: string;
  status: ConnectionStatus;
  status_message?: string;
  enabled: boolean;
  last_verified_at?: string;
  created_at: string;
  has_secret: boolean;
  secret_store?: string;
  options: Record<string, unknown>;
}

export interface Capabilities {
  streaming: boolean;
  tools: boolean;
  structured_output: boolean;
  vision: boolean;
  reasoning: boolean;
  agentic: boolean;
  system_prompt: boolean;
}

export interface Pricing {
  input_per_mtok: number;
  output_per_mtok: number;
  cached_input_per_mtok?: number;
  provenance: Provenance;
}

export interface ModelPreference {
  favourite: boolean;
  disabled: boolean;
  priority: number;
  price_override?: Pricing;
  reserve_percent?: number;
}

export interface ModelInfo {
  key: string;
  account_id: string;
  account_label: string;
  provider: ProviderKind;
  billing_mode: BillingMode;
  model_id: string;
  display_name: string;
  description?: string;
  context_window?: number;
  max_output_tokens?: number;
  capabilities: Capabilities;
  tier: QualityTier;
  speed: SpeedClass;
  pricing?: Pricing;
  metadata_provenance: Provenance;
  is_default: boolean;
  reasoning_efforts?: string[];
  preference: ModelPreference;
  available: boolean;
  unavailable_reason?: string;
  discovered_at: string;
}

export interface LimitWindow {
  account_id: string;
  key: string;
  label: string;
  metric: LimitMetric;
  model_scope?: string;
  limit?: number;
  remaining?: number;
  used?: number;
  used_percent?: number;
  window_secs?: number;
  resets_at?: string;
  provenance: Provenance;
  exhausted: boolean;
  observed_at: string;
}

export interface AccountLimits {
  account_id: string;
  state: LimitState;
  windows: LimitWindow[];
  next_reset?: string;
}

export interface ProviderAccount extends Account {
  model_count: number;
  available_model_count: number;
  capabilities: Pick<Capabilities, "tools" | "vision" | "reasoning" | "structured_output" | "agentic">;
  limits: AccountLimits | null;
  descriptor: ProviderDescriptor;
}

export interface TokenUsage {
  input_tokens?: number;
  output_tokens?: number;
  cached_input_tokens?: number;
  cache_write_tokens?: number;
  reasoning_tokens?: number;
  provenance: Provenance;
}

export interface Cost {
  usd: number;
  provenance: Provenance;
  api_equivalent: boolean;
}

export interface HarnessError {
  kind: ErrorKind;
  message: string;
  status?: number;
  retry_after_secs?: number;
  resets_at?: string;
}

export interface SelectedModel {
  key: string;
  account_id: string;
  provider: ProviderKind;
  model_id: string;
  display_name: string;
}

export interface Classification {
  task: TaskClass;
  complexity: Complexity;
  required: Capabilities;
  estimated_input_tokens: number;
  estimated_output_tokens: number;
  explicit: boolean;
  signals: string[];
}

export interface Candidate {
  model: SelectedModel;
  score: number;
  components: Record<string, number>;
  reasons: string[];
  estimated_cost_usd?: number;
  billable: boolean;
}

export interface Rejection {
  model_key: string;
  display_name: string;
  reason: string;
}

export interface RoutingDecision {
  preset: RoutingPreset;
  classification: Classification;
  candidates: Candidate[];
  rejected: Rejection[];
  allow_fallback: boolean;
}

export interface AttemptRecord {
  model: SelectedModel;
  started_at: string;
  duration_ms: number;
  error?: HarnessError;
}

export interface ExecutionRecord {
  id: string;
  created_at: string;
  completed_at?: string;
  client: string;
  status: ExecutionStatus;
  task: TaskClass;
  complexity: Complexity;
  preset: RoutingPreset;
  model?: SelectedModel;
  usage: TokenUsage;
  cost?: Cost;
  duration_ms?: number;
  time_to_first_token_ms?: number;
  finish_reason?: FinishReason;
  error?: HarnessError;
  stream: boolean;
  attempts: AttemptRecord[];
  routing?: RoutingDecision;
  request_content?: unknown;
  response_content?: string;
}

export interface ExecutionSummary {
  id: string;
  created_at: string;
  client: string;
  status: ExecutionStatus;
  task: TaskClass;
  model?: SelectedModel;
  usage: TokenUsage;
  cost?: Cost;
  duration_ms?: number;
  error?: HarnessError;
  attempts: number;
}

export interface UsageSummary {
  from: number;
  to: number;
  requests: number;
  succeeded: number;
  failed: number;
  cancelled: number;
  running: number;
  input_tokens: number;
  output_tokens: number;
  cached_tokens: number;
  reasoning_tokens: number;
  tokens_reported_requests: number;
  tokens_estimated_requests: number;
  cost_reported_usd: number;
  cost_calculated_usd: number;
  cost_estimated_usd: number;
  api_equivalent_usd: number;
  avg_duration_ms: number | null;
  p95_duration_ms: number | null;
  avg_ttft_ms: number | null;
  requests_per_minute: number;
  fallbacks: number;
}

export interface TimeseriesPoint {
  t: number;
  group?: string;
  requests: number;
  failed: number;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  avg_duration_ms: number | null;
}

export type GroupBy = "none" | "provider" | "model" | "account" | "task" | "status";

export interface BreakdownRow {
  key: string;
  label: string;
  provider?: string;
  requests: number;
  failed: number;
  input_tokens: number;
  output_tokens: number;
  cost_usd: number;
  avg_duration_ms: number | null;
}

export interface ProviderUsageReport {
  account_id: string;
  lifetime_tokens?: number;
  peak_daily_tokens?: number;
  daily: { date: string; tokens: number }[];
  provenance: Provenance;
  observed_at: string;
}

export interface HarnessStatus {
  product: string;
  version: string;
  api_version: number;
  started_at: string;
  uptime_secs: number;
  accounts: number;
  connected_accounts: number;
  models: number;
  available_models: number;
  active_executions: number;
  data_dir: string;
  secret_store: string;
  pid: number;
  principal: string;
  is_admin: boolean;
}

export interface TaskRule {
  task: TaskClass;
  models: string[];
  preset?: RoutingPreset;
}

export interface RoutingConfig {
  preset: RoutingPreset;
  manual_model: string | null;
  provider_order: ProviderKind[];
  fallback_order: string[];
  task_rules: TaskRule[];
  allow_fallback: boolean;
  allow_subscription_to_api: boolean;
  allow_metered: boolean;
  max_cost_per_request_usd: number | null;
  preserve_premium_percent: number | null;
  quality_bias: number;
  learn_from_history: boolean;
}

export type ThemePreference = "system" | "dark" | "light";

export interface Settings {
  general: {
    launch_on_startup: boolean;
    minimise_to_tray: boolean;
    keep_harness_running: boolean;
    automatic_updates: boolean;
    theme: ThemePreference;
    reduced_motion: boolean;
    sidebar_collapsed: boolean;
  };
  analytics: {
    retention_days: number;
    refresh_interval_secs: number;
    local_history: boolean;
    poll_provider_limits: boolean;
    spend_alert_usd: number | null;
  };
  security: {
    retain_request_content: boolean;
    diagnostic_logging: boolean;
    require_scopes: boolean;
  };
  notifications: {
    enabled: boolean;
    provider_disconnected: boolean;
    auth_expired: boolean;
    allowance_exhausted: boolean;
    limit_approaching: boolean;
    allowance_reset: boolean;
    fallback_activated: boolean;
    harness_stopped: boolean;
    spend_threshold: boolean;
  };
  server: {
    port: number;
    bind: string;
    allow_network: boolean;
    request_timeout_secs: number;
    max_concurrency: number;
  };
  onboarding_complete: boolean;
}

export type NotificationKind =
  | "provider_disconnected"
  | "auth_expired"
  | "allowance_exhausted"
  | "limit_approaching"
  | "allowance_reset"
  | "fallback_activated"
  | "harness_stopped"
  | "spend_threshold";

export interface HarnessNotification {
  id: string;
  kind: NotificationKind;
  title: string;
  body: string;
  account_id?: string;
  created_at: string;
  read: boolean;
}

export interface ApiKey {
  id: string;
  name: string;
  prefix: string;
  scopes: string[];
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

export interface CliStatus {
  kind: ProviderKind;
  binary: string;
  installed: boolean;
  path: string | null;
  version: string | null;
  install_command: string | null;
  login_command: string | null;
}

export interface ConnectRequest {
  kind: ProviderKind;
  label?: string;
  api_key?: string;
  base_url?: string;
  options?: Record<string, unknown>;
  billing_mode?: BillingMode;
}

export interface AccountPatch {
  label?: string;
  enabled?: boolean;
  base_url?: string;
  api_key?: string;
  options?: Record<string, unknown>;
  billing_mode?: BillingMode;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface ExecResult {
  execution_id: string;
  model: SelectedModel;
  output_text: string;
  tool_calls?: ToolCall[];
  finish_reason: FinishReason;
  usage: TokenUsage;
  cost?: Cost;
  duration_ms: number;
  time_to_first_token_ms?: number;
  attempts: number;
  created_at: string;
}

export type ExecEvent =
  | { type: "started"; execution_id: string; model: SelectedModel; task: TaskClass; reasons: string[] }
  | { type: "routing_changed"; from: SelectedModel; to: SelectedModel; reason: string }
  | { type: "text_delta"; text: string }
  | { type: "reasoning_delta"; text: string }
  | { type: "tool_call"; call: ToolCall }
  | { type: "completed"; result: ExecResult }
  | { type: "failed"; execution_id: string; error: HarnessError };

export type HarnessEvent =
  | { type: "execution_started"; execution: ExecutionSummary }
  | { type: "execution_finished"; execution: ExecutionSummary }
  | { type: "account_updated"; account: Account }
  | { type: "account_removed"; account_id: string }
  | { type: "models_updated"; account_id: string | null }
  | { type: "limits_updated"; account_id: string }
  | { type: "notification"; notification: HarnessNotification }
  | { type: "settings_updated" }
  | { type: "routing_updated" };

/** Input accepted by `POST /v1/responses`. */
export interface ResponsesRequest {
  model?: string;
  task_type?: TaskClass | string;
  input: string | { role: string; content: unknown }[];
  instructions?: string;
  tools?: unknown[];
  tool_choice?: unknown;
  response_format?: unknown;
  max_output_tokens?: number;
  temperature?: number;
  reasoning_effort?: string;
  stream?: boolean;
  preferences?: {
    preset?: RoutingPreset;
    allow_fallback?: boolean;
    allow_billable?: boolean;
    max_cost_usd?: number;
    providers?: ProviderKind[];
    exclude_providers?: ProviderKind[];
  };
  agent?: { working_dir: string; allow_writes?: boolean };
  metadata?: unknown;
}

export interface ResponsesResult {
  id: string;
  object: "response";
  status: "completed";
  model: SelectedModel;
  output_text: string;
  tool_calls: ToolCall[];
  finish_reason: FinishReason;
  usage: TokenUsage;
  cost?: Cost;
  duration_ms: number;
  time_to_first_token_ms?: number;
  attempts: number;
  routing?: { task: TaskClass; reasons: string[] };
  created_at: string;
}
