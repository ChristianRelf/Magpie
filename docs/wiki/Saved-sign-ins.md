# Saved sign-ins and multiple credentials

**Version requirement:** separate Codex browser profiles and saved Claude Code tokens are available in **v0.1.3**, published on 2026-10-08. They are not available in older v0.1.2 installers. Use a matching app, SDK, and service; a local 0.1.2 checkout will not have these APIs.

The [implementation record](https://github.com/ChristianRelf/Magpie/blob/main/docs/STATUS.md) records fixture verification and the remaining account-owner/platform checks. This guide describes the implemented workflow without claiming that additional real accounts have been certified.

## What a connection represents

Each saved connection has an account ID, label, authentication mode, and its own credential/profile association. Name them by purpose, such as Work, Personal, and Backup. Several connections can expose the same public model name. Select the account-specific model key when exact identity matters.

| Provider path                  | Independent credentials                               | Important limit                                          |
| ------------------------------ | ----------------------------------------------------- | -------------------------------------------------------- |
| API connections                | One stored key per connection                         | Several keys may share one upstream quota                |
| Codex separate browser sign-in | Private profile per connection, official login/logout | Complete browser sign-ins one at a time                  |
| Claude saved token             | One user-supplied setup-token per connection          | No account identity/plan/allowance proof from CLI status |
| Shared CLI login               | The official CLI's active local login                 | Additional labels do not isolate identities              |
| Gemini CLI                     | Shared login                                          | Use separate Gemini API keys for API-account connections |

## Add another credential

In **Providers → Manage → Add another credential**, create a new connection for that provider. Existing shared connections do not need migration. You can rename, disable, replace credentials, and disconnect individual saved connections.

Disabling keeps the credential but excludes the connection from routing. Disconnecting removes the connection's credential/profile authentication. Stop/cancel active executions first. Disconnecting one saved profile should not change an unrelated shared terminal/IDE login.

## Separate Codex browser sign-in

Choose **Separate browser sign-in**. Magpie creates a private `CODEX_HOME` for this connection and asks the official app-server to perform browser consent. The profile requires OS keyring storage via `cli_auth_credentials_store="keyring"`; unavailable credential storage causes an error instead of a plaintext fallback.

Complete the browser flow for the intended account. The harness monitors completion independently of the connect dialog. If the desktop is closed, the harness must remain running for that monitoring; configure background operation before relying on it. The official CLI uses a local callback listener, so complete one browser sign-in before starting another.

No existing CLI credential file is copied into the new profile. The official app-server manages refresh/logout. After sign-in, verify the connection and discover its models before routing work to it.

## Saved Claude Code token

Choose **Saved Claude Code token**, run the official `claude setup-token` flow, and supply the resulting token through the private credential input. Magpie stores it in the OS credential manager and selects it for the corresponding process as `CLAUDE_CODE_OAUTH_TOKEN`, with isolated `CLAUDE_CONFIG_DIR` context.

Inherited authentication overrides and unsupported user/project authentication settings are excluded so the chosen saved token is not silently replaced. Provider-managed policy still applies. CLI status verifies that a token is configured; it does not prove remote validity, identity, subscription plan, or account-wide allowance.

Replace expired/revoked setup-tokens explicitly. Magpie does not promise automatic renewal of this credential type. A rejected saved token remains disconnected until replaced. The status-line allowance bridge is offered only for shared login, preventing another account's percentages from being attributed to a saved token.

## Programmatic management

With the 0.1.3 SDK and an admin client, the implemented shape is:

```ts
const profile = await client.connectProvider({
  kind: "codex_cli",
  label: "Work",
  auth_mode: "isolated",
});
const login = await client.loginProvider(profile.id);
// Open login.auth_url through a trusted browser flow.
// Observe provider state/events for completion.
```

The login endpoint is `POST /v1/providers/{id}/login`, for isolated Codex profiles. Creating saved Claude connections uses `auth_mode: "saved_token"` and `oauth_token`; credential replacement uses the account PATCH endpoint. Secrets are input-only. Authentication modes and managed profile paths cannot be changed arbitrarily through `options`.

The service advertises support so an older service cannot silently reinterpret a saved-auth request as a shared-login request. If the app reports an older/incompatible harness, restart the matching service rather than bypassing that check. The current standalone CLI's basic `connect` flags are not a substitute for these UI/SDK profile flows.

## Fallback and account identity

Fallback applies to work sent through Magpie, according to routing settings, capabilities, and reported state. After credential/limit failures, the engine rechecks eligibility, excludes unavailable account candidates, and records routing changes. Already-streamed output and write-capable agent tasks are not blindly replayed.

Subscription-to-billable switching still requires consent, including when saved subscriptions are unavailable before a new request. Manual/no-fallback selection remains respected. Inspect the selected connection on every attempt when diagnosing a switch.

## Recovery checklist

For missing controls, compare app and harness versions first. For Codex login conflicts, finish/cancel the other browser flow. For keyring errors, unlock the OS store in the actual desktop session. For rejected Claude tokens, replace the token through management. For missing saved-token quotas, recognise that account-wide allowance is unavailable. For disconnect conflicts, finish active work and retry the specific connection operation.

See [Security and permissions](Security-and-permissions.md), [Provider setup](Provider-setup.md), and the [profile API note](https://github.com/ChristianRelf/Magpie/blob/main/docs/API.md#authentication-profile-management).
