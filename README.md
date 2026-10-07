# Magpie

One local harness for compatible AI models. Magpie is a desktop control centre for provider connections, model routing, execution telemetry and developer integrations. It has no chat interface and needs no hosted Magpie account.

The Rust service runs independently of the React/Tauri window. Connect an API key, a local model server, or an already authenticated official provider CLI. Create a scoped key in **Integrations**, then send work to the local API.

## Getting started

1. Install a package for your platform and open Magpie.
2. Connect a provider. API credentials go into your operating system's credential manager. CLI sign-in stays with the official CLI.
3. Choose a routing preset. Background operation and start on login are opt-in.
4. Create an integration key and copy the endpoint from Integrations.

The control centre includes provider and model management, execution inspection, routing priorities, scoped integration keys, persistent settings, Recharts telemetry graphs and a year-long token activity grid. Daily, weekly and cumulative heatmap modes use local execution records, with hover details and date selection. Missing provider quotas remain unavailable.

Linux `.deb` and AppImage builds have been produced locally. Windows NSIS and macOS DMG builds are configured in the **Build installers** workflow. Check [the verification record](docs/STATUS.md) before treating a platform or integration as verified. These initial builds have no configured signed update channel.

## Local API

Default endpoint: `http://127.0.0.1:7878`. All endpoints except the minimal `/health` probe require authentication.

```bash
curl http://127.0.0.1:7878/v1/responses \
  -H "Authorization: Bearer $MAGPIE_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"model":"auto","input":"Summarise this text: ...","stream":true}'
```

`/v1/responses` is Magpie's native protocol. `/v1/chat/completions` implements a subset of the OpenAI Chat Completions protocol for tools that support a custom base URL. See [API and SDK usage](docs/API.md), including tool calls and explicit agent permissions.

```bash
magpie status
magpie providers
magpie models
magpie usage --range 7d
magpie limits
magpie route "Summarise this text"
printf 'Review this repository' | magpie run - --model codex_cli/MODEL_ID --cwd /path/to/repository
```

Codex and Gemini CLIs require explicit agent options because they can read local files. Plain generation is available through API/local adapters and Claude Code's tool-disabled mode. Magpie does not turn consumer subscriptions into unrestricted third-party API access. [Provider support and limits](docs/PROVIDERS.md) describes the boundaries.

## Build and develop

Install current stable Rust, Node 24, pnpm 10.34.5 and [Tauri's native prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS. These are build requirements; ordinary users only need an installer and any official CLI they choose to use.

```bash
pnpm install --frozen-lockfile
pnpm dev                              # native development window + harness
pnpm build                            # platform installers
cargo build --release -p magpie        # optional standalone CLI
pnpm --filter @magpie/sdk build        # TypeScript SDK
```

The SDK currently ships in this workspace and as a locally packable package; it is not claimed to be published to npm. `pnpm --filter @magpie/sdk pack` creates its distributable.

```bash
pnpm typecheck
pnpm test
pnpm build:ui
cargo test --workspace --locked
cargo build -p magpie
node scripts/lifecycle-test.mjs
pnpm --filter @magpie/desktop exec playwright install chromium
pnpm test:e2e
pnpm audit --prod
cargo audit
```

Browser tests start an isolated Rust service and a clearly identified local HTTP fixture. They do not send requests to paid providers. `scripts/provider-smoke.mjs` is optional and requires explicit consent and an already connected named model; it is never run by CI.

See [architecture](docs/ARCHITECTURE.md), [security](SECURITY.md), [release instructions](docs/RELEASE.md), and [completed work / release gates](docs/STATUS.md).
