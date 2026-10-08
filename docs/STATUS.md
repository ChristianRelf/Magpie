# Magpie implementation record

Updated 2026-10-08. Implemented code and verified release behaviour are recorded separately.

## Claude Code allowance fix — 0.1.1

- Added opt-in usage reporting in the Claude provider inspector using the official status-line interface. The private snapshot retains only reported 5-hour/weekly percentages, reset timestamps, linked account ID and observation time. No credential extraction, transcript reading or allowance-consuming refresh is used.
- Existing status-line commands/output and unrelated settings are preserved. Disabling restores the prior status line without overwriting subsequent user edits. Hook restrictions, symlinked settings, Windows shells, portable AppImages and an older running harness are handled explicitly.
- Rate-limit stream events now persist immediately, even if a request later fails. Expired windows show “Awaiting update” and observation times remain visible.
- All 113 Rust tests and frontend/SDK/browser checks passed in [code checks](https://github.com/ChristianRelf/Magpie/actions/runs/37710817567). The [installer matrix](https://github.com/ChristianRelf/Magpie/actions/runs/37710877062) passed on Linux, Windows and both macOS architectures, including the actual native status-line helper fixtures and process lifecycle checks. The downloaded CI AppImage also passed the helper fixtures locally. Live account reporting has not been certified; Claude supplies status-line quotas only after a normal response on supported plans, subject to CLI trust/settings.
- [v0.1.1](https://github.com/ChristianRelf/Magpie/releases/tag/v0.1.1) is published from `9d6b2da3db028bb43b43aceca95ee2eefc712f3d`. All five installers plus checksums/manifest were hash-verified after upload. Builds remain unsigned; published `v0.1` artifacts are unchanged.

## Codex account activity fix — 0.1.2

- Found the cause of empty charts during external Codex sessions: the adapter already fetched official `account/usage/read` reports, but every graph queried only Magpie executions.
- Overview and Analytics now default to connected Codex account usage, with a persistent source selector for Magpie requests. Account totals are kept separate to prevent double counting. Recharts heatmap/time-series views use real daily reports, with unknown dates, UTC boundaries, snapshot deduplication, manual refresh and account CSV export.
- Account monitoring runs every two minutes, including when the harness is idle, with existing provider-polling opt-out and failure backoff. Account usage is fetched independently of quota endpoint success.
- A read-only check against installed Codex 0.156.1 returned daily usage buckets. No live model request was made and no authentication material was extracted. This verifies this machine's supported account-history endpoint, not arbitrary accounts or real-time reporting guarantees.
- TypeScript, all 113 Rust tests and all seven UI/four SDK tests passed locally. The expanded browser end-to-end test passed source selection, real chart rendering from explicit fixtures, refresh, source persistence and separation from harness totals; its screenshot was inspected. The [installer matrix](https://github.com/ChristianRelf/Magpie/actions/runs/37713068413) passed all four platforms, including native lifecycle/Claude helper checks and the Linux WebKit window test. The downloaded Linux AppImage also passed the native helper fixtures. The [final code checks](https://github.com/ChristianRelf/Magpie/actions/runs/37713041509) passed on the same source commit: 113 Rust tests, frontend/SDK checks, lifecycle, expanded browser flow and dependency audits. The first runner downloaded Ubuntu packages very slowly and was restarted; the second attempt passed. The two previously documented Rust informational advisories remain. Account-wide request counts, model breakdowns, latency and costs are not supplied by this endpoint and are not fabricated.

## Completed functionality

- Independent Rust harness with authenticated loopback HTTP/SSE, SQLite migrations, deterministic task classification, capability-aware routing, cancellation, conservative retries/fallback, usage accounting and notifications.
- API adapters for OpenAI-compatible endpoints, Anthropic and Gemini; official Codex, Claude Code and Gemini CLI delegation. Consumer credentials are not extracted.
- CLI, stdio MCP bridge and built/packable TypeScript SDK with typed requests/events and abortable streaming.
- React control centre: onboarding, overview, providers, models, execution inspector, analytics, routing, scoped integrations and settings.
- Recharts time-series graphs and calendar token activity with UTC daily/weekly/cumulative views, hover details, date filtering and accessible data. Cards darkened and calendar cells enlarged with consistent 3px gutters as requested.
- Native Tauri shell, tray menu, detached daemon mode, start/stop/restart, login assistance, native exports, per-user login startup registration and optional signed updater.
- API keys use the OS credential manager in production and fail closed when unavailable. Development plaintext storage is debug-only.
- Qualitative provider limit warnings no longer invent a percentage. Reservations use only reported, unexpired allowances.
- Provider HTTP clients reject redirects so custom authentication headers cannot be forwarded to a different endpoint.
- Explicit agent scope for local filesystem workflows; Codex/Gemini agent execution requires an explicit working directory. Integration keys cannot cancel another client's execution by ID.
- README, API/SDK examples, architecture, security, provider limitations, release guide, dependency update configuration and installer/check workflows.

## Original v0.1 verification

- **104 Rust tests passed on final Linux CI**, covering API authentication/scopes, adapter conversion, route selection, fallback, usage/costs, limits, secret storage, persistence/migrations and engine error cases; includes two native command tests. The local full workspace run passed 103 tests before the final redirect regression was added.
- The provider suite also passed its new redirect regression (26 provider tests): custom authentication headers cannot follow HTTP redirects to another endpoint.
- TypeScript typecheck and Vite production build passed. Four SDK streaming tests passed; SDK build and pack completed.
- Three calendar aggregation tests passed. One run timed out under concurrent native compilation; date formatting was changed to reuse formatters, and the rerun passed.
- Playwright passed an isolated end-to-end browser flow using the real Rust harness and an explicitly identified local HTTP fixture: onboarding, connect/discovery, execution and telemetry, every screen, calendar modes, favourites, execution details, routing persistence, key restrictions/revocation and settings after reload.
- Actual Linux Tauri/WebKit window launched under Xvfb with an isolated D-Bus session. UI-to-harness connection worked; default close stopped the service; the background opt-in preserved it after UI exit. Screenshot inspected. Initial headless runs stalled on the host's missing desktop portal; the isolated bus resolved the test environment issue.
- Final Linux `.deb`, AppImage and standalone CLI builds completed from `493c3db`, including the quota-provenance and redirect corrections.
- The final AppImage passed authenticated lifecycle checks with the final release CLI: scoped access, durable settings across restart, duplicate-process protection and graceful shutdown without a display. Delivery packages and SHA-256 checksums are available in `/srv/codex/cache/magpie/delivery` on the build machine.
- JavaScript production dependency audit: no known vulnerabilities. Rust audit completed with two informational transitive GTK findings; see SECURITY.md. Do not describe the Rust audit as clean.
- Commits are pushed to `origin/main` as requested. No paid model execution or production signing/notarisation have been claimed; the read-only account metadata checks are recorded above.

## Published releases

- [Magpie v0.1.2](https://github.com/ChristianRelf/Magpie/releases/tag/v0.1.2) is the latest release, published on 2026-10-08 from `5593a88c7afdda3c8c84be7d81bb6cf099412989`. Five Windows/macOS/Linux installers plus `SHA256SUMS` and `release-manifest.json` are attached; all seven uploaded digests matched their tested local artifacts before publication. It includes the Codex account-chart fix and the Claude reporting fix from v0.1.1. Builds remain unsigned/unnotarised with manual updates.
- [Magpie v0.1.1](https://github.com/ChristianRelf/Magpie/releases/tag/v0.1.1) contains the Claude allowance fix and remains available. Its verified source and builds are recorded above.

- [Magpie v0.1](https://github.com/ChristianRelf/Magpie/releases/tag/v0.1) was published on 2026-10-08 at the user's request. Its application version is `0.1.0`; the release tag points to the tested source commit `493c3dbf79b85721f87ebe1414d0a28c76f99ea6`.
- Five unchanged CI installers are attached: Linux x64 `.deb`/AppImage, Windows x64 NSIS, and Apple Silicon/Intel macOS DMGs. `SHA256SUMS` and `release-manifest.json` record hashes and build provenance. All seven uploaded asset digests were verified against local files before publication.
- The release notes explicitly disclose unsigned/unnotarised builds, unavailable automatic updates, outstanding manual/provider verification and both upstream Rust audit findings. Publication does not resolve the release gates below.

## Cross-platform verification

- [Initial installer matrix](https://github.com/ChristianRelf/Magpie/actions/runs/37704353609): Linux x64, Windows x64, Apple Silicon and Intel macOS passed native installer builds, Rust tests and authenticated lifecycle checks. Linux also passed the actual WebKit window/background opt-in test.
- [Final installer matrix](https://github.com/ChristianRelf/Magpie/actions/runs/37706726510) passed on all four targets from `493c3db` with the final security and quota corrections. Each platform passed Rust tests and the authenticated CLI/native-daemon lifecycle test. Downloadable artifacts include the platform installer and standalone CLI; CI retains them for 14 days.
- [Final code checks](https://github.com/ChristianRelf/Magpie/actions/runs/37706389429) passed on the same commit: frontend tests/build, Rust formatting and 104 tests, lifecycle, browser end-to-end flow and dependency audits. The Rust audit reports the two documented informational findings, without suppression. Subsequent documentation-only changes do not alter those binaries.

## Remaining release gates

- Actual installation/OS credential-store checks and real CLI login remain release gates; automated platform tests do not certify these user-environment flows.
- Configure installer signing/notarisation and a signed HTTPS updater channel; verify a real version-to-version update.
- Live provider authentication/execution/quota checks require separately supplied test credentials and explicit consent to spend allowance. Optional smoke test is provided and is never run by CI.
- Resolve or formally review the upstream `glib` iterator and `proc-macro-error` audit findings before claiming a clean public-release security review.
