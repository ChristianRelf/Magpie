# CLI reference

The standalone `magpie` executable controls the same local harness as the desktop. It supports shell execution, diagnostics, provider/key management, and a stdio MCP bridge. It is a separate binary from `magpie-desktop`.

## Install the CLI

The desktop installer does not guarantee a `magpie` command on your shell's `PATH`. Build it from a checkout using stable Rust:

```bash
cargo build --release -p magpie --locked
./target/release/magpie --help
```

On Windows the binary is `target/release/magpie.exe`. Run it by absolute path or install the CLI package from the workspace into Cargo's binary directory:

```bash
cargo install --path apps/cli --locked
magpie --version
```

Ensure Cargo's binary directory is on `PATH`. For a matching published version, build from the corresponding tag in a separate checkout rather than resetting a working tree with edits. CI installer artifacts can include the CLI, but do not assume the public release attaches it as an independent asset.

## Owner versus scoped client mode

Without `MAGPIE_API_KEY`, API commands discover the local data directory and use the owner token. They may start a service if none is running. With `MAGPIE_API_KEY`, they use that token and `MAGPIE_URL` and do not auto-start a harness. A set-but-empty key is an error rather than a fallback to owner access.

`MAGPIE_URL` defaults to `http://127.0.0.1:7878` and, for the CLI's scoped mode, must be a loopback HTTP URL. It is the root URL, without `/v1`. Setting `MAGPIE_URL` alone does not switch owner discovery to a different server.

The `status` command is a special local-discovery operation, even when a key environment variable is set. Use authenticated `GET /v1/status` to verify the principal at a particular client endpoint. `serve` directly starts the local runtime and is also a local lifecycle operation.

## Global options

| Option       | Effect                                                                                      |
| ------------ | ------------------------------------------------------------------------------------------- |
| `--json`     | JSON output for commands that implement structured results; streamed runs output event JSON |
| `--no-start` | Prevent normal owner API commands from starting a stopped service                           |
| `--help`     | Show command/option help                                                                    |
| `--version`  | Show this CLI binary's version                                                              |

Examples use long options for clarity. Help is the authority for the exact binary you installed.

## Service lifecycle

| Command                    | Behaviour                                                                               |
| -------------------------- | --------------------------------------------------------------------------------------- |
| `magpie serve`             | Run the harness in the foreground                                                       |
| `magpie serve --port 8787` | Run locally using that port; the runtime saves the override in its settings             |
| `magpie start`             | Discover/start a detached harness in owner mode                                         |
| `magpie stop`              | Stop the discovered owner service; with a supplied key, request administrative shutdown |
| `magpie restart`           | Stop/start the owner service; refuses scoped-key mode                                   |
| `magpie status`            | Show local runtime state and data location without starting it                          |

Stop/restart cancels active work. `--no-start` is useful for diagnostics but does not turn an explicit `start` into a passive query. The lock prevents concurrent services using the same data directory; use a separate `MAGPIE_HOME` and port for testing.

## Inventory and telemetry

```bash
magpie providers
magpie models
magpie models --available
magpie limits
magpie usage --range 7d
magpie activity --limit 50
magpie --json providers
```

`usage` reports work through the harness, not Codex account-wide history. The documented CLI ranges are `1h`, `24h`, `7d`, and `30d` (`24h` default). Activity defaults to 20 records. For pagination/custom date ranges/account reports, use the [API](API-reference.md).

## Provider management

```bash
magpie connect ollama --label 'Local Ollama'
magpie connect open_ai_compatible --label 'Custom server' --base-url http://127.0.0.1:8080/v1
magpie connect claude_code
magpie disconnect ACCOUNT_ID
```

Connect options are `--label`, `--base-url`, and `--key-stdin`. The latter reads a single line from stdin and trims it. Supply secrets from a private prompt or your credential tool, not as command-line arguments. Management requires owner/admin access. `disconnect` removes that connection and its stored credential; `ACCOUNT_ID` is the connection ID from providers, not its display name.

The basic CLI flags do not expose the separate-sign-in UI introduced in 0.1.3. Use the compatible desktop/SDK flow for [Saved sign-ins](Saved-sign-ins.md).

## Preview routing

```bash
magpie route 'Summarise these notes' --preset economical --task summarisation
magpie --json route 'Explain this failure' --model PROVIDER/MODEL_ID
```

`route` accepts `--preset`, `--task`, and `--model`, requires `read`, and performs no inference. It has no agent `--cwd` flag. To preview an agent request, send the native body to `/v1/route`.

## Execute

```bash
magpie run 'Explain the difference between a queue and a stack' --verbose
magpie run 'Summarise this text' --preset economical --task summarisation
magpie run 'Review this repository' --model codex_cli/MODEL_ID --cwd /absolute/path/to/repo
```

| Run option        | Meaning                          |
| ----------------- | -------------------------------- |
| `--model`, `-m`   | Model reference; default `auto`  |
| `--preset`, `-p`  | Routing preset override          |
| `--task`, `-t`    | Task hint                        |
| `--system`, `-s`  | System instructions              |
| `--verbose`, `-v` | Routing detail on stderr         |
| `--cwd`           | Explicit agent working directory |
| `--allow-writes`  | Permit edits; requires `--cwd`   |

Runs stream output. They can consume allowance/money. CLI run does not have a dedicated per-request `--no-fallback` flag; use the native API or saved policy when strict selection is necessary. An agent-scoped key also needs `execute`.

### Stdin rules

Use `-` to make stdin the prompt. With no prompt and non-terminal stdin, the CLI also reads stdin. If you supply prompt words and pipe content, it appends the content after those words with a blank line.

```bash
printf '%s\n' 'Text to summarise' | magpie run - --task summarisation
cat notes.txt | magpie run 'Summarise these notes:'
```

Quote shell metacharacters in prompt arguments. Streaming provider output is not shell code; do not pipe generated text into a command interpreter as an automatic next step.

## Integration keys

```bash
magpie keys list
magpie keys create editor --scope read --scope execute
magpie keys create trusted-agent --scope read --scope execute --scope agent
magpie keys revoke KEY_ID
```

Creation/revocation/listing require admin. Default creation scopes are `execute` and `read`. Creation prints the token once; do not log that output in a shared CI job. Revoke using the key's ID rather than the secret token. A revoked token should fail new protected requests.

## Environment hints and MCP

`magpie env` prints `OPENAI_BASE_URL`, `OPENAI_API_BASE`, and `MAGPIE_URL` exports. It does **not** print or create a key. Set `OPENAI_API_KEY`/`MAGPIE_API_KEY` separately as required by your client.

`magpie mcp` runs the stdio bridge. Pass a dedicated key in the MCP process environment; otherwise it can inherit owner mode. Protocol output goes to stdout, so do not add banner text to a wrapper. See [SDK and MCP](SDK-and-MCP.md).

## Diagnose a CLI failure

Run `magpie --version`, `magpie status`, and a read-only command with `--no-start`. Check whether a scoped key is unintentionally limiting a management operation, whether `MAGPIE_HOME` matches the desktop, and whether the running API version matches this binary. Use [Troubleshooting](Troubleshooting.md) for exact errors.

Source: [CLI command definitions](https://github.com/ChristianRelf/Magpie/blob/main/apps/cli/src/main.rs), [connection behaviour](https://github.com/ChristianRelf/Magpie/blob/main/apps/cli/src/api.rs).
