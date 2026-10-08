# Codex CLI

Magpie delegates Codex work to the official CLI/app-server. You can use a shared local login or, since v0.1.3, separate browser sign-ins. Older v0.1.2 connections use the shared login; see [Saved sign-ins](Saved-sign-ins.md).

## Connect and discover

Install the official CLI, complete its normal authentication, and run Magpie under the same OS user. Then connect:

```bash
codex --version
magpie connect codex_cli
magpie providers
magpie models --available
```

Magpie uses app-server model discovery rather than assuming a fixed list of current Codex models. Select an ID returned by your installed CLI/account. A version mismatch can affect discovery or account reports even if the executable itself launches.

A Codex CLI connection and an OpenAI API connection are different adapters. Magpie's OpenAI API adapter uses Chat Completions and filters Responses-only Codex model families. Connecting an OpenAI key does not make the native Magpie endpoint a full OpenAI Responses server.

## Execute with explicit agent context

Codex can access local files and its own tools. Magpie therefore rejects ordinary generation requests without explicit working-directory context. An external client needs `execute` and `agent` scopes.

```json
{
  "model": "codex_cli/MODEL_ID",
  "input": "Review this repository and list the three highest priority defects.",
  "agent": {
    "working_dir": "/absolute/path/to/repository",
    "allow_writes": false
  },
  "preferences": { "allow_fallback": false },
  "stream": true
}
```

Replace `MODEL_ID`; use an existing directory. CLI equivalent:

```bash
magpie run 'Review this repository and list the three highest priority defects' \
  --model codex_cli/MODEL_ID --cwd /absolute/path/to/repository
```

The CLI command inherits configured fallback preferences; use the native example if you need to disable fallback for that request. Add `--allow-writes` only for an intended editing task. The provider CLI's configuration and policies still apply, and a working directory does not isolate the filesystem.

The current MCP generate tool has no working-directory argument, so it cannot express this explicit agent request. Use the native API or CLI for it.

## Account history versus local requests

For compatible accounts and CLI versions, Magpie fetches official app-server `account/usage/read` reports. The report contains daily token buckets and may include activity from external Codex clients. It is separate from executions routed through Magpie.

In Analytics, select the account usage source to inspect external Codex activity, or **Magpie requests** to inspect this harness's execution details. Do not add their totals. Account reports do not supply per-request model breakdowns, latency, request counts, or costs; Magpie does not synthesize those fields.

Daily buckets use UTC. Today is a reported snapshot, not a live token stream. Missing dates are unknown rather than fabricated zero usage. Snapshot handling avoids adding repeated observations of the same daily totals.

## Refresh behaviour

Supported account reports are monitored approximately every two minutes, subject to the provider-polling setting and failure backoff. Refresh usage requests another supported report; it does not force the provider to update its own data or run inference. Account usage retrieval is independent of quota endpoint success.

The documented implementation record includes a successful read-only account-history check with Codex CLI 0.156.1 on one development machine. That is evidence for that environment, not a guarantee for arbitrary accounts, future/older CLI versions, or API-key-only authentication.

## Troubleshoot Codex

| Symptom                                        | Likely distinction                          | Check and fix                                                      |
| ---------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------ |
| “Requires an explicit agent working directory” | Request policy, not missing login           | Supply `agent` and the required scopes                             |
| Model not found                                | Stale/invented model ID or wrong connection | Refresh discovery and copy the returned ID/key                     |
| External sessions absent from chart            | Local source selected                       | Select reported account usage                                      |
| Account history unavailable                    | CLI/account endpoint unsupported            | Read the report status; check compatible CLI/authentication        |
| Chart lags active work                         | Provider snapshots and polling              | Inspect observation time; refresh once and allow publication delay |
| Login works in terminal, not service           | User/environment/path mismatch              | Inspect CLI discovery and the service launch environment           |
| Separate login option missing                  | Version mismatch                            | It is not included in v0.1.2; compare app and harness versions     |

Do not copy hidden authentication files into Magpie or manufacture usage from session transcripts. Use the official authentication path and report endpoint. For more diagnostics, read [Troubleshooting](Troubleshooting.md).

References: [Magpie adapter](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-providers/src/cli/codex.rs), [official app-server documentation](https://developers.openai.com/codex/app-server/), [provider behaviour record](https://github.com/ChristianRelf/Magpie/blob/main/docs/PROVIDERS.md).
