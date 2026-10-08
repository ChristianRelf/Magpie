# Developer API

## Authentication and permissions

Create an access key in Integrations or `magpie keys create my-tool`. Supply `Authorization: Bearer <key>`. Keys are displayed once; only their SHA-256 hashes are retained. Revoke them from Integrations. Never give external tools the desktop's `admin.token`.

| Scope     | Access                                                                                                 |
| --------- | ------------------------------------------------------------------------------------------------------ |
| `read`    | Models, health details, usage, limits, routing previews and execution/event metadata                   |
| `execute` | Text/tool executions; aborting the request stream cancels its execution                                |
| `agent`   | Additionally permits explicit CLI-agent requests with local filesystem access; also requires `execute` |
| `admin`   | Account/model/settings/key management, service shutdown and cancellation by execution ID               |

Telemetry scopes grant visibility to retained execution metadata across clients. They are not per-project tenancy. Disabling strict read scopes in Settings lets any authenticated key read telemetry; it never disables authentication or agent/admin checks.

## Native responses

```json
{
  "model": "auto",
  "input": "Inspect this function for bugs: ...",
  "task_type": "debugging",
  "stream": true,
  "max_output_tokens": 2048,
  "preferences": {
    "preset": "best_quality",
    "allow_fallback": true,
    "allow_billable": false
  }
}
```

POST to `/v1/responses`. Use an `input` array of message objects for multi-turn structured input and tool results; `instructions` supplies a system prompt. The top-level `messages` field belongs to `/v1/chat/completions`, not the native endpoint. `tools`, `tool_choice`, `response_format`, `reasoning_effort` and image content are negotiated against adapter capabilities. Unsupported requirements yield a structured error or an eligible fallback, never a silent downgrade. Explicit model IDs may be a harness model key or a provider/model ID from `/v1/models`.

SSE data frames are typed JSON: `started`, `text_delta`, `reasoning_delta`, `tool_call`, `routing_changed`, `completed`, or `failed` (see the SDK's `ExecEvent` union for all events). Completion includes consistent result, usage, cost, model, timing and routing metadata. Frames must be parsed across arbitrary byte boundaries. Disconnecting the stream cancels its request. Events are live; reconnecting does not replay an event log. Use `/v1/executions` to reconcile history.

A native response is **not** the OpenAI Responses object. Magpie does not implement OpenAI hosted tools, conversations, background response storage or response retrieval APIs.

## Chat Completions compatibility

`POST /v1/chat/completions` supports text/messages, streaming deltas, image content for vision-capable adapters, function tools/tool results, JSON/schema formats, max tokens, temperature, and negotiated reasoning effort. `GET /v1/models` provides compatible model listings with additional Magpie metadata. Tool calling remains a client-controlled loop.

Configure an OpenAI-compatible client with `base_url=http://127.0.0.1:7878/v1` and an integration key. This does not imply compatibility with every IDE or coding product. A tool must support custom endpoints and the implemented Chat Completions subset. Unsupported audio, image generation, batch APIs and hosted tools are outside this API.

## TypeScript SDK

```ts
import { MagpieClient } from "@magpie/sdk";
const client = new MagpieClient({
  baseUrl: "http://127.0.0.1:7878",
  apiKey: process.env.MAGPIE_API_KEY!,
});
const abort = new AbortController();
for await (const event of client.stream({ model: "auto", input: "Summarise ..." }, abort.signal)) {
  if (event.type === "text_delta") process.stdout.write(event.text);
}
```

The SDK is ESM, includes TypeScript declarations, validates health and handles SSE UTF-8/chunk boundaries and cancellation. Build/pack it from `packages/sdk` until a package is published. For exact methods and schemas see `packages/sdk/src/client.ts` and `types.ts`.

## Agent workflows

Only trusted local tools should receive agent permission. A working directory is task context, not an OS-enforced filesystem jail; provider CLI configuration and administrator policies still apply.

```json
{
  "model": "codex_cli/MODEL_ID",
  "input": "Review this repository and report defects",
  "agent": { "working_dir": "/absolute/path/to/repo", "allow_writes": false },
  "preferences": { "allow_fallback": false }
}
```

Codex and Gemini CLI executions require this explicit context. Claude Code can also run plain generation with tools disabled. CLI requests can read local context as permitted by the CLI, and CLI-owned history may exist independently of Magpie retention. Review provider settings before granting agent access. File edits require `allow_writes: true`; interactive approvals cannot be completed through Magpie.

The CLI equivalent is `magpie run --model codex_cli/MODEL_ID --cwd /path/to/repo "Review this repository"`; add `--allow-writes` only when intended. `MAGPIE_API_KEY` and `MAGPIE_URL` keep CLI/MCP calls scoped to the supplied integration key. Without these variables the standalone CLI acts as the local owner.

## Read and management endpoints

| Endpoint                                             | Purpose                                          |
| ---------------------------------------------------- | ------------------------------------------------ |
| `GET /health`                                        | Minimal unauthenticated product/version/liveness |
| `GET /v1/status`                                     | Authenticated runtime status                     |
| `GET /v1/models`                                     | Available models and capability metadata         |
| `GET /v1/providers`                                  | Descriptors and connection states                |
| `GET /v1/limits`                                     | Known windows, provenance and resets             |
| `POST /v1/route`                                     | Dry-run model selection                          |
| `GET /v1/usage/summary`, `/timeseries`, `/breakdown` | Local usage analytics                            |
| `GET /v1/usage/provider-reports`                     | Separate reported account usage when available   |
| `GET /v1/usage/export`                               | Metadata export                                  |
| `GET /v1/executions`, `/active`, `/{id}`             | Execution metadata and inspector                 |
| `GET /v1/events`                                     | Authenticated live SSE telemetry                 |
| `GET/PUT /v1/routing`, `/v1/settings`                | Saved configuration; writes require admin        |
| `GET/POST /v1/keys`, `DELETE /v1/keys/{id}`          | Admin client-key management                      |

Usage query parameters include `range=1h|24h|7d|30d|custom`, `from`/`to` in Unix milliseconds, `provider`, `model_key`, `bucket_ms` for time series, and `group_by` for breakdowns. Error responses have an `error` object with a stable type and safe message. Retry only according to the returned error and your side-effect policy.

## Authentication profile management

These endpoints require an admin key. Each connection has a distinct account ID; requests pinned to that account remain pinned unless fallback is enabled.

```ts
// Existing default CLI connections continue to work without auth_mode.
const profile = await client.connectProvider({
  kind: "codex_cli", label: "Work", auth_mode: "isolated",
}); // status: needs_auth; no executable models yet
const { auth_url } = await client.loginProvider(profile.id);
// Open auth_url in a browser. The harness detects completion independently.
// Observe account_updated events or GET /v1/providers for connected/needs_auth.

await client.connectProvider({
  kind: "claude_code", label: "Backup", auth_mode: "saved_token",
  oauth_token: tokenFromOfficialSetupToken,
});
```

`POST /v1/providers/{id}/login` only starts sign-in for an isolated Codex profile. `POST /v1/providers/{id}/verify` checks connection state without inference. `PATCH /v1/providers/{id}` accepts `oauth_token` for a saved Claude token or `api_key` for an API connection; these secrets are never returned. Authentication mode and profile paths cannot be changed through `options`. `DELETE /v1/providers/{id}` removes the saved credential; isolated Codex profiles are logged out through the official CLI. Cancel active executions before disconnecting. See [provider limitations](PROVIDERS.md#saved-authentication-profiles) for renewal, quota and safe fallback semantics.

## MCP

`magpie mcp` is a stdio MCP bridge exposing model listing, routing and execution tools to compatible MCP clients. Set `MAGPIE_API_KEY` to a dedicated scoped key in that client's process environment; no separate remote MCP server is required. This is an optional developer integration, not a claim of verified compatibility with every named IDE.
