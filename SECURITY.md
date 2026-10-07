# Security and privacy

Magpie runs locally and has no required hosted backend. Provider requests go to the account's configured endpoint or its official CLI. There is no analytics upload. Update checks contact only the release-time HTTPS channel when configured and enabled.

Provider API keys use the operating system credential manager. Production writes fail closed when that store is unavailable; unlock Keychain/Credential Manager/Secret Service and reconnect. `MAGPIE_SECRET_STORE=file` is honoured only by debug builds for isolated testing. Production does not silently downgrade credentials to plaintext.

The API binds to loopback by default, validates Host headers and requires a bearer token except for minimal liveness. Client keys are scoped, hashed and revocable. The local owner's bootstrap token is in the private per-user data directory; same-user malware is outside this trust boundary. Do not share that directory or the owner's token. Explicit network binding is an advanced configuration and requires your own transport protection; Magpie does not supply TLS for remote deployment.

`execute` does not grant arbitrary local agent workflows. `agent` permission also requires explicit agent options and should only be granted to trusted software. Official agents run as the logged-in user and inherit their provider-managed policies and configuration. A working directory is not a filesystem jail. A `read` key can see retained telemetry across clients; Magpie is a single-user harness, not a multi-tenant isolation service.

Request/response contents are not retained by default. Local history, retention, diagnostic logging, export and deletion are available in Settings. Provider errors and metadata may contain provider-supplied descriptions; never include secrets in labels/model identifiers. Magpie redacts credential patterns, does not export provider keys and uses private native file exports. Provider CLIs may retain their own session history independently.

Startup registration is opt-in and per-user. The desktop only launches allowlisted official CLI login commands. External URLs use the native opener; webviews have a restrictive CSP and no generic shell execution capability. Update installation requires a compiled public key and signed artifacts. Initial local builds are not signed/notarised releases.

CI runs Rust and JavaScript tests and dependency audits. Dependabot proposes dependency/action updates. Report security issues privately to the repository owner; include affected versions and a minimal reproduction, never active credentials.

## Current dependency audit findings

The 2026-10-08 Rust audit completed with two informational findings in the Linux GTK/Tauri dependency tree: [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html), an unsound `glib` string-variant iterator fixed in the newer 0.20 line, and [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html), an unmaintained compile-time macro dependency. The current Tauri GTK3 stack depends on `glib` 0.18. These findings are not suppressed; review upstream fixes or a maintained backport before declaring the dependency audit clean. Magpie code does not directly invoke that iterator, but this is not proof of absence of transitive use. The JavaScript production dependency audit reported no known vulnerabilities.
