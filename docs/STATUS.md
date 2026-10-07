# Magpie implementation record

Updated 2026-10-08. Implemented code and verified release behaviour are recorded separately.

## Completed functionality

- Independent Rust harness with authenticated loopback HTTP/SSE, SQLite migrations, deterministic task classification, capability-aware routing, cancellation, conservative retries/fallback, usage accounting and notifications.
- API adapters for OpenAI-compatible endpoints, Anthropic and Gemini; official Codex, Claude Code and Gemini CLI delegation. Consumer credentials are not extracted.
- CLI, stdio MCP bridge and built/packable TypeScript SDK with typed requests/events and abortable streaming.
- React control centre: onboarding, overview, providers, models, execution inspector, analytics, routing, scoped integrations and settings.
- Recharts time-series graphs and calendar token activity with UTC daily/weekly/cumulative views, hover details, date filtering and accessible data. Cards darkened and calendar cells enlarged with consistent 3px gutters as requested.
- Native Tauri shell, tray menu, detached daemon mode, start/stop/restart, login assistance, native exports, per-user login startup registration and optional signed updater.
- API keys use the OS credential manager in production and fail closed when unavailable. Development plaintext storage is debug-only.
- Explicit agent scope for local filesystem workflows; Codex/Gemini agent execution requires an explicit working directory. Integration keys cannot cancel another client's execution by ID.
- README, API/SDK examples, architecture, security, provider limitations, release guide, dependency update configuration and installer/check workflows.

## Verified in this workspace

- **102 Rust tests passed**, covering API authentication/scopes, adapter conversion, route selection, fallback, usage/costs, limits, secret storage, persistence/migrations and engine error cases; includes two native command tests.
- TypeScript typecheck and Vite production build passed. Four SDK streaming tests passed; SDK build and pack completed.
- Three calendar aggregation tests passed. One run timed out under concurrent native compilation; date formatting was changed to reuse formatters, and the rerun passed.
- Playwright passed an isolated end-to-end browser flow using the real Rust harness and an explicitly identified local HTTP fixture: onboarding, connect/discovery, execution and telemetry, every screen, calendar modes, favourites, execution details, routing persistence, key restrictions/revocation and settings after reload.
- Actual Linux Tauri/WebKit window launched under Xvfb with an isolated D-Bus session. UI-to-harness connection worked; default close stopped the service; the background opt-in preserved it after UI exit. Screenshot inspected. Initial headless runs stalled on the host's missing desktop portal; the isolated bus resolved the test environment issue.
- Linux `.deb` and AppImage were built. Final rebuild with the latest changes is being verified.
- JavaScript production dependency audit: no known vulnerabilities. Rust audit completed with two informational transitive GTK findings; see SECURITY.md. Do not describe the Rust audit as clean.
- Commits are pushed to `origin/main` as requested. No live provider calls, paid execution, signing/notarisation or public release have been claimed.

## Remaining release gates

- Finish current packaged-runtime lifecycle check and final Linux rebuild.
- Run Windows/macOS installer and OS credential-store checks on native runners. Workflows are configured; those platforms are not yet certified here.
- Configure installer signing/notarisation and a signed HTTPS updater channel; verify a real version-to-version update.
- Live provider authentication/execution/quota checks require separately supplied test credentials and explicit consent to spend allowance. Optional smoke test is provided and is never run by CI.
- Resolve or formally review the upstream `glib` iterator and `proc-macro-error` audit findings before claiming a clean public-release security review.
