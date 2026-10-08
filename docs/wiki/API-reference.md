# API reference

Default origin: `http://127.0.0.1:7878`. This page documents the published 0.1.3 API and identifies the profile endpoint added in that version. The native response protocol is Magpie-specific; Chat Completions is a supported compatibility subset.

## Authentication and request conventions

Send `Authorization: Bearer <Magpie integration key>`. The server also recognises `x-api-key`. Every route except `/health` requires authentication. The owner session/admin scope can manage the harness; external clients should use dedicated scoped keys.

Use JSON request bodies with `Content-Type: application/json`. IDs in URL path segments must be URL-encoded. The server's JSON body limit is 32 MiB. Loopback Host checks and restricted browser CORS origins apply independently of token validity.

Examples assume `MAGPIE_URL` is the root origin and `MAGPIE_API_KEY` is a suitable client key. See [Getting started](Getting-started.md) for shell setup.

## Execution endpoints

| Method and path                   | Required scope                              | Purpose                                            |
| --------------------------------- | ------------------------------------------- | -------------------------------------------------- |
| `POST /v1/responses`              | `execute`; also `agent` for agent options   | Native JSON result or SSE                          |
| `POST /v1/chat/completions`       | `execute`; also `agent` for agent extension | Chat Completions subset                            |
| `POST /v1/route`                  | `read`                                      | Classification/selection preview without execution |
| `POST /v1/executions/{id}/cancel` | `admin`                                     | Cancel an active execution by ID                   |

### Native request fields

| Field                             | Shape / default                                      | Notes                                              |
| --------------------------------- | ---------------------------------------------------- | -------------------------------------------------- |
| `model`                           | String, default `auto`                               | Discovered key/public ID or automatic routing      |
| `input`                           | Required string or array of message objects          | Use an array for history/tool results              |
| `instructions`                    | Optional string                                      | System instructions; `system` is an alias          |
| `task_type`                       | Optional string                                      | Canonical task or supported alias                  |
| `stream`                          | Boolean, default `false`                             | Named native SSE events when true                  |
| `max_output_tokens`               | Optional integer                                     | Adapter/model output constraints still apply       |
| `temperature`                     | Optional number from 0 to 2                          | Provider support varies                            |
| `reasoning_effort`                | Optional string                                      | Negotiated where supported                         |
| `tools`                           | Array of function definitions                        | Client owns execution of returned calls            |
| `tool_choice`                     | `auto`, `none`, `required`, or named function object | Capability-dependent                               |
| `response_format` / `text_format` | Text/JSON/schema object                              | Capability-dependent                               |
| `preferences`                     | Routing preference object                            | [Routing reference](Routing-and-fallback.md)       |
| `agent`                           | `working_dir` and optional `allow_writes`            | Explicit filesystem workflow; writes default false |
| `metadata`                        | Optional JSON                                        | Avoid secrets and personal data you do not need    |

**For native multi-turn input, use `input: [...]`.** A top-level `messages` field is the Chat Completions shape, not the native request field. Unknown fields should not be assumed to activate a feature.

```json
{
  "model": "auto",
  "instructions": "Answer concisely.",
  "input": [
    { "role": "user", "content": "What is a queue?" },
    { "role": "assistant", "content": "A first-in, first-out collection." },
    { "role": "user", "content": "Give one software example." }
  ],
  "max_output_tokens": 256,
  "stream": false,
  "preferences": { "allow_fallback": false }
}
```

Messages accept text content and supported content arrays. Roles normalise `user`, `assistant`, `system`, and `tool`, with selected compatibility aliases. Image content requires a vision-capable route and a supported image URL/data representation. Tools/schema formats must also match adapter capabilities.

### Native results

A successful non-streaming response includes `id`, `object: "response"`, `status: "completed"`, selected `model` object, `output_text`, `tool_calls`, `finish_reason`, `usage`, `cost`, `duration_ms`, `time_to_first_token_ms`, `attempts`, `routing`, `routing_changes`, and `created_at` as available.

The streaming `completed` event contains an execution result whose identifier field is `execution_id`. Do not assume it is byte-for-byte the same wrapper as the non-streaming response. Handle the typed contracts in the SDK. Native streaming events are documented in [Streaming and tools](Streaming-and-tools.md).

## Chat Completions compatibility

Use client base URL `http://127.0.0.1:7878/v1` when the client appends `/chat/completions`. The payload uses `messages`, not native `input`. Supported features include text, supported image inputs, function tools/results, streaming, max-token controls, temperature, supported JSON formats, and reasoning effort.

Magpie-specific options can be supplied in the `magpie` extension object, containing `task_type`, `preferences`, and `agent`. Capability support is adapter-specific. Full Responses semantics, hosted tools, audio/image generation, batches, conversations, and stored-response retrieval are not implemented by this compatibility layer.

## Inventory and provider management

| Method and path                   | Scope   | Result/action                                        |
| --------------------------------- | ------- | ---------------------------------------------------- |
| `GET /health`                     | None    | Minimal product/status/application/API version       |
| `GET /v1/status`                  | `read`  | Runtime, inventory counts, data directory, principal |
| `GET /v1/models`                  | `read`  | Compatible list with additional `magpie` metadata    |
| `POST /v1/model-preferences`      | `admin` | Set `{key, preference}`                              |
| `GET /v1/providers`               | `read`  | Accounts and provider descriptors                    |
| `POST /v1/providers`              | `admin` | Connect a provider                                   |
| `GET /v1/providers/cli`           | `read`  | Detect supported official CLIs                       |
| `PATCH /v1/providers/{id}`        | `admin` | Update account configuration/credential              |
| `DELETE /v1/providers/{id}`       | `admin` | Disconnect account and remove its stored credential  |
| `POST /v1/providers/{id}/verify`  | `admin` | Verify connection                                    |
| `POST /v1/providers/{id}/refresh` | `admin` | Refresh supported inventory/account information      |
| `GET /v1/limits`                  | `read`  | Known windows, states, provenance, reset timestamps  |
| `POST /v1/providers/{id}/login`   | `admin` | **Since 0.1.3:** isolated Codex browser sign-in      |

Provider create fields include `kind`, `label`, `base_url`, `api_key`, and provider-specific options. Never log request bodies containing secrets. For new authentication modes, see [Saved sign-ins](Saved-sign-ins.md) and use a compatible SDK/service pair.

## Telemetry and history

| Method and path                  | Scope   | Purpose                                               |
| -------------------------------- | ------- | ----------------------------------------------------- |
| `GET /v1/usage/summary`          | `read`  | Current/previous period local totals and active count |
| `GET /v1/usage/timeseries`       | `read`  | Time buckets, optionally grouped                      |
| `GET /v1/usage/breakdown`        | `read`  | Local aggregates by dimension                         |
| `GET /v1/usage/provider-reports` | `read`  | Separate cached supported account reports             |
| `GET /v1/usage/export`           | `read`  | Local execution metadata as CSV/JSON                  |
| `DELETE /v1/history`             | `admin` | Delete retained execution history                     |
| `GET /v1/executions`             | `read`  | Paginated/filterable records and total                |
| `GET /v1/executions/active`      | `read`  | Active execution summaries                            |
| `GET /v1/executions/{id}`        | `read`  | Stored record; retained content requires admin        |
| `GET /v1/events`                 | `read`  | Live harness telemetry SSE, without replay            |

Usage ranges: `1h`, `24h` (default), `7d`, `30d`, `90d`, or `custom`. For custom bounds send `from` and optional `to` as **Unix milliseconds**; `to` must be later than `from`. Aggregate filters include `provider`, `account_id`, and `model_key`. `group_by` accepts `none`, `provider`, `model`, `account`, `task`, or `status`. Explicit `bucket_ms` is accepted for time series when at least 60,000; otherwise the server chooses a bucket size and aligns the starting boundary.

```bash
curl --fail-with-body "$MAGPIE_URL/v1/usage/summary?range=7d" \
  -H "Authorization: Bearer $MAGPIE_API_KEY"
curl --fail-with-body "$MAGPIE_URL/v1/usage/breakdown?range=7d&group_by=model" \
  -H "Authorization: Bearer $MAGPIE_API_KEY"
curl --fail-with-body "$MAGPIE_URL/v1/executions?limit=20&offset=0&status=failed" \
  -H "Authorization: Bearer $MAGPIE_API_KEY"
```

Execution list filters also include `task`, `status`, `from`, and `to`. Use `limit` and `offset` with the returned `total`. The store caps each list request; paginate instead of expecting an arbitrarily large `limit` to return all records. Provider account reports are separate from these local filters.

Export accepts range/bounds and `format=csv|json`; CSV is default. The current export implementation selects by date and a fixed internal limit, rather than applying all aggregate filters or providing unlimited export. Split large exports into smaller time ranges and verify counts. Exports omit prompt/response content and provider secrets.

## Configuration endpoints

| Method and path               | Scope   | Purpose                                      |
| ----------------------------- | ------- | -------------------------------------------- |
| `GET /v1/settings`            | `read`  | `{settings, autostart_enabled}`              |
| `PUT /v1/settings`            | `admin` | Replace saved settings object                |
| `GET /v1/routing`             | `read`  | Routing configuration                        |
| `PUT /v1/routing`             | `admin` | Replace routing configuration                |
| `GET /v1/keys`                | `admin` | Key metadata                                 |
| `POST /v1/keys`               | `admin` | Create `{name, scopes}`; token returned once |
| `DELETE /v1/keys/{id}`        | `admin` | Revoke key                                   |
| `GET /v1/notifications`       | `read`  | Notifications                                |
| `POST /v1/notifications/read` | `read`  | Mark notifications read                      |
| `POST /v1/admin/shutdown`     | `admin` | Request graceful shutdown                    |

**PUT is not PATCH.** Read the existing settings/routing object, change the intended fields, then send the full object. Omitted settings can deserialize to defaults and overwrite unrelated preferences. Settings changes can have side effects such as startup registration, retention cleanup, and content purging; see [Settings and storage](Settings-and-storage.md).

## Error reference

Application errors normally use `{"error":{"message":"...","type":"...","code":"..."}}`. They may include a reset time and `Retry-After` header. Host rejection, malformed bodies, body-size limits, and framework errors can have a different/plain response body, so clients should handle non-JSON failures too.

| HTTP | Example type                                                                      | Interpretation / action                               |
| ---- | --------------------------------------------------------------------------------- | ----------------------------------------------------- |
| 400  | `invalid_request`, `unsupported`, `context_length`                                | Fix request/options/context; blind retry is unhelpful |
| 401  | `invalid_api_key`                                                                 | Missing, wrong, or revoked Magpie key                 |
| 403  | `insufficient_scope`                                                              | Key lacks the required permission                     |
| 404  | `model_not_found`, `not_found`                                                    | Model/record/key not present                          |
| 422  | `refused`                                                                         | Provider refusal; inspect the message                 |
| 429  | `rate_limited`, `insufficient_quota`                                              | Wait/review quota and retry metadata                  |
| 499  | `cancelled`                                                                       | Request was cancelled                                 |
| 502  | `upstream_authentication`, `permission_denied`, `network`, `provider_unavailable` | Upstream account/network/provider failure             |
| 503  | `local_dependency`, `no_eligible_model`                                           | Local dependency or routing availability problem      |
| 504  | `timeout`                                                                         | Provider/attempt timeout                              |
| 500  | `internal`                                                                        | Inspect redacted logs and report a reproduction       |

For a stream already admitted with HTTP 200, execution failure arrives as an event. A client that only checks HTTP status will miss it. See [Troubleshooting](Troubleshooting.md) for recovery procedures.

Source of truth: [routes](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-api/src/lib.rs), [wire conversion](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-api/src/convert.rs), [error mapping](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-api/src/error.rs), and [SDK contracts](https://github.com/ChristianRelf/Magpie/blob/main/packages/sdk/src/types.ts).
