# Magpie implementation record

Updated 2026-10-08. This record distinguishes implemented code from verified release behaviour.

## Implemented

- Independent Rust harness, authenticated loopback API, SQLite migrations, task classification, capability-aware routing, streaming, cancellation, conservative fallback, usage accounting and notifications.
- Provider adapters for API keys, compatible/local endpoints and delegation to official Codex, Claude Code and Gemini CLIs. Consumer credentials are not extracted. Live paid-provider execution is not yet verified in this workspace.
- CLI and TypeScript SDK. Native `/v1/responses` uses Magpie's schema; `/v1/chat/completions` implements a documented compatibility subset.
- React control centre: onboarding, overview, providers, models, execution inspection, analytics, routing, scoped integrations and settings.
- Recharts graphs and a calendar token-activity heatmap with daily, weekly and cumulative views, UTC buckets, hover details, date filtering and accessible data.
- Production secret writes use the OS credential manager and fail closed; plaintext storage is explicitly limited to debug builds for isolated development.

## Verification checkpoints

- Existing backend: 95 Rust tests passed before native-shell additions (API, engine, adapters, routing, accounting, store and security).
- Desktop TypeScript typecheck and Vite production build passed. SDK build imports successfully in Node.
- Four SDK streaming tests and three calendar aggregation tests passed.
- Playwright passed an end-to-end browser flow against an isolated local HTTP provider and the real Rust harness: onboarding, connect/discovery, generation, telemetry, every screen, favourites, execution details, routing persistence, scoped keys/revocation, settings after reload.
- Tauri native shell compiles on Linux. Linux release packaging is in progress.
- No live provider calls, signing, publication or updater release have been claimed.

## Remaining release work

- Complete native build, browser interaction tests, lifecycle tests and installable Linux package.
- Check Windows/macOS installers using native CI runners; configure release signing and updater public key/HTTPS channel.
- Real-provider smoke tests require separately supplied test credentials and explicit consent to spend allowance.
- Validate accessibility, provider contract limitations and release documentation.
