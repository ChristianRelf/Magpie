# TypeScript SDK and MCP

The TypeScript SDK provides native execution, management, telemetry, and streaming. The optional MCP bridge exposes a smaller set of tools over local stdio. Both use the same authenticated harness.

## Build and install the SDK

The package is ESM with TypeScript declarations. This repository does not assume it has been published to npm. From the workspace root:

```bash
pnpm install --frozen-lockfile
pnpm --filter @magpie/sdk build
pnpm --filter @magpie/sdk pack
```

Use the tarball path printed by `pack` in your consuming application:

```bash
# In your application's directory; replace this with the actual packed file.
pnpm add /absolute/path/to/magpie-sdk-VERSION.tgz
```

Within this monorepo, packages can use the workspace dependency. For external apps, the built/packed package avoids relying on the desktop's special source-resolution settings. Use a package/service version that supports the features you call, especially saved authentication profiles introduced in 0.1.3.

## Connect and check

```ts
import { MagpieClient, MagpieError } from "@magpie/sdk";

const apiKey = process.env.MAGPIE_API_KEY;
if (!apiKey) throw new Error("Set MAGPIE_API_KEY to a Magpie integration key");

const client = new MagpieClient({
  baseUrl: process.env.MAGPIE_URL ?? "http://127.0.0.1:7878",
  apiKey,
});

await client.health();
const status = await client.status();
const models = await client.models();
console.log(status.version, models.length);
```

The base URL is the origin, without `/v1`; methods add their paths. `health()` checks the Magpie product/API identity. `models()` returns model metadata without the virtual `auto` list entry. An optional `fetch` implementation can be supplied to the constructor for your runtime/testing needs.

Use the SDK from an environment that supports the required fetch/stream APIs. Browser calls remain subject to Magpie's CORS policy; this is not an invitation to put an admin key into a public website bundle.

## Preview and respond

```ts
const decision = await client.route({
  model: "auto",
  input: "Summarise the following notes: ...",
  task_type: "summarisation",
  preferences: { preset: "economical" },
});
console.log(decision.candidates.map((candidate) => candidate.reasons));

const result = await client.respond({
  model: "auto",
  input: "Explain a local API in one sentence.",
  max_output_tokens: 128,
});
console.log(result.output_text);
```

The route call is a preview; `respond` runs inference and can use allowance/money. The response is the native Magpie result, not an OpenAI Responses object. Multi-turn native input is an `input` array; see [API reference](API-reference.md).

## Stream and cancel

```ts
const controller = new AbortController();
const timer = setTimeout(() => controller.abort(), 60_000);
let completed = false;
try {
  for await (const event of client.stream(
    { model: "auto", input: "Explain streaming in two sentences." },
    controller.signal,
  )) {
    if (event.type === "text_delta") process.stdout.write(event.text);
    if (event.type === "routing_changed") console.error(event.reason);
    if (event.type === "failed") throw new Error(event.error.message);
    if (event.type === "completed") completed = true;
  }
  if (!completed) throw new Error("Stream ended without a completed event");
} finally {
  clearTimeout(timer);
}
```

Aborting cancels the response stream. It does not undo earlier provider/agent actions. `client.cancel(executionId)` calls the administrative cancellation endpoint and needs `admin`; ordinary scoped clients cancel their own stream instead.

The SDK parses SSE across UTF-8/chunk boundaries. A `failed` event is part of an admitted stream, so handle it explicitly. It is different from an HTTP failure thrown before streaming begins.

## Handle HTTP errors

```ts
try {
  await client.status();
} catch (error) {
  if (error instanceof MagpieError) {
    console.error(error.status, error.type, error.message, error.retryAfter);
  } else {
    throw error;
  }
}
```

`MagpieError` exposes status/type/message and optional retry-after seconds. Invalid keys, insufficient scopes, and unsupported requests need a configuration fix, not automatic retry. An agent request can have side effects before a network failure; apply your application's retry policy deliberately.

## Common method groups

| Area                 | Methods                                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------ |
| Execution            | `route`, `respond`, `stream`, `cancel`                                                     |
| Runtime/inventory    | `health`, `status`, `models`, `providers`, `detectClis`, `limits`                          |
| Provider management  | `connectProvider`, `updateProvider`, `deleteProvider`, `verifyProvider`, `refreshProvider` |
| Models               | `setModelPreference`                                                                       |
| Usage                | `usageSummary`, `usageTimeseries`, `usageBreakdown`, `providerReports`, `exportUsage`      |
| History              | `executions`, `activeExecutions`, `execution`, `clearHistory`                              |
| Configuration        | `settings`, `updateSettings`, `routing`, `updateRouting`                                   |
| Keys                 | `keys`, `createKey`, `revokeKey`                                                           |
| Events/notifications | `events`, `notifications`, `markNotificationsRead`                                         |
| Lifecycle            | `shutdown`                                                                                 |

Read/write scopes mirror the HTTP API. For settings/routing updates, fetch and modify the full object; these methods call PUT replacement endpoints. Saved-profile `loginProvider` support is version-specific and documented separately.

## Configure a stdio MCP client

Install the standalone CLI and start the harness. Create a dedicated key with `read` and `execute`. Adapt this template to the configuration schema of your MCP client:

```json
{
  "mcpServers": {
    "magpie": {
      "command": "/absolute/path/to/magpie",
      "args": ["mcp"],
      "env": {
        "MAGPIE_URL": "http://127.0.0.1:7878",
        "MAGPIE_API_KEY": "REPLACE_WITH_A_PRIVATE_INTEGRATION_KEY"
      }
    }
  }
}
```

Use your client's private secret mechanism if supported and keep the real token out of Git. On Windows, the command points to `magpie.exe`; JSON paths need escaped backslashes or a supported forward-slash form. Avoid a wrapper that writes non-protocol text to stdout.

With a key supplied, the bridge does not automatically start a stopped harness. Start Magpie first or use its background/login settings. This is a local stdio bridge, not a hosted HTTP MCP endpoint.

## MCP tools and boundaries

| Tool                 | Inputs                                                               | Result                                |
| -------------------- | -------------------------------------------------------------------- | ------------------------------------- |
| `magpie_generate`    | Required `prompt`; optional `system`, `model`, `task_type`, `preset` | Generated output text                 |
| `magpie_list_models` | None                                                                 | Model/capability availability summary |
| `magpie_usage`       | Optional `range`: `1h`, `24h`, `7d`, `30d`                           | Local usage summary                   |
| `magpie_limits`      | None                                                                 | Known provider windows                |

The implemented bridge does not expose a separate route-preview tool or agent working-directory parameter. `magpie_generate` can incur cost and is not a dry run. Use native API/CLI for explicit Codex/Gemini agent tasks. Granting an `agent` key alone does not add absent tool arguments.

For missing tools, verify the client's stdio configuration and executable path. For tool-call errors, check the local endpoint, key/scopes, route eligibility, and model ID. A working MCP handshake does not certify every integration client's behaviour.

Source: [SDK client](https://github.com/ChristianRelf/Magpie/blob/main/packages/sdk/src/client.ts), [types](https://github.com/ChristianRelf/Magpie/blob/main/packages/sdk/src/types.ts), [MCP tools](https://github.com/ChristianRelf/Magpie/blob/main/apps/cli/src/mcp.rs).
