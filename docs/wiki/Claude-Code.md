# Claude Code

Magpie delegates to the installed official `claude` CLI. Both shared login and saved-token connections are available in v0.1.3. The saved-token workflow was added in 0.1.3 and is described in [Saved sign-ins](Saved-sign-ins.md).

## Connect the shared login

1. Install Claude Code from its official distribution.
2. Complete its official sign-in under the same OS user as Magpie. The documented login command used by the integration is `claude auth login`.
3. Confirm the CLI launches and reports its authentication state in your terminal.
4. Select Claude Code in **Providers**, or run `magpie connect claude_code` in an owner session.
5. Inspect the connection and discovered aliases/models. A catalogue alias is not a guarantee of account entitlement.

```bash
claude --version
claude auth status
magpie connect claude_code
magpie providers
magpie models --available
```

The login flow is interactive and may open a browser. A generation request through Magpie cannot complete an interactive login/approval conversation for you.

## Plain generation and agent mode

For ordinary generation, Magpie disables tools, uses strict MCP configuration, and disables supported non-managed hooks. Provider-managed policies remain authoritative. This is why Claude Code can serve an ordinary text request while Codex and Gemini CLIs require explicit agent context.

For repository work, supply `agent.working_dir` or `magpie run --cwd`. Use `allow_writes: true` / `--allow-writes` only if editing is intended. The process still runs as your user and follows official CLI policy; a working directory is not a complete sandbox.

CLI-owned history and settings can exist independently of Magpie's retention controls. Changing Magpie's local history setting does not erase Claude's own files.

## Enable allowance reporting

Open **Providers → Claude Code → Manage → Enable usage reporting**. This is an explicit opt-in to the official user `statusLine` setting. Magpie preserves an existing status-line command and forwards its stdin/output while extracting only supported allowance fields.

The bridge records reported five-hour and weekly percentages/reset timestamps, the linked Magpie account ID, and observation time in a private local snapshot. It does not read credentials or transcripts, call private usage endpoints, or create a prompt to measure quota.

After enabling it, use Claude Code normally. On supported plans and versions/settings, the official status line receives `rate_limits.five_hour` and `rate_limits.seven_day` after a normal response. Magpie imports new reports approximately every five seconds while running. Merely logging in or clicking Refresh does not supply those fields.

The helper can continue collecting reports while the Magpie window/service is closed. The harness imports the snapshot when running again. Only one shared Claude CLI profile/connection can receive this reporting at a time; `CLAUDE_CONFIG_DIR` affects which configuration is updated.

## Why percentages remain unknown

| Symptom                                 | Check                                                              | Repair / expected result                                               |
| --------------------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Reporting just enabled; no numbers      | Has a normal supported Claude response happened since?             | Wait for ordinary use; do not loop paid requests to force a number     |
| Hook enabled but no snapshot            | Project/managed status-line override, workspace trust, hook policy | Use permitted CLI settings; Magpie does not override those protections |
| Some windows absent                     | Whether Claude supplied each field                                 | Missing fields remain unavailable                                      |
| Reset passed but percentage is old      | Observation time and expired window                                | UI shows Awaiting update until a fresh observation                     |
| Portable app moved                      | Helper's registered executable location                            | Disable and re-enable reporting from the new location                  |
| Upgrade has no visible effect           | Background harness version                                         | Restart it from Settings                                               |
| Saved Claude token has no account quota | Connection authentication mode                                     | Expected: bridge belongs only to shared login                          |

An exhausted allowance can be recorded from request-stream limit events even when the request ultimately fails. A stale reset timestamp should not be presented as a fresh balance.

## Disable reporting safely

Use the same provider management panel to disable usage reporting. Magpie attempts to restore the previous status-line setting while preserving later user edits. It avoids blindly replacing a status line you have changed after enabling the bridge. If the app reports a conflict, inspect the relevant user settings with the official CLI's documentation; do not replace the entire settings file from a stale copy.

Before moving/removing a portable app, disable the bridge from the old installation. Re-enable it from the new stable path if desired.

## Authentication and execution failures

First determine whether the problem is Magpie client authentication (`401 invalid_api_key`) or upstream Claude authentication (`502 upstream_authentication`, connection `needs_auth`, or a provider failure). A new Magpie integration key does not renew Claude's login.

Verify the CLI under the correct user, inspect the connection detail, and check its executable version. If the OS credential manager is involved in a saved-token profile, unlock it and replace the token through the supported management flow. Do not read/copy unrelated CLI credential files to repair an account.

Source and provider reference: [Magpie's Claude adapter](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-providers/src/cli/claude_code.rs), [official status-line quota fields](https://code.claude.com/docs/en/statusline#rate-limit-usage). Live reporting depends on the actual plan, CLI version, and settings; fixture coverage is recorded in the [implementation record](https://github.com/ChristianRelf/Magpie/blob/main/docs/STATUS.md).
