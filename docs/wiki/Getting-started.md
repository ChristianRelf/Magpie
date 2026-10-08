# Getting started

This walkthrough takes you from a fresh installation to an inspected execution. For architecture first, read [How it works](How-it-works.md). For package-specific instructions, read [Installation and updates](Installation-and-updates.md).

## Before you start

You need Magpie, one usable provider connection, and a client that can call its local API. Pick one provider path:

| Path             | Prepare first                                                  | Suitable first request         |
| ---------------- | -------------------------------------------------------------- | ------------------------------ |
| API              | A provider API key and an OS credential manager you can unlock | Plain text generation          |
| Local            | Running Ollama/LM Studio/compatible server with a model        | Plain text generation          |
| Claude Code      | Installed official CLI and completed login                     | Tool-disabled plain generation |
| Codex/Gemini CLI | Installed official CLI, completed login, a working directory   | Explicit agent request         |

A cloud subscription is not automatically an API key. Use the supported official CLI for that connection type. Magpie does not perform a paid test prompt just to classify a route.

## Connect in the desktop

1. Open Magpie and complete onboarding.
2. Choose a provider, give it a useful label, and supply its endpoint/credential if needed.
3. Verify that the connection is usable in **Providers**. Read the status message if it needs authentication or a local dependency.
4. Open **Models** and inspect availability and capabilities. Keep the discovered ID for explicit model selection later.
5. Open **Routing**, choose a preset, and save. Start with `automatic` unless you need a specific model or policy.

For local servers, the URL points to the upstream server, such as `http://127.0.0.1:11434/v1`. That is different from Magpie's own client endpoint, `http://127.0.0.1:7878`.

## Create a client key

In **Integrations**, create a key named for your client. Select `read` and `execute`. Copy its token immediately and keep it private. The list later shows metadata, not the recoverable full token.

In Bash:

```bash
read -rsp 'Magpie integration key: ' MAGPIE_API_KEY
printf '\n'
export MAGPIE_API_KEY
export MAGPIE_URL='http://127.0.0.1:7878'
```

The variable lasts for this shell session and its child processes. Store persistent secrets through your client's supported secret configuration rather than committing them to a project.

## Check without executing

```bash
curl --fail-with-body "$MAGPIE_URL/health"
curl --fail-with-body "$MAGPIE_URL/v1/status" \
  -H "Authorization: Bearer $MAGPIE_API_KEY"
curl --fail-with-body "$MAGPIE_URL/v1/models" \
  -H "Authorization: Bearer $MAGPIE_API_KEY"
curl --fail-with-body "$MAGPIE_URL/v1/route" \
  -H "Authorization: Bearer $MAGPIE_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"model":"auto","input":"Explain a local API in one sentence."}'
```

Success means `/health` identifies Magpie, authenticated status identifies the service, and routing finds a suitable candidate. `/health` alone does not test a key, credential store, provider account, or model entitlement. `auto` is a virtual routing choice, not a installed model.

If the only connections are Codex/Gemini CLIs, the plain route will exclude them because it lacks agent context. Follow the agent example below; this is intentional permission enforcement.

## Send a small request

This performs inference and may spend allowance or money:

```bash
curl --fail-with-body "$MAGPIE_URL/v1/responses" \
  -H "Authorization: Bearer $MAGPIE_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{
    "model":"auto",
    "input":"Explain a local API in one sentence.",
    "max_output_tokens":128,
    "stream":false
  }'
```

Read `output_text`. The result also includes the selected model/connection, usage, timings, attempt count, and routing information. Missing cost or usage fields remain unavailable.

To stream, use `"stream": true` and `curl --no-buffer`. Handle `failed` events as well as `completed`; see [Streaming and tools](Streaming-and-tools.md).

## Windows PowerShell equivalent

PowerShell's `Invoke-RestMethod` avoids Bash quoting and `curl` alias differences. This example prompts for the key without echoing it, then sends a non-streaming request:

```powershell
$magpieSecret = Read-Host 'Magpie integration key' -AsSecureString
$magpieCredential = [pscredential]::new('magpie', $magpieSecret)
$magpieHeaders = @{ Authorization = 'Bearer ' + $magpieCredential.GetNetworkCredential().Password }
$magpieUrl = 'http://127.0.0.1:7878'
Invoke-RestMethod "$magpieUrl/health"
Invoke-RestMethod "$magpieUrl/v1/models" -Headers $magpieHeaders
$magpieBody = @{
    model = 'auto'
    input = 'Explain a local API in one sentence.'
    max_output_tokens = 128
    stream = $false
} | ConvertTo-Json
Invoke-RestMethod "$magpieUrl/v1/responses" -Method Post -Headers $magpieHeaders -ContentType 'application/json' -Body $magpieBody
Remove-Variable magpieSecret, magpieCredential, magpieHeaders
```

## First explicit agent request

Create a separate key with `read`, `execute`, and `agent`, or use your trusted local owner CLI. Replace the model placeholder with a discovered ID and use an existing absolute directory:

```bash
magpie run 'Read this repository and summarise its structure. Do not edit files.' \
  --model codex_cli/MODEL_ID --cwd /absolute/path/to/repository
```

The request can read local context under the official CLI's policies. `--cwd` is not a filesystem jail. Writes require `--allow-writes`; see [Security and permissions](Security-and-permissions.md). For a strict no-fallback request, use the native API with `preferences.allow_fallback: false`.

## Inspect and keep using it

Open **Activity**, select the execution, and check the connection, status, tokens, time, and attempts. Open **Analytics** and select **Magpie requests** to inspect this harness's work; Codex account reports are a different source.

Before closing the app, decide whether external tools need continued access. The default close policy stops the service. Enable and save **Keep harness running after quit** if needed. See [How to use Magpie](How-to-use.md) for ongoing workflows, or [Troubleshooting](Troubleshooting.md) if any check failed.
