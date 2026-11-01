# Magpie handbook

Magpie is a local service and desktop control centre for compatible AI providers. It connects API accounts, local servers, and official provider CLIs, routes requests, and explains the resulting activity. Your tools supply the prompts; Magpie has no chat screen.

This handbook covers four jobs: **understand how it works, learn how to use it, fix issues, and troubleshoot your installation**. The pages are also checked into `docs/wiki` so you can read them offline and review documentation changes alongside code.

## Choose a starting point

| Your goal                       | Read this                             |
| ------------------------------- | ------------------------------------- |
| Understand the architecture     | [How it works](How-it-works.md)       |
| Find a practical workflow       | [How to use Magpie](How-to-use.md)    |
| Install and run a first request | [Getting started](Getting-started.md) |
| Something is broken             | [Troubleshooting](Troubleshooting.md) |
| Report or implement a fix       | [Fixing issues](Fixing-issues.md)     |

## Install and operate

- [Installation and updates](Installation-and-updates.md): packages, checksums, upgrades, startup problems, removal.
- [Desktop guide](Desktop-guide.md): every screen and everyday maintenance.
- [Provider setup](Provider-setup.md): connection types, identifiers, endpoints, verification.
- [Claude Code](Claude-Code.md): authentication, tool-disabled generation, allowance reporting.
- [Codex CLI](Codex-CLI.md): explicit agent requests and reported account history.
- [Gemini CLI](Gemini-CLI.md): login, agent execution, and quota limitations.
- [Saved sign-ins](Saved-sign-ins.md): separate credentials and profile workflows introduced in 0.1.3.
- [Routing and fallback](Routing-and-fallback.md): selection, policy precedence, and safe retries.
- [Usage and limits](Usage-and-limits.md): charts, provenance, resets, costs, and exports.
- [Settings and storage](Settings-and-storage.md): defaults, directories, environment, backups.
- [Security and permissions](Security-and-permissions.md): trust boundaries, scopes, retention.

## Integrate and develop

- [API reference](API-reference.md): endpoints, request fields, errors, and query parameters.
- [Streaming and tools](Streaming-and-tools.md): SSE, terminal events, tools, cancellation.
- [CLI reference](CLI-reference.md): installation, commands, flags, stdin, and scope behaviour.
- [SDK and MCP](SDK-and-MCP.md): TypeScript examples and a stdio client template.
- [Development and testing](Development-and-testing.md): build dependencies and focused checks.
- [Release maintenance](Release-maintenance.md): packaging, signing, verification, and release gates.
- [Wiki maintenance](Wiki-maintenance.md): edit, check, stage, and publish these pages.
- [FAQ and glossary](FAQ.md): short answers and terminology.

## Versions and evidence

Documentation baseline: **published v0.1.3**, checked on **2026-10-08**. This release includes saved authentication profiles and the refined desktop. Older v0.1.2 installations do not offer the separate Codex/saved Claude profile workflows. Source builds and local working copies can differ from published releases. Check the actual app and harness versions before treating a missing control as a bug.

The [release list](https://github.com/ChristianRelf/Magpie/releases) identifies available installers. The [implementation record](https://github.com/ChristianRelf/Magpie/blob/main/docs/STATUS.md) distinguishes implemented behaviour, automated fixtures, and live/manual verification. An adapter existing in code does not certify every account or CLI version.

Examples use the default `http://127.0.0.1:7878` endpoint. Substitute your actual port. Shell blocks marked `bash` assume Bash; use the PowerShell example in [Getting started](Getting-started.md) on Windows. `MODEL_ID`, `ACCOUNT_ID`, and similar uppercase names are placeholders, not usable credentials or discovered models.

## Need help?

Use [Troubleshooting](Troubleshooting.md) before deleting data or repeatedly executing paid requests. A route preview does not run inference. A real generation request can consume allowance or money, even when used to diagnose a problem.

For a bug, follow the [report template](Fixing-issues.md#write-a-useful-issue) and open [GitHub Issues](https://github.com/ChristianRelf/Magpie/issues). For sensitive security problems, follow the [security policy](https://github.com/ChristianRelf/Magpie/blob/main/SECURITY.md). Never include active credentials or an unreviewed copy of the application data directory.
