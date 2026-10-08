import type {
  AccountLimits,
  AccountPatch,
  ApiKey,
  BreakdownRow,
  CliStatus,
  ConnectRequest,
  ExecEvent,
  ExecutionRecord,
  ExecutionSummary,
  GroupBy,
  HarnessEvent,
  HarnessNotification,
  HarnessStatus,
  ModelInfo,
  ModelPreference,
  ProviderAccount,
  ProviderDescriptor,
  ProviderUsageReport,
  ResponsesRequest,
  ResponsesResult,
  RoutingConfig,
  RoutingDecision,
  Settings,
  TimeseriesPoint,
  UsageSummary,
  Account,
} from "./types.js";

export class MagpieError extends Error {
  readonly status: number;
  readonly type: string;
  readonly retryAfter?: number;
  constructor(
    message: string,
    status: number,
    type: string,
    retryAfter?: number,
  ) {
    super(message);
    this.name = "MagpieError";
    this.status = status;
    this.type = type;
    this.retryAfter = retryAfter;
  }
}

export interface MagpieClientOptions {
  /** Harness URL, e.g. `http://127.0.0.1:7878`. */
  baseUrl?: string;
  /** A Magpie API key (`mgp_...`) or the admin token. */
  apiKey: string;
  fetch?: typeof fetch;
}

export type Range = "1h" | "24h" | "7d" | "30d" | "90d";

export interface RangeQuery {
  range?: Range | "custom";
  from?: number;
  to?: number;
  provider?: string;
  account_id?: string;
  model_key?: string;
}

export interface ExecutionQuery {
  limit?: number;
  offset?: number;
  status?: string;
  provider?: string;
  account_id?: string;
  model_key?: string;
  task?: string;
  from?: number;
  to?: number;
}

/** Parse a `text/event-stream` body into `{event, data}` records. */
export async function* parseSSE(
  body: ReadableStream<Uint8Array>,
  signal?: AbortSignal,
): AsyncGenerator<{ event?: string; data: string }> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let pendingCR = false;
  const cancel = () => {
    void reader.cancel().catch(() => {});
  };
  signal?.addEventListener("abort", cancel, { once: true });
  try {
    while (!signal?.aborted) {
      const { value, done } = await reader.read();
      let chunk: string =
        (pendingCR ? "\r" : "") +
        (done ? decoder.decode() : decoder.decode(value, { stream: true }));
      pendingCR = !done && chunk.endsWith("\r");
      if (pendingCR) chunk = chunk.slice(0, -1);
      buffer += chunk.replace(/\r\n?/g, "\n");
      if (buffer.length > 8 * 1024 * 1024)
        throw new Error("SSE event exceeds 8 MiB");
      let idx: number;
      while ((idx = buffer.indexOf("\n\n")) >= 0) {
        const block = buffer.slice(0, idx);
        buffer = buffer.slice(idx + 2);
        let event: string | undefined;
        const data: string[] = [];
        for (const line of block.split("\n")) {
          if (!line || line.startsWith(":")) continue;
          const colon = line.indexOf(":");
          const field = colon < 0 ? line : line.slice(0, colon);
          const value =
            colon < 0 ? "" : line.slice(colon + 1).replace(/^ /, "");
          if (field === "event") event = value;
          else if (field === "data") data.push(value);
        }
        if (data.length) yield { event, data: data.join("\n") };
      }
      if (done) break;
    }
  } finally {
    signal?.removeEventListener("abort", cancel);
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}

function qs(params: object): string {
  const entries = Object.entries(params).filter(
    ([, v]) => v !== undefined && v !== null && v !== "",
  );
  if (!entries.length) return "";
  return (
    "?" +
    entries
      .map(
        ([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`,
      )
      .join("&")
  );
}

/** Client for the Magpie local harness API. */
export class MagpieClient {
  readonly baseUrl: string;
  private readonly apiKey: string;
  private readonly fetchImpl: typeof fetch;

  constructor(opts: MagpieClientOptions) {
    this.baseUrl = (opts.baseUrl ?? "http://127.0.0.1:7878").replace(/\/$/, "");
    this.apiKey = opts.apiKey;
    this.fetchImpl = opts.fetch ?? globalThis.fetch.bind(globalThis);
  }

  private async raw(
    method: string,
    path: string,
    body?: unknown,
    signal?: AbortSignal,
  ): Promise<Response> {
    const res = await this.fetchImpl(this.baseUrl + path, {
      method,
      headers: {
        Authorization: `Bearer ${this.apiKey}`,
        ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
      },
      body: body !== undefined ? JSON.stringify(body) : undefined,
      signal,
    });
    if (!res.ok) {
      let message = `HTTP ${res.status}`;
      let type = "http_error";
      try {
        const j = await res.json();
        message = j?.error?.message ?? message;
        type = j?.error?.type ?? type;
      } catch {
        /* non-JSON error body */
      }
      const ra = res.headers.get("retry-after");
      throw new MagpieError(
        message,
        res.status,
        type,
        ra ? Number(ra) : undefined,
      );
    }
    return res;
  }

  private async json<T>(
    method: string,
    path: string,
    body?: unknown,
    signal?: AbortSignal,
  ): Promise<T> {
    const res = await this.raw(method, path, body, signal);
    if (res.status === 204) return undefined as T;
    const text = await res.text();
    return (text ? JSON.parse(text) : undefined) as T;
  }

  // Execution ---------------------------------------------------------------

  /** Execute and wait for the complete result. */
  respond(
    req: ResponsesRequest,
    signal?: AbortSignal,
  ): Promise<ResponsesResult> {
    return this.json(
      "POST",
      "/v1/responses",
      { ...req, stream: false },
      signal,
    );
  }

  /** Execute with streaming events. */
  async *stream(
    req: ResponsesRequest,
    signal?: AbortSignal,
  ): AsyncGenerator<ExecEvent> {
    const res = await this.raw(
      "POST",
      "/v1/responses",
      { ...req, stream: true },
      signal,
    );
    if (!res.body) return;
    for await (const ev of parseSSE(res.body, signal)) {
      yield JSON.parse(ev.data) as ExecEvent;
    }
  }

  /** Explain the routing decision without executing. */
  route(req: ResponsesRequest): Promise<RoutingDecision> {
    return this.json("POST", "/v1/route", req);
  }

  cancel(executionId: string): Promise<{ cancelled: boolean }> {
    return this.json(
      "POST",
      `/v1/executions/${encodeURIComponent(executionId)}/cancel`,
    );
  }

  // Inventory -----------------------------------------------------------------

  status(): Promise<HarnessStatus> {
    return this.json("GET", "/v1/status");
  }

  async health(): Promise<{
    status: string;
    version: string;
    api_version: number;
  }> {
    const res = await this.fetchImpl(this.baseUrl + "/health");
    if (!res.ok)
      throw new MagpieError(
        "Harness health check failed",
        res.status,
        "health_check",
      );
    const health = await res.json();
    if (
      health.status !== "ok" ||
      health.product !== "Magpie" ||
      health.api_version !== 1
    )
      throw new Error("Incompatible harness health response");
    return health;
  }

  async models(): Promise<ModelInfo[]> {
    const r = await this.json<{ data: { id: string; magpie: ModelInfo }[] }>(
      "GET",
      "/v1/models",
    );
    return r.data.filter((m) => m.id !== "auto").map((m) => m.magpie);
  }

  setModelPreference(
    key: string,
    preference: ModelPreference,
  ): Promise<ModelPreference> {
    return this.json("POST", "/v1/model-preferences", { key, preference });
  }

  providers(): Promise<{
    accounts: ProviderAccount[];
    kinds: ProviderDescriptor[];
  }> {
    return this.json("GET", "/v1/providers");
  }

  async connectProvider(req: ConnectRequest): Promise<Account> {
    // Older v1 services ignore unknown JSON fields. Do not send a saved
    // token to one and accidentally connect its shared CLI login instead.
    if (req.auth_mode && req.auth_mode !== "existing") {
      const status = await this.status();
      if (!status.features?.includes("auth_profiles")) {
        throw new MagpieError("Restart the harness in Settings to use saved authentication profiles.", 409, "outdated_harness");
      }
    }
    return this.json("POST", "/v1/providers", req);
  }

  updateProvider(id: string, patch: AccountPatch): Promise<Account> {
    return this.json("PATCH", `/v1/providers/${encodeURIComponent(id)}`, patch);
  }

  deleteProvider(id: string): Promise<void> {
    return this.json("DELETE", `/v1/providers/${encodeURIComponent(id)}`);
  }

  verifyProvider(id: string): Promise<Account> {
    return this.json("POST", `/v1/providers/${encodeURIComponent(id)}/verify`);
  }

  /** Open auth_url in the browser; the harness completes sign-in in the background. */
  loginProvider(id: string): Promise<{ auth_url: string }> {
    return this.json("POST", `/v1/providers/${encodeURIComponent(id)}/login`);
  }

  refreshProvider(
    id: string,
  ): Promise<{ account: Account; models: number | null }> {
    return this.json("POST", `/v1/providers/${encodeURIComponent(id)}/refresh`);
  }

  detectClis(): Promise<{ clis: CliStatus[] }> {
    return this.json("GET", "/v1/providers/cli");
  }

  limits(): Promise<{ accounts: AccountLimits[] }> {
    return this.json("GET", "/v1/limits");
  }

  // Telemetry ---------------------------------------------------------------

  usageSummary(
    q: RangeQuery = {},
  ): Promise<{
    current: UsageSummary;
    previous: UsageSummary;
    active: number;
  }> {
    return this.json("GET", "/v1/usage/summary" + qs(q));
  }

  usageTimeseries(
    q: RangeQuery & { bucket_ms?: number; group_by?: GroupBy } = {},
  ): Promise<{
    from: number;
    to: number;
    bucket_ms: number;
    group_by: GroupBy;
    points: TimeseriesPoint[];
  }> {
    return this.json("GET", "/v1/usage/timeseries" + qs(q));
  }

  usageBreakdown(
    q: RangeQuery & { group_by?: GroupBy } = {},
  ): Promise<{ rows: BreakdownRow[]; group_by: GroupBy }> {
    return this.json("GET", "/v1/usage/breakdown" + qs(q));
  }

  providerReports(): Promise<{ reports: ProviderUsageReport[] }> {
    return this.json("GET", "/v1/usage/provider-reports");
  }

  async exportUsage(
    q: RangeQuery & { format?: "csv" | "json" } = {},
  ): Promise<string> {
    const res = await this.raw("GET", "/v1/usage/export" + qs(q));
    return res.text();
  }

  clearHistory(): Promise<{ deleted: number }> {
    return this.json("DELETE", "/v1/history");
  }

  executions(
    q: ExecutionQuery = {},
  ): Promise<{ data: ExecutionRecord[]; total: number }> {
    return this.json("GET", "/v1/executions" + qs(q));
  }

  activeExecutions(): Promise<{ data: ExecutionSummary[] }> {
    return this.json("GET", "/v1/executions/active");
  }

  execution(id: string): Promise<ExecutionRecord> {
    return this.json("GET", `/v1/executions/${encodeURIComponent(id)}`);
  }

  /** Subscribe to harness events. Resolves when the stream ends. */
  async *events(signal?: AbortSignal): AsyncGenerator<HarnessEvent> {
    const res = await this.raw("GET", "/v1/events", undefined, signal);
    if (!res.body) return;
    for await (const ev of parseSSE(res.body, signal)) {
      try {
        yield JSON.parse(ev.data) as HarnessEvent;
      } catch {
        /* ignore malformed events */
      }
    }
  }

  // Configuration -----------------------------------------------------------

  settings(): Promise<{ settings: Settings; autostart_enabled: boolean }> {
    return this.json("GET", "/v1/settings");
  }

  updateSettings(
    s: Settings,
  ): Promise<{ settings: Settings; autostart_enabled: boolean }> {
    return this.json("PUT", "/v1/settings", s);
  }

  routing(): Promise<RoutingConfig> {
    return this.json("GET", "/v1/routing");
  }

  updateRouting(cfg: RoutingConfig): Promise<RoutingConfig> {
    return this.json("PUT", "/v1/routing", cfg);
  }

  keys(): Promise<{ keys: ApiKey[] }> {
    return this.json("GET", "/v1/keys");
  }

  createKey(
    name: string,
    scopes: string[],
  ): Promise<{ client: ApiKey; token: string }> {
    return this.json("POST", "/v1/keys", { name, scopes });
  }

  revokeKey(id: string): Promise<void> {
    return this.json("DELETE", `/v1/keys/${encodeURIComponent(id)}`);
  }

  notifications(): Promise<{ notifications: HarnessNotification[] }> {
    return this.json("GET", "/v1/notifications");
  }

  markNotificationsRead(): Promise<void> {
    return this.json("POST", "/v1/notifications/read");
  }

  shutdown(): Promise<{ stopping: boolean }> {
    return this.json("POST", "/v1/admin/shutdown");
  }
}
