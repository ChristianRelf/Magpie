# How to use Magpie

Choose the workflow that matches what you want to accomplish. If this is your first installation, complete [Getting started](Getting-started.md) first.

## Use a local model through one endpoint

1. Start your local model server and make a model available there.
2. Connect it in **Providers** using `ollama`, `lm_studio`, or a custom compatible connection.
3. Confirm the model in **Models**.
4. Create a `read`/`execute` integration key.
5. Point your client at Magpie's Chat Completions base URL, `http://127.0.0.1:7878/v1`.
6. Select the discovered model or use `auto` with local-only request preferences.

To restrict a native request to local providers, use `preferences.providers: ["ollama", "lm_studio"]`. `allow_billable: false` alone is not a statement that all data stays on your computer: a subscription CLI may also be non-metered and still contact a cloud provider.

## Let Magpie choose between accounts

Connect the accounts, set routing preferences, and send `model: "auto"`. Use **Routing** to choose quality, speed, economy, or allowance preservation. Preview representative prompts before executing them:

```bash
magpie route 'Summarise this note' --task summarisation
magpie route 'Find the bug in this function' --task debugging --preset best_quality
```

Check reasons and rejected candidates. Route selection uses local heuristics and metadata; it is not a promise that the selected model will produce the best possible answer. Test suitability with your own permitted workloads.

## Keep one request on one connection

Discover the account-specific model key, then set both `model` and `preferences.allow_fallback: false` in the native API. A bare provider/model ID can match multiple connections. A model preference alone does not prohibit fallback.

Use this for reproducible evaluations, an account-specific workflow, or a request that must not move after an error. Inspect the final execution's selected account, not just the initially requested string.

## Give an editor or script access

Create one key per tool with `execute` and any required `read` scope. Configure its base URL and key. Start with a small text request before enabling optional client features. Chat Completions compatibility does not include every OpenAI endpoint or hosted tool.

For a TypeScript service, use [SDK and MCP](SDK-and-MCP.md). For command-line automation, use [CLI reference](CLI-reference.md). For an MCP client, launch `magpie mcp` with a scoped environment. Never distribute the owner `admin.token` as an application key.

## Run a repository task

Use an official agent-capable CLI connection and explicitly supply a working directory. Grant `execute` plus `agent`; grant writes separately only when intended.

```bash
magpie run 'Review the tests and report missing cases' \
  --model codex_cli/MODEL_ID --cwd /absolute/path/to/repo
```

After a write-capable task, inspect the repository diff and run its tests. A cancelled or failed request does not undo work the provider CLI already performed. See [Streaming and tools](Streaming-and-tools.md) for cancellation boundaries.

## Understand where your allowance went

In **Analytics**, choose the correct source before interpreting totals. **Magpie requests** shows work routed through this service; Codex account usage can include external sessions. Open **Providers** for reported allowance windows and reset times. CLI token counts, plan allowances, and API spend are different measurements.

For Claude's shared login, opt into the status-line bridge and allow a normal Claude response to report its quotas. Do not repeatedly send test prompts just to make a percentage appear. [Usage and limits](Usage-and-limits.md) explains unknown values and stale snapshots.

## Work without the window

Save **Keep harness running after quit**, close the desktop, then verify the service using `/health` and an authenticated status request. Use **Launch harness at login** if you want the service at your next login. These are separate settings from **Minimise to tray**.

The standalone CLI can also use `magpie start`, `serve`, `stop`, and `restart`. A client configured with a scoped key does not start the service automatically. See [Settings and storage](Settings-and-storage.md).

## Retire a tool or account

Revoke the tool's integration key in **Integrations**. Disable an account to exclude it temporarily while retaining its configuration. Disconnect it when you intend to remove that connection and its stored credential. Avoid disconnecting an account during active work; first inspect/cancel executions. Revoking a Magpie client key is different from revoking an upstream provider credential.

For saved profile management introduced in 0.1.3, read [Saved sign-ins](Saved-sign-ins.md). For unexpected behaviour at any step, start with [Troubleshooting](Troubleshooting.md).
