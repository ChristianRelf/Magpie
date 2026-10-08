# Provider setup

A provider connection is an account/configuration entry, not the same thing as a model. One connection can expose several models, and several connections can expose the same public model ID. Use labels to distinguish accounts and an account-specific model key for exact routing.

## Choose an authentication path

| Path                | Where authentication lives                    | What Magpie needs                                              |
| ------------------- | --------------------------------------------- | -------------------------------------------------------------- |
| API key             | OS credential manager through Magpie          | Provider kind, key, optional base URL                          |
| Local server        | Server's own configuration                    | Reachable compatible URL, optional credential                  |
| Shared official CLI | CLI-managed login/configuration               | Installed executable, valid login, same OS user                |
| Saved CLI profile   | Provider-specific isolated profile/OS keyring | Available since 0.1.3; see [Saved sign-ins](Saved-sign-ins.md) |

Connecting and verifying are metadata operations; successful verification does not certify every model entitlement or future execution. Some CLI checks only establish configuration presence. Read the connection's status detail.

## API connection identifiers and defaults

These base URLs are the defaults in Magpie's provider registry. Magpie adds the adapter-specific resource path; do not supply a full `/chat/completions` endpoint as the base.

| Provider   | Kind                 | Default base URL                            |
| ---------- | -------------------- | ------------------------------------------- |
| OpenAI     | `openai`             | `https://api.openai.com/v1`                 |
| Anthropic  | `anthropic`          | `https://api.anthropic.com`                 |
| Gemini API | `gemini`             | `https://generativelanguage.googleapis.com` |
| OpenRouter | `open_router`        | `https://openrouter.ai/api/v1`              |
| Groq       | `groq`               | `https://api.groq.com/openai/v1`            |
| Mistral    | `mistral`            | `https://api.mistral.ai/v1`                 |
| DeepSeek   | `deep_seek`          | `https://api.deepseek.com/v1`               |
| Ollama     | `ollama`             | `http://127.0.0.1:11434/v1`                 |
| LM Studio  | `lm_studio`          | `http://127.0.0.1:1234/v1`                  |
| Custom     | `open_ai_compatible` | Required from you                           |

Use the exact kind spelling; for example, `open_router` and `deep_seek` contain underscores. Discover actual model IDs after connecting rather than copying an invented model name from a tutorial.

## Add an API key

In **Providers**, select the provider, name the connection, supply the API key, and verify it. Use the provider's own account console to create/revoke upstream keys. A Magpie integration key belongs to a different boundary and cannot authenticate directly to an upstream provider.

For the standalone CLI, run management in an owner session or with an admin key. This Bash example prompts for the provider key without putting it in command history:

```bash
read -rsp 'Provider API key: ' MAGPIE_PROVIDER_SECRET
printf '\n'
printf '%s\n' "$MAGPIE_PROVIDER_SECRET" | magpie connect openai --label 'Work API' --key-stdin
unset MAGPIE_PROVIDER_SECRET
```

Replace `openai` with the desired kind. If your shell still has a `read`/`execute` integration key from a tutorial, this management operation will correctly fail with insufficient scope. Open an owner shell without that variable or use a deliberate admin key.

After connecting, inspect `magpie providers` and `magpie models --available`. Use a route preview before a real prompt. Production credential storage must be available; do not use the debug plaintext file backend as a workaround for a locked production keyring.

## Local Ollama or LM Studio

Start the local server and make a model available using that product's normal workflow. Then connect:

```bash
magpie connect ollama --label 'Local Ollama'
magpie connect lm_studio --label 'Local LM Studio'
```

Choose the command for your installed server. If its port differs, provide `--base-url`. A quick model-list check against the upstream endpoint can separate server problems from Magpie problems:

```bash
curl --fail-with-body http://127.0.0.1:11434/v1/models
```

An empty upstream model list should be fixed in the local server first. The harness does not download model weights or start arbitrary local server processes. Returned token counts depend on what the endpoint reports; a local server has no subscription allowance for Magpie to invent.

## Custom compatible endpoint

```bash
magpie connect open_ai_compatible \
  --label 'Custom local server' \
  --base-url http://127.0.0.1:8080/v1
```

Add `--key-stdin` if it requires authentication. Include the API prefix once. Magpie's compatible adapter requests resources such as `/models` and `/chat/completions` beneath the base URL. A double `/v1/v1` or an HTML landing-page URL commonly causes 404/parse failures.

Provider HTTP clients do not follow redirects. Configure the final intended endpoint rather than relying on a redirect that could move authentication headers elsewhere. A working model list does not demonstrate support for tools, image inputs, reasoning options, or structured output. Check capabilities and then test only the feature you need.

## Official CLI connections

| Kind          | Executable | Plain generation              | Explicit agent request          |
| ------------- | ---------- | ----------------------------- | ------------------------------- |
| `claude_code` | `claude`   | Supported with tools disabled | Supported subject to CLI policy |
| `codex_cli`   | `codex`    | Requires agent context        | Required                        |
| `gemini_cli`  | `gemini`   | Requires agent context        | Required                        |

Install and sign in using the official tool under the same user who runs Magpie, then connect its kind. Magpie looks at an explicit path override, `PATH`, and common installation directories. Services launched at login may have a different environment from an interactive terminal.

See the individual guides: [Claude Code](Claude-Code.md), [Codex CLI](Codex-CLI.md), and [Gemini CLI](Gemini-CLI.md). Shared CLI connections, including the older 0.1.2 workflow, follow the CLI's active account; creating extra labels does not create independent CLI identities.

## Manage connection failures

| State         | Meaning to investigate                         | First repair                                 |
| ------------- | ---------------------------------------------- | -------------------------------------------- |
| `needs_auth`  | Authentication missing/rejected/expired        | Complete upstream sign-in or replace the key |
| `unavailable` | Local executable/server/dependency unavailable | Fix the dependency or endpoint               |
| `error`       | Verification failed with another error         | Read the status message and harness log      |
| `disabled`    | Connection disabled by preference              | Enable only if it should receive work        |
| `pending`     | Verification has not completed                 | Verify and inspect the resulting detail      |
| `connected`   | Adapter considers the connection usable        | Still check model/capability entitlement     |

After fixing the underlying cause, verify/refresh the connection, inspect available models, and preview routing. Only then decide whether a small real request is necessary. If a provider credential changes, restart/reconnect as required by that provider rather than editing Magpie's SQLite tables.

References: [provider registry](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-core/src/provider.rs), [provider limitations](https://github.com/ChristianRelf/Magpie/blob/main/docs/PROVIDERS.md), and [Troubleshooting](Troubleshooting.md).
