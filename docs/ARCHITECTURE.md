# Architecture

## Process boundary

The Tauri executable has two entry modes: the desktop window, and `--harness`. Daemon mode starts Rust/Tokio directly, before any webview, tray or display connection is initialised. The standalone `magpie serve` CLI runs the same runtime. The desktop discovers a healthy authenticated service in its data directory or launches the daemon detached. Version, API version and health are checked before connecting.

The window holds a scoped-to-the-owner administrative connection in memory. External clients receive revocable keys and never need provider credentials. Closing the window stops the service by default; keeping it running, minimising to tray and start on login are explicit settings. Shutdown cancels executions and drains the HTTP server. A filesystem lock prevents duplicate services sharing the database. Incomplete executions are recovered as interrupted after a crash.

## Rust workspace

| Crate                    | Responsibility                                                                          |
| ------------------------ | --------------------------------------------------------------------------------------- |
| `magpie-core`            | Provider, capability, execution, usage, limit and settings contracts                    |
| `magpie-security`        | OS secret store, private files, key generation/hash verification and redaction          |
| `magpie-store`           | SQLite migrations, account/model configuration, execution history and analytics queries |
| `magpie-providers`       | Independent adapters, HTTP/SSE parsing, official CLI delegation and model metadata      |
| `magpie-router`          | Deterministic task classification, eligibility checks, ranking and routing explanations |
| `magpie-engine`          | Provider lifecycle, cancellation, retries/fallback, telemetry and notifications         |
| `magpie-api`             | Authenticated Axum routes, native SSE and compatibility conversion                      |
| `magpie-runtime`         | Discovery, detached process launch, locking and daemon lifecycle                        |
| `apps/cli`               | `magpie` commands and optional stdio MCP server                                         |
| `apps/desktop/src-tauri` | Native window/tray, login assistance, lifecycle commands, exports and updater           |

The original project grouped persistence, usage tracking and quotas into cohesive crates rather than separate micro-crates. UI mutations call typed SDK methods; they do not execute providers. `packages/sdk` contains the shared TypeScript contracts, fetch client and streaming parser.

## Execution

1. Host validation and bearer authentication run before protected handlers. Endpoint scopes distinguish read, execute, agent and admin access.
2. Input is validated and classified using request metadata and cheap heuristics. No paid model is called to classify a request.
3. Eligibility checks enforce enabled/connected state, capabilities, context, limits, billing policy and explicit agent context.
4. Ranking combines quality, latency history, cost estimate, quota preservation, task rules and user priorities. Every candidate has reasons; rejected candidates record why.
5. The engine creates an execution, obtains an async adapter, forwards events and records provider-reported tokens and measured timing.
6. Only retryable failures before meaningful output/tool events permit safe fallback. Agent runs and external side effects are not blindly replayed. Subscription-to-API fallback requires opt-in.
7. Completion stores metadata, updates known limits and emits structured events. Request/response content is omitted unless retention was explicitly enabled.

Client-defined tool calls are returned to the requesting application. That application executes its tools and submits tool-result messages for the next step. Native CLI agents own their tool loops; Magpie does not silently run arbitrary tools from an ordinary API response.

## Storage and telemetry

SQLite uses migrations applied transactionally, WAL, foreign keys and indexed execution timestamps/model/account identifiers. Tables hold accounts (no provider keys), discovered models, model preferences, executions, limits, key-value configuration, hashed API clients and notifications. Provider credentials use Keychain, Windows Credential Manager or Secret Service. The service's bootstrap token is stored in its private data directory.

Usage has explicit provenance: provider-reported token counts, locally calculated durations/costs, estimated catalogue prices, or unavailable fields. Provider account reports remain separate from local harness activity. Calendar buckets are UTC; a blank cell means no retained harness activity for that day. Default history retention is 90 days; increase it to retain a full year.

Events invalidate relevant React Query caches. Live telemetry polling respects the configured refresh interval; provider-limit polling backs off and does not generate prompts. Large activity lists are virtualised. Recharts is used for time-series and calendar rendering, with accessible activity data and reduced-motion support.

## Extension points

Implement `ProviderAdapter` with verification, discovery, execution and optional limit/account usage methods; register a provider descriptor in the provider registry. Adapters negotiate concrete capabilities and retain provider-specific metadata. Unknown quotas are not synthesised. Multi-account API providers use separate secret-store entries; official CLI connections follow the CLI's own active-account/session constraints.

Settings are version-tolerant JSON records with defaults. API breaking changes increment `API_VERSION`. A release must update the workspace/package/Tauri version together. No database service, Docker container or hosted Magpie backend is needed.
