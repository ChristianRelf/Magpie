# Troubleshooting

Start by identifying which layer failed: application launch, local service, client authentication, routing, upstream provider, or presentation. Fix that layer and repeat the smallest useful check. Avoid deleting your database or repeatedly making paid requests as a first response.

## Quick triage

```bash
magpie --version
magpie status
magpie --no-start providers
magpie --no-start models --available
magpie --no-start limits
magpie --no-start activity --limit 10
curl --fail-with-body http://127.0.0.1:7878/health
```

Use the installed CLI path if `magpie` is not on `PATH`. Substitute the actual port. These checks do not generate model output. They can read retained metadata, so review output before sharing.

| Observation                                     | Investigate next                                |
| ----------------------------------------------- | ----------------------------------------------- |
| App never launches                              | Package, architecture, OS trust/dependencies    |
| `/health` unreachable                           | Process, port, bind address, data root, logs    |
| `/health` works; protected endpoint returns 401 | Magpie key                                      |
| Protected endpoint returns 403                  | Scope or Host restriction                       |
| Routing returns no eligible model               | Model/account/capability/policy filters         |
| Route works; execution fails                    | Upstream auth, quota, network, CLI policy       |
| Execution works; charts look wrong              | Source, date range, retention, report freshness |

Use the [API error table](API-reference.md#error-reference) to interpret the exact status/type. A provider authentication error is normally a 502 upstream error, not the local 401 key error.

## Service will not start or connection is refused

**Check:** open Settings' service controls or run `magpie status`. Check the actual data root and `logs/harness.log`. Confirm that an old process is not still holding the lock/port and that the client is calling the configured port.

**Fix:** start the service deliberately through the desktop or `magpie start` in an owner session. If using a scoped environment, start the service separately; the key-based CLI does not auto-launch it. If configuration changed, restart after finishing active work.

**Verify:** `/health` identifies Magpie, then authenticated `/v1/status` works. A browser loading a Vite development page is not proof that the Rust service is running.

Do not create a new data directory merely to hide a startup error. First read the existing directory's log and check permissions/disk availability. For an isolated diagnostic run, use the [development recipe](Development-and-testing.md#isolate-a-development-service).

## Port already in use

The message `Could not listen on ... Is another harness running?` means binding failed. On Linux, inspect listeners:

```bash
ss -ltnp 'sport = :7878'
```

On macOS:

```bash
lsof -nP -iTCP:7878 -sTCP:LISTEN
```

On Windows PowerShell:

```powershell
Get-NetTCPConnection -LocalPort 7878 -State Listen
```

Identify the process before stopping anything. If it is the intended Magpie service, connect to it. If another application needs that port, choose a different Magpie port, restart, and update clients. When no service can start on the old port, an owner foreground launch such as `magpie serve --port 8787` can save a different port in that data root. Do not run it alongside a service that already owns the same root.

Verify the new `/health` and authenticated endpoint. Changing only the client URL cannot move the server.

## A harness is already running

Magpie holds an OS file lock for its data root. Check `magpie status`, the process table, and the runtime metadata. The file `harness.lock` can remain after shutdown; its existence alone is not a stuck lock.

Stop the known service normally if a restart is intended. If it is hung, identify that exact process and preserve logs before using the OS's targeted termination controls. Do not delete the lock file or database while a process might still use it. Start one instance and verify discovery afterwards.

## 401 invalid API key

**Cause:** no valid Magpie client key was supplied. Common cases are an upstream provider key used by mistake, a revoked key, copied whitespace, or an environment variable missing from the actual client process.

**Fix:** create a replacement in Integrations, copy it once, and update the intended client's private configuration. Use `Authorization: Bearer ...`. Do not print the secret while debugging headers.

**Verify:** call authenticated `/v1/status` with a `read` key. `/health` does not validate credentials. A newly created key can authenticate even if every provider is offline; those are separate checks.

## 403 insufficient scope or Host not allowed

For JSON `insufficient_scope`, compare the endpoint with [Security and permissions](Security-and-permissions.md). Model listing/telemetry normally needs `read`, generation `execute`, agent work `execute` plus `agent`, and management `admin`. Recreate a suitably scoped key or use the owner session for intentional administration.

`Host not allowed` is a different boundary. Use the actual loopback URL such as `http://127.0.0.1:7878`, not an arbitrary hostname/proxy that changes Host. Browser CORS is also separate: a valid token does not authorise every web origin. Use the native application or an appropriate local/server-side integration.

Do not turn off authentication or widen network exposure to repair a malformed request.

## No eligible model

Run a route preview with the same prompt and preferences. Check each potential exclusion:

| Check                                            | Repair                                                                |
| ------------------------------------------------ | --------------------------------------------------------------------- |
| No connected/enabled account                     | Connect/verify or enable the intended account                         |
| Model disabled                                   | Re-enable it if it should receive work                                |
| Only Codex/Gemini CLI available                  | Supply explicit agent context and permissions                         |
| Tools/vision/JSON/reasoning unsupported          | Choose an adapter/model with the capability or remove the requirement |
| Estimated input plus output exceeds context      | Reduce content/output allowance or select a larger context model      |
| Known allowance exhausted                        | Wait for supported reset/update or choose another eligible account    |
| Billable/provider filter excludes all candidates | Reconcile the request policy with available providers                 |
| Premium reserve excludes simple work             | Choose another model or deliberately adjust the reserve               |
| Explicit ID is stale/wrong                       | Refresh inventory and use a discovered ID/key                         |

The CLI route command cannot express `--cwd`; use a native route preview for agent context. Do not set an arbitrary task label to bypass permission checks. After a change, preview again before inference.

## Official CLI is installed but not detected

Run its `--version` command under the same OS user. Compare terminal discovery (`command -v claude`, `command -v codex`, or `command -v gemini` in Bash) with the launch environment used by Magpie. Startup/login services can have a minimal `PATH`.

Magpie checks explicit overrides, `PATH`, and common install locations, and augments child PATH for nearby runtime binaries. A custom location, broken shim, or missing Node runtime can still fail. Correct the installation/path, restart the harness after environment changes, and verify discovery again. On Windows use `Get-Command` to inspect a command rather than assuming a Unix executable name.

Do not install a second unofficial CLI simply to satisfy detection; use the provider's supported distribution.

## Provider authentication fails

Distinguish local client authentication from upstream authentication. If `/v1/status` works but generation reports `upstream_authentication`, a new Magpie key will not repair the provider account.

For API connections, confirm the key belongs to the intended provider/project and replace revoked/expired credentials through management. For shared CLI connections, complete the official login under the correct user, then verify in Magpie. For saved profiles introduced in 0.1.3, use the selected connection's login/token replacement flow rather than changing the shared CLI identity.

Some verification checks only detect configured authentication; remote rejection can occur on the next request. Inspect provider-specific guidance: [Claude](Claude-Code.md), [Codex](Codex-CLI.md), [Gemini](Gemini-CLI.md).

## Credential manager unavailable

Production credential writes fail when Keychain/Credential Manager/Secret Service cannot be used. Unlock the relevant store and ensure Magpie runs in the expected user/desktop session. Linux headless sessions may have no usable Secret Service even if the desktop session does.

Retry connecting after fixing the store. Check that status reports the intended backend. Do not recommend `MAGPIE_SECRET_STORE=file` for production; it is a debug-only plaintext fixture option. Copying the database to another machine does not move the OS keyring entries.

## Local model server or custom endpoint fails

Check the upstream model endpoint directly and verify that its server is running with a loaded model. Verify base URL prefix and port. A path such as `/v1/v1/models`, a login-page HTML response, or an HTTP redirect often points to a wrong base URL.

Magpie's provider client intentionally rejects redirects. Configure the final API endpoint. If discovery succeeds but generation/options fail, reduce the request to supported features and inspect adapter/model capabilities. Do not equate “OpenAI-compatible” with support for every OpenAI API.

## Rate limited or quota exhausted

Read the error type, `Retry-After`, relevant window, reset time, and provenance. Wait for the provider's supported retry/reset interval or select another eligible account according to your billing policy. Avoid a tight retry loop; it can worsen rate limiting.

A qualitative limit warning is not an exact percentage. An expired observation can become Awaiting update. Duplicate keys/accounts may share provider quota. Adding a Magpie integration key does not increase it. Subscription-to-metered fallback requires deliberate consent.

## Stream hangs or stops mid-response

1. Confirm the client is using the matching native or Chat Completions parser.
2. Disable terminal/client buffering where appropriate (`curl --no-buffer`).
3. Check `started`, `routing_changed`, `completed`, and `failed`, not just text.
4. Inspect active executions and provider startup/timeout errors.
5. Compare client deadlines with per-attempt timeout, queueing, and retries.

An HTTP 200 stream can still fail. A stream ending without a terminal event is incomplete. Reconcile Activity before retrying an agent/tool task, because earlier side effects may remain. Browser `EventSource` is not a direct substitute for an authenticated POST stream.

## Fallback did not happen or chose another model

Check saved and request-level fallback settings, explicit model key, candidate list, billing consent, error kind, and whether output/tool activity already occurred. Write-capable agent requests are not blindly replayed. A model preference with fallback enabled can legitimately produce a different final selection.

Use `allow_fallback: false` and an account-specific model key for strict selection. Do not automatically retry from the client to defeat a safety stop without inspecting the original attempt. See [Routing and fallback](Routing-and-fallback.md).

## Charts are empty, stale, or different from another client

Choose the source first. Local Magpie history does not include requests made directly in an unrelated CLI. Codex account history can include external activity, but is a provider snapshot with its own publication delay and support limits.

Check UTC dates, range, selected account, local-history setting, retention, observation time, and polling. Increasing retention cannot recover deleted data. Repeatedly refreshing cannot force an unsupported provider to return account history. See [Usage and limits](Usage-and-limits.md).

For Claude percentages, check the shared-login reporting opt-in, next normal response, supported plan/CLI fields, project/managed status-line overrides, and moved executable paths. Saved Claude tokens do not have this shared quota bridge.

## Settings disappear or app and CLI disagree

Save the UI draft and confirm both processes use the same `MAGPIE_HOME`, OS user, and service. Owner discovery and scoped `MAGPIE_URL` can target different instances. Check the running harness version after upgrades.

When using the API, send the full current settings/routing object on PUT. A partial object can reset omitted fields to defaults. Restart for port/concurrency/logging changes. See [Settings and storage](Settings-and-storage.md).

## Background or start-on-login behaviour is wrong

Separate the three controls: tray hiding, keep running after quit, and launch at login. Save the intended choice. Check tray availability and the registered executable path. If a portable app moved, disable/re-enable login startup from its new location.

A CLI available in an interactive terminal may be missing in the login environment. Confirm the actual service state after a real login, not merely the presence of a registry value or startup file. An external client with a supplied key does not start a stopped service for you.

## Update channel unavailable

The installed build needs a compiled updater public key and HTTPS manifest endpoint. The documented v0.1.3 release uses manual updates, so this message is expected. Install the appropriate release package; toggling automatic update checks cannot add missing build-time configuration. See [Installation and updates](Installation-and-updates.md).

## Export appears incomplete

The current history export goes through the store's 5,000-record cap and does not apply every aggregate filter. Narrow date ranges and verify record counts. A metadata export is not a full backup, and it does not contain retained prompt/output contents. Use [backup guidance](Settings-and-storage.md#backup-and-restore) for complete local state.

## Build or browser test fails

Use Node 24/pinned pnpm and stable Rust, install native Tauri dependencies, and build the CLI before lifecycle/browser tests. Install Playwright Chromium. Check development port 1420 and fixture ports 17878/17879 for conflicts. A headless WebKit launch also needs its native/display/session dependencies; a normal browser test does not certify those.

Use [Development and testing](Development-and-testing.md) to choose the right test rather than rerunning unrelated suites. Preserve the first useful error, command, versions, and relevant redacted log lines.

## Escalate with evidence

If the smallest check still fails after the relevant fix, collect versions, OS/architecture, connection type, actual endpoint/data root, error type, execution ID, and reproduction steps. Do not attach secrets, `admin.token`, upstream credential files, or an unreviewed database. Follow [Fixing issues](Fixing-issues.md#write-a-useful-issue).
