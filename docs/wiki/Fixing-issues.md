# Fixing issues

An effective fix starts with a reproducible failure and a clear layer boundary. Use [Troubleshooting](Troubleshooting.md) for immediate user recovery; use this page to report or implement the underlying correction.

## Establish the baseline

Record the desktop version, CLI version, running harness version, OS/architecture, and provider CLI version where relevant. A newer window connected to an older service can resemble a missing-feature bug. Older v0.1.2 and current v0.1.3 profiles have different capabilities.

Identify the data root and whether your request uses owner credentials or a scoped key. Check the endpoint actually used by the failing client. Do not publish the token while comparing configuration.

Reproduce with the smallest request that still fails. Remove unrelated options one at a time, preserving the field that triggers the failure. Prefer route previews, model/status queries, and explicit local fixtures before making any paid provider calls.

## Classify the failure

| Failure                               | Likely code layer                                        | Useful evidence                                   |
| ------------------------------------- | -------------------------------------------------------- | ------------------------------------------------- |
| Cannot discover/start/stop service    | `magpie-runtime`, native shell                           | Runtime info, lock/port state, lifecycle log      |
| Wrong key/scope behaviour             | `magpie-api`, `magpie-engine` clients, `magpie-security` | Endpoint, principal/scopes, status/type           |
| Wrong provider request/response       | `magpie-providers`                                       | Redacted protocol fixture, capability metadata    |
| Incorrect route/exclusion             | `magpie-router`                                          | Input, config, inventory, limits, decision        |
| Unsafe retry/fallback or cancellation | `magpie-engine`                                          | Attempt/event order and side-effect boundary      |
| Missing history/incorrect aggregation | `magpie-store`, engine                                   | Query bounds, retained records, source/provenance |
| Stream parser breaks on chunks        | `packages/sdk`                                           | Minimal bytes/chunk sequence, terminal event      |
| UI shows stale/wrong data             | React queries/view helpers                               | API response versus rendered source/range         |
| Tray/login/export/updater issue       | `apps/desktop/src-tauri`, engine autostart               | Native environment, path, operation result        |

Do not fix a provider protocol bug by hiding its error in a chart, or a display bug by inventing a missing backend metric. Preserve the distinction between reported, calculated, estimated, and unavailable values.

## Write a useful issue

Copy this template into [GitHub Issues](https://github.com/ChristianRelf/Magpie/issues):

```text
Title: [concrete trigger] causes [observable failure]

Versions:
- Magpie desktop:
- Magpie CLI / running harness:
- Source commit or release tag:
- OS and architecture:
- Provider CLI version, if applicable:

Connection and client:
- Provider kind and auth mode (no credentials):
- Native API / Chat Completions / CLI / SDK / MCP / desktop:
- Key scope names (no key value):
- Default or custom port/data directory:

Steps to reproduce:
1.
2.
3.

Expected result:
Actual result:
HTTP status and error.type, or exact redacted UI message:
Execution ID and approximate timestamp:
Whether it reproduces with an isolated local fixture:
Relevant redacted log excerpt:
```

For quota/chart issues also include source selector, UTC range, observation time, provenance, and whether the work happened through Magpie or directly in a provider CLI. For a stream issue, state whether a terminal event arrived. For an agent failure, state whether files/tool actions occurred before it stopped.

Keep screenshots and logs focused. Remove emails/identities if unnecessary, secrets, private paths, and prompt/response contents you do not intend to share. Sensitive security reports should follow the private [security policy](https://github.com/ChristianRelf/Magpie/blob/main/SECURITY.md).

## Implement the correction

1. Inspect the nearest existing tests and contract types.
2. Capture a minimal fixture/regression that demonstrates the failure.
3. Change the responsible layer, keeping API/SDK/UI contracts consistent if the shape changes.
4. Preserve authentication, billing consent, content retention, and replay boundaries.
5. Run the focused test, then the relevant integration check.
6. Update the user-facing guide if the behaviour or recovery procedure changes.

Provider fixes should preserve cancellation and report actual errors/usage. Routing fixes should be deterministic for a given fixture and respect explicit exclusions. Storage fixes should consider migrations and old data. UI fixes should preserve source labels and missing-data semantics rather than substituting attractive but ungrounded values.

## Regression examples worth testing

| Bug                                           | Meaningful regression                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------- |
| Tool stream duplicated after failure          | Emit a tool/output event then fail; assert no unsafe replay            |
| Revoked credential still used                 | Revoke/replace and verify the selected account/secret lifecycle        |
| Wrong source combined in charts               | Supply overlapping account and local data; assert they remain separate |
| Midnight usage in wrong date                  | Fixture around UTC boundary with explicit bounds                       |
| Missing quota shown as zero                   | Omit the provider field; assert unavailable state                      |
| UTF-8 corruption in SSE                       | Split a multibyte character and frame across chunks                    |
| GUI close unexpectedly kills opted-in service | Native lifecycle test for both close policies                          |

Use deterministic local fixtures, not a live account or paid request in CI. A fixture proves the exercised code path, not all upstream account entitlements. Manual tests should describe exactly which account-independent or account-owner behaviour was checked.

## Validate and communicate

Run the commands relevant to the layer from [Development and testing](Development-and-testing.md). For a docs-only change, validate links and examples without rebuilding every installer. For a protocol/lifecycle change, include the relevant Rust and integration checks.

Describe the concrete trigger, resulting behaviour, and validation in the change summary. Record material limits, such as fixture-only verification or an untested platform. Do not claim a bug fixed because a test that never exercised the failing path happened to pass.

If the cause is upstream or environmental, document the reproducible boundary and a supported recovery/workaround. Do not silently widen permissions, bypass provider policy, or erase evidence to make a symptom disappear.
