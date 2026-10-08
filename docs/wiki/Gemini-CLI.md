# Gemini CLI

Gemini CLI and Gemini API are separate connection types. `gemini_cli` delegates to the installed official agent; `gemini` uses an API key and the generateContent adapter. Choose the path that matches your authentication and task.

## Connect the shared CLI login

Install Gemini CLI and complete its official interactive authentication under the same user that runs Magpie. Then connect the CLI:

```bash
gemini --version
magpie connect gemini_cli
magpie providers
magpie models --available
```

Magpie's verification checks supported configuration/authentication-file presence without extracting secrets. This can show that a login is configured but does not prove token freshness or account entitlement. An expired session may only become clear when execution reaches the provider. Read the verification detail rather than interpreting `connected` as a complete live certification.

Model discovery uses configured information and catalogue candidates. A candidate being listed does not establish that every account can run it.

## Use explicit agent permissions

Gemini CLI is an agent with local tools/context. Magpie requires an explicit working directory and an integration key with both `execute` and `agent`.

```bash
magpie run 'Review the documentation structure and suggest improvements' \
  --model gemini_cli/MODEL_ID --cwd /absolute/path/to/repository
```

Replace the model placeholder with the discovered ID. Omit `--allow-writes` for a review task. For intended edits, that flag requires `--cwd`. Native API requests use `agent.working_dir` and `agent.allow_writes`; use `preferences.allow_fallback: false` if you need exact selection.

Provider policy may refuse actions or require interaction that a headless request cannot complete. Do not treat the working directory as a sandbox or retry a failed agent action without checking what already happened. The CLI's own history and configuration are separate from Magpie's retention controls.

## Understand usage and quotas

Magpie records result usage when the CLI emits it. Remaining quota and reset timestamps are often unavailable. A known request token count is not enough to reconstruct a subscription's remaining allowance, so the UI leaves unsupported quota fields unknown.

A provider quota/rate-limit error may include useful retry metadata. Honour the returned information; a new Magpie key or duplicate connection label does not create extra upstream allowance. For account-wide usage charts, do not assume that the Codex-specific report path exists for Gemini CLI.

## Use Gemini API instead

For plain generation without a local agent working directory, connect `gemini` with an API key. Use the desktop or an owner-session stdin flow:

```bash
read -rsp 'Gemini API key: ' MAGPIE_PROVIDER_SECRET
printf '\n'
printf '%s\n' "$MAGPIE_PROVIDER_SECRET" | magpie connect gemini --key-stdin --label 'Gemini API'
unset MAGPIE_PROVIDER_SECRET
```

API billing is separate from the shared CLI subscription/account path. Multiple API-key connections are supported, but keys belonging to one account can share its quota. Gemini CLI remains one shared login in the documented saved-profile implementation; it does not gain independent browser profiles just because Codex supports them.

## Troubleshooting

If detection fails, compare `gemini --version` in your terminal with the service's executable discovery. Login items can run with a minimal `PATH`; check installed runtime dependencies and use the same OS user.

If a request is excluded, inspect the routing reason for missing agent context, capabilities, or model availability. If it reaches the provider and fails authentication, reauthenticate through the official CLI, then verify the Magpie connection. If approval is required, adjust only the authorised CLI workflow/policy; Magpie cannot answer an interactive prompt on your behalf.

If all that works but quota remains unknown, that may be the supported state rather than a bug. Capture the version, authentication type, exact redacted error, and execution ID before reporting a problem.

References: [Magpie adapter](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-providers/src/cli/gemini_cli.rs), [official Gemini CLI authentication](https://geminicli.com/docs/get-started/authentication/), [headless execution](https://geminicli.com/docs/cli/headless/), and [Troubleshooting](Troubleshooting.md).
