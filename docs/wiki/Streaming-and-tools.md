# Streaming, tools, and cancellation

Magpie has three distinct event surfaces: native execution SSE, Chat Completions streaming, and live harness telemetry. Parse the protocol you requested rather than assuming every stream contains OpenAI-style chunks.

## Native execution events

Send `stream: true` to `POST /v1/responses`. Each SSE data field contains one typed JSON event. The named `event:` field matches the execution event type.

| Event             | Important fields                           | Client action                                             |
| ----------------- | ------------------------------------------ | --------------------------------------------------------- |
| `started`         | `execution_id`, `model`, `task`, `reasons` | Track the execution and chosen connection                 |
| `routing_changed` | `from`, `to`, `reason`                     | Update the visible selected route                         |
| `text_delta`      | `text`                                     | Append text                                               |
| `reasoning_delta` | `text`                                     | Handle separately if supported/needed                     |
| `tool_call`       | `call` with ID/name/arguments              | Collect a function call; do not execute unvalidated input |
| `completed`       | `result`                                   | Record success, usage, finish reason, timings             |
| `failed`          | `execution_id`, `error`                    | Treat the execution as failed                             |

Illustrative native frame:

```text
event: text_delta
data: {"type":"text_delta","text":"Hello"}

```

SSE frames end with a blank line. A network chunk can contain part of a frame, several frames, or part of a UTF-8 character. Do not JSON-parse arbitrary transport chunks. Keep-alive comments are not execution events. The API sends keep-alives approximately every 15 seconds while a stream is open.

Use `curl --no-buffer` for terminal inspection. Prefer the SDK's streaming parser for application code. It handles frame/UTF-8 boundaries and cancellation; raw browser `EventSource` does not directly support this authenticated POST workflow.

## HTTP success versus execution success

Validation/routing errors can produce an ordinary non-2xx JSON error before the stream starts. Once SSE headers have been sent, later provider failure appears as `failed`, even though HTTP status is 200. A successful client waits for the terminal `completed` event rather than declaring success at the first text token.

If a stream closes without a terminal event, treat the result as incomplete. Do not immediately replay an agent or tool workflow: it may already have produced side effects. Reconcile the execution in Activity or `/v1/executions/{id}` when history is available.

## Chat Completions streaming

`POST /v1/chat/completions` with `stream: true` emits compatible chunk data and a `[DONE]` terminator. The native endpoint does not use that exact format. Choose a parser that matches your client protocol and handle error chunks/connection failure as supported by the compatibility implementation.

Tool arguments can arrive across deltas in compatible clients. Assemble the complete call before parsing JSON or executing a function. Preserve tool-call IDs across the next turn.

## Client-owned function tools

Magpie returns calls; your application decides whether/how to execute them. Define a small tool with a schema:

```json
{
  "model": "auto",
  "input": "What is the weather in London?",
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "get_weather",
        "description": "Look up weather for a named city.",
        "parameters": {
          "type": "object",
          "properties": { "city": { "type": "string" } },
          "required": ["city"],
          "additionalProperties": false
        }
      }
    }
  ],
  "tool_choice": "auto",
  "stream": false
}
```

Route this to a tool-capable model. The returned call's `arguments` is a JSON-encoded string. Parse it, validate it against the schema, enforce your application's permissions, and execute only the intended function. A model-supplied name/path/command is not authorisation by itself.

Send the original history, assistant call, and tool result back as native `input` messages. This example assumes the returned call was named `get_weather` with ID `call_example`; substitute the actual call and actual result:

```json
{
  "model": "auto",
  "input": [
    { "role": "user", "content": "What is the weather in London?" },
    {
      "role": "assistant",
      "content": null,
      "tool_calls": [
        {
          "id": "call_example",
          "type": "function",
          "function": { "name": "get_weather", "arguments": "{\"city\":\"London\"}" }
        }
      ]
    },
    { "role": "tool", "tool_call_id": "call_example", "content": "Example tool result supplied by your application." }
  ],
  "stream": false
}
```

Include tool definitions again if further calls are permitted. Keep the same account/model when continuity requires it. Magpie does not store an OpenAI-style conversation for you; send the necessary history each turn.

## Structured output

Use `response_format`/`text_format` with a supported JSON format and an eligible model. For a schema request:

```json
{
  "model": "auto",
  "input": "Extract the project name from: Magpie is a local AI harness.",
  "response_format": {
    "type": "json_schema",
    "json_schema": {
      "name": "project",
      "strict": true,
      "schema": {
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"],
        "additionalProperties": false
      }
    }
  }
}
```

Validate the returned value in your application. Adapter support and provider semantics still matter; compatibility should not be inferred from the URL alone.

## CLI agents are different

Official Codex/Claude/Gemini agents own their local tool loop. Supply explicit `agent` context and permissions; you are not providing the same function-tool loop as above. Magpie cannot finish interactive approvals through an HTTP request. Write-capable agent tasks are not blindly replayed after failure.

## Cancellation

Use an `AbortController` with the SDK or close your response stream to cancel its execution. Administrative clients can call `POST /v1/executions/{id}/cancel`. A normal execute-scoped client cannot cancel an unrelated execution by ID.

Cancellation does not roll back a completed API action, remove emitted text, reverse files already edited by an agent, or guarantee a provider has charged nothing. Inspect results before reissuing a side-effecting task.

## Live telemetry and reconnects

`GET /v1/events` emits harness-level changes for UI/cache updates and requires `read`. It is live, not a durable log with resumable IDs. After reconnecting, refresh providers, limits, active executions, and retained history as appropriate. Do not use event delivery alone as the authoritative store of completed work.

Related: [SDK and MCP](SDK-and-MCP.md), [API reference](API-reference.md), [Routing and fallback](Routing-and-fallback.md).
