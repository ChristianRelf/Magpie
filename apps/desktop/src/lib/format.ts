import type {
  BillingMode,
  Provenance,
  ProviderKind,
  QualityTier,
  TaskClass,
  ErrorKind,
  LimitState,
  ConnectionStatus,
} from "@magpie/sdk";

export function compact(n: number | null | undefined): string {
  if (n === null || n === undefined || Number.isNaN(n)) return "—";
  const abs = Math.abs(n);
  if (abs >= 1e9) return `${trim(n / 1e9)}B`;
  if (abs >= 1e6) return `${trim(n / 1e6)}M`;
  if (abs >= 1e4) return `${Math.round(n / 1e3)}K`;
  if (abs >= 1e3) return `${trim(n / 1e3)}K`;
  return `${Math.round(n)}`;
}

function trim(v: number): string {
  return v.toFixed(v >= 100 ? 0 : 1).replace(/\.0$/, "");
}

export function integer(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  return Math.round(n).toLocaleString("en-US");
}

export function usd(n: number | null | undefined, opts: { precise?: boolean } = {}): string {
  if (n === null || n === undefined) return "—";
  if (n === 0) return "$0";
  if (opts.precise || Math.abs(n) < 0.01) return `$${n.toFixed(n < 0.0001 ? 6 : 4)}`;
  if (Math.abs(n) < 100) return `$${n.toFixed(2)}`;
  return `$${Math.round(n).toLocaleString("en-US")}`;
}

export function percent(fraction: number | null | undefined, digits = 0): string {
  if (fraction === null || fraction === undefined || Number.isNaN(fraction)) return "—";
  return `${(fraction * 100).toFixed(digits)}%`;
}

export function ms(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  if (n < 1000) return `${Math.round(n)} ms`;
  if (n < 60_000) return `${(n / 1000).toFixed(n < 10_000 ? 2 : 1)} s`;
  return `${Math.floor(n / 60_000)}m ${Math.round((n % 60_000) / 1000)}s`;
}

export function duration(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  if (s >= 86_400) return `${Math.floor(s / 86_400)}d ${Math.floor((s % 86_400) / 3600)}h`;
  if (s >= 3600) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  if (s >= 60) return `${Math.floor(s / 60)}m`;
  return `${s}s`;
}

export function relative(iso: string | number | null | undefined, now = Date.now()): string {
  if (iso === null || iso === undefined) return "—";
  const t = typeof iso === "number" ? iso : Date.parse(iso);
  if (Number.isNaN(t)) return "—";
  const diff = (t - now) / 1000;
  if (Math.abs(diff) < 45) return diff >= 0 ? "in a moment" : "just now";
  return diff > 0 ? `in ${duration(diff)}` : `${duration(-diff)} ago`;
}

export function clock(iso: string | number): string {
  const d = new Date(iso);
  return d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function dateTime(iso: string | number): string {
  const d = new Date(iso);
  return d.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function shortDate(t: number, bucketMs: number, utc = false): string {
  const d = new Date(t);
  if (bucketMs >= 86_400_000)
    return d.toLocaleDateString([], { month: "short", day: "numeric", timeZone: utc ? "UTC" : undefined });
  if (bucketMs >= 3_600_000)
    return d.toLocaleString([], {
      month: "short",
      day: "numeric",
      hour: "2-digit",
    });
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export const PROVIDER_NAMES: Record<ProviderKind, string> = {
  openai: "OpenAI API",
  anthropic: "Anthropic API",
  gemini: "Gemini API",
  open_router: "OpenRouter",
  groq: "Groq",
  mistral: "Mistral",
  deep_seek: "DeepSeek",
  ollama: "Ollama",
  lm_studio: "LM Studio",
  open_ai_compatible: "OpenAI-compatible",
  claude_code: "Claude Code",
  codex_cli: "Codex",
  gemini_cli: "Gemini CLI",
};

export function providerName(kind: string | undefined | null): string {
  if (!kind) return "—";
  return PROVIDER_NAMES[kind as ProviderKind] ?? kind;
}

export const TASK_LABELS: Record<TaskClass, string> = {
  simple_question: "Simple question",
  code_generation: "Code generation",
  debugging: "Debugging",
  repository_analysis: "Repository analysis",
  math_reasoning: "Math reasoning",
  planning: "Planning",
  summarisation: "Summarisation",
  tool_execution: "Tool execution",
  large_context: "Large context",
  data_extraction: "Data extraction",
  general: "General",
};

export const TIER_LABELS: Record<QualityTier, string> = {
  light: "Light",
  standard: "Standard",
  high: "High",
  frontier: "Frontier",
};

export const BILLING_LABELS: Record<BillingMode, string> = {
  subscription: "Subscription",
  metered: "Metered API",
  credits: "Prepaid credits",
  local: "Local",
  unknown: "Unknown billing",
};

export const PROVENANCE_LABELS: Record<Provenance, string> = {
  reported: "Reported",
  calculated: "Calculated",
  estimated: "Estimated",
  unavailable: "Unavailable",
};

export const PROVENANCE_HELP: Record<Provenance, string> = {
  reported: "Obtained directly from the provider.",
  calculated: "Computed locally from reported token counts and known prices.",
  estimated: "Inferred from incomplete data, such as text length or catalog prices.",
  unavailable: "The provider does not expose this information.",
};

export const LIMIT_STATE_LABELS: Record<LimitState, string> = {
  unknown: "Unknown",
  available: "Available",
  approaching: "Approaching limit",
  limited: "Limited",
  exhausted: "Exhausted",
  reset_pending: "Reset pending",
};

export const CONNECTION_LABELS: Record<ConnectionStatus, string> = {
  connected: "Connected",
  needs_auth: "Sign-in required",
  unavailable: "Unavailable",
  error: "Error",
  disabled: "Disabled",
  pending: "Checking",
};

export const ERROR_LABELS: Partial<Record<ErrorKind, string>> = {
  authentication: "Authentication failed",
  permission_denied: "Permission denied",
  rate_limited: "Rate limited",
  quota_exhausted: "Allowance exhausted",
  invalid_request: "Invalid request",
  context_length: "Context too long",
  unsupported: "Unsupported",
  model_not_found: "Model not found",
  provider_unavailable: "Provider unavailable",
  network: "Network error",
  timeout: "Timed out",
  cancelled: "Cancelled",
  local_dependency: "Local dependency missing",
  refused: "Refused",
  no_eligible_model: "No eligible model",
  internal: "Internal error",
};

export function titleCase(s: string): string {
  return s.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());
}
