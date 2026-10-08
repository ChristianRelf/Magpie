# Development and testing

The workspace combines Rust crates, a React/Tauri desktop, a standalone Rust CLI, and an ESM TypeScript SDK. Use the current checkout's lockfiles and inspect local changes before changing branches or cleaning outputs.

## Toolchain

| Tool                 | Repository baseline                                                 |
| -------------------- | ------------------------------------------------------------------- |
| Rust                 | Stable in CI; manifests declare minimum 1.89                        |
| Node                 | 24 in CI; root engine constraint `>=22.18`                          |
| pnpm                 | `10.34.5` pinned in `package.json`                                  |
| Desktop dependencies | Tauri 2 native prerequisites for the target OS                      |
| Browser tests        | Playwright Chromium plus host dependencies                          |
| Docs tooling         | Python 3.10+ standard library; Bash for shell-example syntax checks |

Follow [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/) for Windows/macOS/Linux. The Ubuntu workflow installs WebKitGTK 4.1 development files, Ayatana AppIndicator, librsvg, patchelf, and OpenSSL development files. The native window test additionally uses Xvfb, xdotool, and xwd/X11 tools.

These packages are needed for desktop/native builds, not every end-user install. Rust workspace tests include the native shell crate, so a full workspace command can require desktop libraries even if your intended fix is in a backend crate.

## First build

```bash
pnpm install --frozen-lockfile
pnpm typecheck
pnpm --filter @magpie/sdk build
cargo build -p magpie --locked
pnpm dev
```

`pnpm dev` invokes Tauri development and opens the native application. `pnpm --filter @magpie/desktop dev` starts only Vite on port 1420. The latter needs a separately configured harness and cannot supply native lifecycle/tray/login/export functionality.

Use the normal package manager cache rather than committing dependency output. If a lockfile mismatch occurs, first confirm the pinned pnpm version and the branch's intended dependency change; avoid regenerating locks as a reflexive repair.

## Isolate a development service

A development harness can otherwise discover your normal owner data. Use a fresh root and separate port for diagnostic experiments. In Bash, from the repository:

```bash
MAGPIE_DEV_DIR=$(mktemp -d)
MAGPIE_HOME="$MAGPIE_DEV_DIR" MAGPIE_LOG=debug \
  ./target/debug/magpie serve --port 17880
```

This runs in the foreground; Ctrl-C stops it. It contains no copied providers/keys/history. Use a second shell with the same `MAGPIE_HOME` to administer that isolated service. Keep the directory private and remove it only after the service stops and you no longer need its evidence.

`MAGPIE_SECRET_STORE=file` is available only for debug builds and explicitly isolated fixture credentials. Production must use the OS store. Never point a test's plaintext backend at the normal application directory or feed it real provider credentials.

## Fast checks by layer

| Change                          | Focused check                                        |
| ------------------------------- | ---------------------------------------------------- |
| Core contracts / validation     | `cargo test -p magpie-core --locked`                 |
| Provider adapters               | `cargo test -p magpie-providers --locked`            |
| Routing/classification          | `cargo test -p magpie-router --locked`               |
| Execution/accounts/fallback     | `cargo test -p magpie-engine --locked`               |
| HTTP/scopes/conversion          | `cargo test -p magpie-api --locked`                  |
| Persistence/analytics           | `cargo test -p magpie-store --locked`                |
| SDK stream/contracts            | `pnpm --filter @magpie/sdk test` and `build`         |
| UI helpers                      | `pnpm --filter @magpie/desktop test` and `typecheck` |
| End-to-end desktop browser flow | Build CLI, install Chromium, run `pnpm test:e2e`     |
| Documentation                   | `python3 scripts/wiki.py check`                      |

Use a meaningful regression for behaviour changes. Simple prose edits do not require a newly invented application test. Once the relevant checks pass, expand only when the change affects broader boundaries or failures warrant it.

## Workspace checks

```bash
pnpm typecheck
pnpm test
pnpm build:ui
cargo fmt --all --check
cargo test --workspace --locked
cargo build -p magpie --locked
node scripts/lifecycle-test.mjs
pnpm --filter @magpie/desktop exec playwright install chromium
pnpm test:e2e
```

Browser fixtures start an isolated service and explicit local HTTP provider. Fixture ports include 17878/17879, and Vite uses 1420. These tests do not make paid provider requests. On supported CI hosts, Playwright's `install --with-deps chromium` also installs browser OS dependencies.

Version 0.1.3 includes additional CLI/profile fixtures. Compare test counts against the actual commit; do not reuse historical counts as proof that a newer branch was checked.

## Lifecycle and native window checks

The lifecycle script exercises real service start/stop/discovery, scoped access, persistence, and duplicate protection. It defaults to the debug CLI; set `MAGPIE_TEST_BINARY` for another build. To exercise the desktop daemon mode too, supply the built desktop path:

```bash
MAGPIE_TEST_BINARY=target/release/magpie \
MAGPIE_TEST_DESKTOP=target/release/magpie-desktop \
  node scripts/lifecycle-test.mjs
```

Linux native window verification uses the actual WebKit window and both close policies:

```bash
NO_AT_BRIDGE=1 GTK_USE_PORTAL=0 \
  dbus-run-session --config-file=scripts/headless-session.conf \
  -- python3 scripts/native-window-test.py
```

Build the needed binaries and install native test prerequisites first. The isolated D-Bus configuration avoids activating portals from an unrelated desktop session. Passing Chromium browser tests is not equivalent to passing the Tauri/WebKit window test.

The Claude usage helper has a separate native fixture script, `node scripts/claude-usage-test.mjs`; configure binary paths as its environment requires. It tests official-shape local inputs and restoration behaviour, not live plan entitlement.

## Real-provider smoke test

This is optional and explicitly spends a selected provider's allowance/money if applicable. It is never run by CI. Set a dedicated integration key, root URL, and an explicit connected API/local model:

```bash
export MAGPIE_SMOKE_MODEL='PROVIDER/MODEL_ID'
node scripts/provider-smoke.mjs --allow-real-provider
```

The script refuses absent/automatic model selection and missing consent, disables fallback, and asks for a short output. It is not a full agent-login or account-quota certification. Do not use production secrets in fixtures or run this command merely to validate documentation.

## Build outputs and audits

```bash
pnpm build
cargo build --release -p magpie --locked
pnpm --filter @magpie/sdk pack
pnpm audit --prod
cargo audit
```

Install `cargo-audit` separately with `cargo install cargo-audit --locked` if needed. Treat audit output as evidence to review; the historical Rust informational findings are documented in [SECURITY.md](https://github.com/ChristianRelf/Magpie/blob/main/SECURITY.md), not suppressed by this guide.

Native bundles appear beneath Cargo's target release bundle directory. A custom `CARGO_TARGET_DIR` changes that location. Platform packages need their platform build environment; compiling the UI alone does not produce installers.

## Common development failures

For missing GTK/WebKit packages, install the matching native prerequisites. For a busy target/cache lock, identify another build before deleting anything. For occupied fixture ports, stop only the known conflicting test process. For a disconnected browser view, check its harness URL/token and CORS origin; do not embed production keys in a Vite bundle.

Use [Fixing issues](Fixing-issues.md) for a regression workflow and [Release maintenance](Release-maintenance.md) when a change is ready to package.
