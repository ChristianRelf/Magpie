# Settings, storage, and backups

Most preferences live in SQLite and are edited in **Settings** or **Routing**. Environment variables affect process startup/discovery and development workflows. There is no general user-editable `magpie.toml` configuration file in this implementation.

## Save and restart semantics

Save draft changes explicitly in the UI. API clients should read settings, modify the intended values, and PUT the full object. Omitted fields can revert to defaults; PUT is not a partial patch.

Restart the harness after changing bind address, port, concurrency, or diagnostic logging. The UI groups API settings under a restart notice. The timeout is read for subsequent executions, but a restart is a clear way to apply a batch of API changes after current work finishes.

Two privacy settings have immediate data effects: reducing retention can trigger cleanup, and turning retained request content off purges previously stored request/response content. Export or back up what you need before making those changes.

## Important defaults

| Section / key                     | Default     | Meaning                                                      |
| --------------------------------- | ----------- | ------------------------------------------------------------ |
| `general.launch_on_startup`       | `false`     | Per-user harness login registration                          |
| `general.minimise_to_tray`        | `false`     | Hide window on close when tray is available                  |
| `general.keep_harness_running`    | `false`     | Service survives desktop quit                                |
| `general.automatic_updates`       | `true`      | Check on launch only when a channel exists                   |
| `general.theme`                   | `dark`      | `dark`, `light`, or `system`                                 |
| `general.reduced_motion`          | `false`     | Explicit reduced-motion override; OS preference also matters |
| `analytics.local_history`         | `true`      | Persist execution history                                    |
| `analytics.retention_days`        | `90`        | `0` keeps records indefinitely; accepted maximum 3650        |
| `analytics.refresh_interval_secs` | `5`         | Live UI refresh; API accepts 1–3600                          |
| `analytics.poll_provider_limits`  | `true`      | Supported remote metadata polling                            |
| `analytics.spend_alert_usd`       | Unset       | Optional local spend notification threshold                  |
| `security.retain_request_content` | `false`     | Store future prompt/output contents                          |
| `security.diagnostic_logging`     | `false`     | Verbose local diagnostics after restart                      |
| `security.require_scopes`         | `true`      | Require explicit read scope for telemetry                    |
| `server.bind`                     | `127.0.0.1` | Local listener address                                       |
| `server.port`                     | `7878`      | Local HTTP port                                              |
| `server.allow_network`            | `false`     | Advanced non-loopback binding permission                     |
| `server.request_timeout_secs`     | `600`       | Per-attempt setting; API accepts 1–3600                      |
| `server.max_concurrency`          | `16`        | Concurrent execution permits; accepted range 1–128           |

The execution adapter timeout currently has a minimum of 10 seconds even if a lower accepted setting is supplied. Per-attempt timeout is not a guaranteed end-to-end deadline across queueing, retries, and fallback. Set an appropriate client deadline too.

Notifications are enabled by default. Provider disconnection, authentication expiry, exhaustion, approaching limit, stopped harness, and spend threshold notifications default on; allowance reset and fallback notifications default off. Notification delivery depends on the native environment and app state.

## Locate your data

| OS      | Default root                                               |
| ------- | ---------------------------------------------------------- |
| Linux   | `$XDG_DATA_HOME/magpie`, otherwise `~/.local/share/magpie` |
| macOS   | `~/Library/Application Support/magpie`                     |
| Windows | `%APPDATA%\magpie`                                         |

`MAGPIE_HOME` overrides the root. Use **Settings → Application data** or `magpie status` to find the actual directory. Prefer an absolute override and use the same value for the intended desktop/CLI/service session. Different roots create apparently different installations with different settings, tokens, and history.

## Files and their roles

| File/directory                   | Purpose                                                     | Handling                                        |
| -------------------------------- | ----------------------------------------------------------- | ----------------------------------------------- |
| `magpie.db`                      | SQLite configuration and execution history                  | Back up consistently; avoid manual edits        |
| `magpie.db-wal`, `magpie.db-shm` | SQLite working files while active                           | Do not omit them from an unsafe live-file copy  |
| `admin.token`                    | Owner bootstrap credential                                  | Keep private; never attach to an issue          |
| `harness.json`                   | Runtime discovery metadata                                  | Not a user configuration file                   |
| `harness.lock`                   | Process lock target                                         | Do not delete while a service may own it        |
| `logs/harness.log`               | Detached harness stdout/stderr                              | Review/redact before sharing                    |
| `scratch`                        | Adapter temporary execution context                         | Not a promise of filesystem confinement         |
| `secrets.json`                   | Debug-only file-store credentials when deliberately enabled | Plaintext test material; not production storage |

Optional Claude reporting and managed CLI profiles create additional private state. Their presence does not change where production API secrets belong: the OS credential manager.

The detached log is appended by the launcher. Do not assume built-in rotation will keep it small forever. If archiving a large log, stop the service first and retain only what you need after reviewing its contents.

## Environment variables

| Variable                   | Consumer                            | Effect                                                   |
| -------------------------- | ----------------------------------- | -------------------------------------------------------- |
| `MAGPIE_HOME`              | Runtime/owner clients               | Override data root                                       |
| `MAGPIE_API_KEY`           | CLI/MCP/examples                    | Use a supplied scoped key instead of owner API access    |
| `MAGPIE_URL`               | CLI/MCP/examples                    | Root endpoint in scoped mode; default loopback port 7878 |
| `MAGPIE_LOG`               | Runtime                             | Tracing filter at service startup                        |
| `MAGPIE_SECRET_STORE=file` | Debug runtime only                  | Isolated plaintext testing backend                       |
| `CLAUDE_CONFIG_DIR`        | Shared Claude reporting/CLI context | Alternative official Claude user configuration           |
| `MAGPIE_UPDATE_PUBLIC_KEY` | Build                               | Signed updater public key                                |
| `MAGPIE_UPDATE_URL`        | Build                               | HTTPS updater manifest endpoint                          |
| `MAGPIE_TEST_BINARY`       | Test scripts                        | Override test CLI path                                   |
| `MAGPIE_TEST_DESKTOP`      | Native tests                        | Override desktop binary path                             |
| `MAGPIE_SMOKE_MODEL`       | Optional provider smoke script      | Explicit connected API/local model                       |

Frontend test variables such as `VITE_MAGPIE_URL` and `VITE_MAGPIE_TOKEN` are development plumbing. A `VITE_` value can enter a browser bundle; do not build a public UI with a production owner token in it.

`MAGPIE_URL` alone does not override owner discovery. Scoped CLI mode requires `MAGPIE_API_KEY`. The local `status` command still reports the data-root service; use HTTP `/v1/status` for a specific integration address.

## Background and login behaviour

| Setting                         | Effect                                        |
| ------------------------------- | --------------------------------------------- |
| Minimise to tray                | Close hides the window when the tray exists   |
| Keep harness running after quit | External clients continue after the app exits |
| Launch harness at login         | Starts the service without opening the window |

Login registration is per-user: Linux uses an autostart `.desktop` entry, macOS a LaunchAgent, and Windows the current-user Run registry key. Relevant paths are `~/.config/autostart/magpie-harness.desktop` (honouring `XDG_CONFIG_HOME`), `~/Library/LaunchAgents/dev.magpie.harness.plist`, and the `MagpieHarness` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

Use Settings to manage registration. After moving a portable executable, disable/re-enable it from the new stable location. A successful registry/file write does not prove that the next login's environment can find provider CLIs or access a keyring.

## Backup and restore

For a consistent local backup:

1. Finish/cancel active requests, quit the desktop, and stop a background harness.
2. Confirm no process still uses the intended data root. Prevent login/desktop automation from restarting it during the copy.
3. Copy the entire root into a private backup location, preserving permissions. Record the Magpie version and any relevant CLI versions.
4. Store the backup as sensitive data: it can contain the owner token, retained contents, labels, and development secrets.
5. Restart and verify status after the copy.

A stopped-service filesystem copy is easier to reason about than copying only `magpie.db` during WAL writes. A history CSV/JSON export is not a complete backup and can be capped; see [Usage and limits](Usage-and-limits.md).

Restoring requires a stopped service and a version-compatible database. Preserve the current root separately before replacement. Reconnect credentials if the OS keyring entries or official CLI login are absent; copying SQLite does not migrate those secrets. Do not share one root between concurrently running instances.

For diagnosis, prefer a fresh temporary root on another port rather than overwriting/restoring your live database. Never delete data solely because a connection, key, or model failed.

Sources: [settings defaults](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-core/src/settings.rs), [data paths](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-store/src/paths.rs), [runtime](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-runtime/src/lib.rs).
