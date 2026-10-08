# Security and permissions

Magpie is a single-user local harness. Its security boundary separates integration clients from provider credentials and administrative access. It is not a multi-tenant server or an OS sandbox for untrusted agents.

## Three kinds of credentials

| Credential                               | Used by                           | Stored/managed by                               |
| ---------------------------------------- | --------------------------------- | ----------------------------------------------- |
| Provider API key / supported saved token | Upstream adapter                  | OS credential manager                           |
| Official CLI login                       | Provider CLI                      | Official tool and its configured secure storage |
| Magpie integration key                   | External client calling local API | Full token shown once; hash stored by Magpie    |

The owner bootstrap credential in `admin.token` is separate from all of these. Keep it private and never distribute it as an integration key. A new Magpie key cannot repair an expired provider login; reconnect upstream authentication instead.

## Scopes

| Scope     | Grants                                                               | Typical client                    |
| --------- | -------------------------------------------------------------------- | --------------------------------- |
| `read`    | Models, runtime/usage/limit metadata, previews, events               | Dashboard or model picker         |
| `execute` | Ordinary execution                                                   | Script or compatible editor       |
| `agent`   | Agent requests with explicit context, in addition to execute         | Trusted local repository workflow |
| `admin`   | Provider/settings/model/key management, shutdown, cancellation by ID | Deliberate administration         |

Read access exposes retained telemetry across clients. It is not project-scoped isolation. Retained prompt/response content in the execution detail endpoint is administrative. Prefer separate named keys so a tool can be revoked independently.

Disabling **Require read permissions** allows any valid key to access read endpoints. It does not disable authentication and does not remove execute/agent/admin requirements. Scope reduction is not a fix for a client that is sending the wrong token.

## Agent permissions

Codex and Gemini CLI requests require explicit `agent.working_dir` and an `agent`-capable caller. `execute` alone does not authorise a local agent workflow. `allow_writes` defaults false and must be set deliberately for editing.

The official CLI runs as the logged-in user, follows its own/managed policies, and can have access beyond the working directory. That directory is context, not a filesystem jail. A failed/cancelled request may already have read files or produced changes; cancellation is not rollback.

For a client-owned function-tool loop, validate returned arguments and enforce your application's permissions before performing actions. Model output is untrusted input to your tool implementation.

## Local network boundary

By default the service listens on `127.0.0.1:7878`. Host-header checks accept recognised loopback names, and browser CORS allows the native webview and development origins rather than arbitrary websites. `/health` is a minimal unauthenticated probe; protected endpoints require a key.

Magpie does not supply TLS for a public deployment. Advanced non-loopback binding requires deliberate configuration and your own transport/access protection. The standalone scoped CLI only accepts loopback HTTP endpoints. Use the service in its intended local context unless you have separately designed the network boundary.

Same-user malware can cross local private-file/process boundaries. Storing the owner token in a private directory does not protect it from every program running with your privileges.

## Secret storage and redirects

Production provider credential writes use Keychain, Windows Credential Manager, or Secret Service and fail closed when unavailable. Unlock/fix the OS store in the actual user session. `MAGPIE_SECRET_STORE=file` is debug-only plaintext testing support, not a production recovery recommendation.

Provider HTTP clients do not follow redirects, so custom authentication headers are not silently forwarded to another endpoint. Configure the final provider URL. An endpoint returning a redirect should be diagnosed as endpoint configuration, not worked around by disabling this protection.

## Content, telemetry, and exports

Prompt/response content retention defaults off. Metadata can still contain labels, model IDs, task types, timestamps, errors, and token/cost information. Do not place secrets in account labels, model identifiers, or metadata. Provider error messages can contain descriptions you should review before sharing.

Turning content retention on affects future stored executions. Turning it off purges stored request/response content in the current implementation. Disabling local history affects recording; it does not by itself revoke providers or delete independent CLI session history.

Exports omit provider keys and request/response content, but remain potentially sensitive metadata. The whole data directory can contain `admin.token`, retained contents, and debug secrets. Never attach an unreviewed archive to a public issue.

There is no required hosted Magpie account or analytics upload. Cloud inference still sends request contents to the chosen provider. Update checks contact the configured release-time HTTPS channel when present/enabled.

## Claude reporting privacy

The opt-in bridge uses the official status-line data. It preserves the existing command and stores only reported quota fields, account association, and observation time. It does not extract credentials, read transcripts, or call private usage endpoints. The previous status-line command continues with its existing user privileges. Project/managed overrides and hook trust still matter.

Saved Claude tokens introduced in 0.1.3 do not use the shared-login quota bridge; see [Saved sign-ins](Saved-sign-ins.md).

## Updates and dependency evidence

The documented v0.1.3 installers are unsigned/unnotarised, with no configured automatic update channel. An implemented signature-verifying updater does not make an unsigned installer signed. Platform signing and updater signing are distinct release tasks.

The [security policy](https://github.com/ChristianRelf/Magpie/blob/main/SECURITY.md) records known dependency audit findings. Re-run audits for the commit you ship and review their actual output. A test pass or historical audit result is not a guarantee of present security.

## Respond to an exposed credential

If a Magpie client key leaked, revoke it and replace the affected tool's configuration. If an upstream provider key/token leaked, revoke it through the provider and replace that connection's credential. If the owner token/data directory leaked, treat the local administrative boundary as compromised and review what else was exposed before restoring trust.

Report security issues privately to the repository owner as described in [SECURITY.md](https://github.com/ChristianRelf/Magpie/blob/main/SECURITY.md). Include versions and a minimal redacted reproduction; do not send active credentials to demonstrate the problem.
