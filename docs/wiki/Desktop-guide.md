# Desktop guide

The desktop configures and observes the local harness. It does not execute a provider directly from React. Commands pass through the typed SDK and authenticated API; native features such as the tray and file dialogs pass through the Tauri shell.

## Onboarding

Connect a usable provider, select your routing preferences, and finish the first-run flow. You can add more providers and change preferences later. For external tools, create an integration key after setup. If the initial connection fails, fix its status message before repeatedly retrying onboarding; [Provider setup](Provider-setup.md) explains dependencies.

## Overview

Use Overview as a service/account summary: connected plans, reported allowances, upcoming resets, and activity. Its layout may differ between the published release and ongoing UI work. Read the source label and observation time beside numbers. Missing allowance fields do not mean unlimited usage.

When a request fails, open Activity for execution-specific evidence. When an account is unavailable, inspect that provider. An aggregate card usually does not contain the full diagnostic reason.

## Providers

Connect accounts and use their inspector/manage controls for details. Labels should identify purpose, such as Personal, Work API, or Local Ollama, without embedding secrets.

| Action     | Effect                                                                    | When to use it                                    |
| ---------- | ------------------------------------------------------------------------- | ------------------------------------------------- |
| Verify     | Checks connection/authentication state using the adapter                  | After login or fixing a dependency                |
| Refresh    | Refreshes supported discovery/usage/limit information                     | When inventory or reported state may have changed |
| Disable    | Removes the connection from usable routing while preserving configuration | Temporary maintenance or avoiding that account    |
| Disconnect | Removes the connection and its stored credential                          | Retiring that connection                          |
| Rename     | Changes the display label                                                 | Distinguishing otherwise similar accounts         |

Verification is not a universal remote-token test. For example, Gemini CLI configuration presence and saved Claude token configuration do not prove that a future execution will succeed. Read the detail the adapter reports.

The shared Claude Code inspector offers opt-in usage reporting. Separate sign-in/credential controls introduced in 0.1.3 are documented in [Saved sign-ins](Saved-sign-ins.md) and are not present in v0.1.2.

## Models

Search and inspect discovered models. Check provider/connection identity, context information, supported capabilities, and unavailability reasons. Preferences such as favourites, disabled state, priorities, reserves, and price overrides affect routing or presentation; they do not grant provider entitlement or add a missing capability.

A public model name can be exposed through several accounts. Use the account-specific key when exact connection selection matters. Refresh the provider before assuming a renamed or removed model still exists upstream.

## Activity

Activity is the execution inspector. Select a record to see its status, selected model/account, timings, tokens, costs, errors, and attempts. Fallback can make the final selected connection different from the first candidate. A route error may have no provider attempt at all.

Retained prompt/response text is available only when content retention was enabled for that execution and the caller has administrative access. Enabling it later cannot reconstruct past content. Cancelling work stops continuation as supported by the provider; it does not undo emitted output or completed agent edits.

## Analytics

Choose **Usage source** first. Codex account history and local Magpie requests answer different questions and are not summed. Choose the range, inspect time-series points, and use calendar daily/weekly/cumulative modes for local activity. Hover or accessible data shows detail. Calendar dates use UTC.

The source selector is persisted locally. If a chart looks empty after a request, check that it is showing Magpie requests, that the request falls within the selected dates, and that local history is enabled. For external Codex sessions, choose the account source and inspect when it was reported. [Usage and limits](Usage-and-limits.md) explains exports and unavailable metrics.

## Routing

Choose a preset, model/provider preference order, task rules, fallback policy, and billing controls. Save the changes. Then preview realistic prompts and inspect the resulting candidates. A disabled/unavailable model cannot become eligible just by moving it earlier in an order.

Use both explicit model selection and disabled fallback when a task must stay on one connection. Subscription-to-metered switching is a separate opt-in. See [Routing and fallback](Routing-and-fallback.md).

## Integrations

Create named keys with the smallest useful scope set and copy the one-time token. Use the endpoint format for the intended protocol: the SDK/native client takes the root address, while many Chat Completions clients expect `/v1` in their base URL. Revoke a key to prevent future authentication with it; create a replacement when a token is lost.

Keep `agent` for trusted filesystem workflows and `admin` for intentional management. A read key can see retained telemetry across clients; this is a single-user service, not project-isolated tenancy.

## Settings

Edit, then **Save changes**. The page includes service Start/Stop/Restart, theme/reduced motion, startup/tray/background behaviour, history/retention, polling, notifications, privacy, API settings, and updates. **Discard** restores the saved draft.

Port, concurrency, and diagnostic logging changes need a harness restart to apply fully. Restart cancels active work. **Export all** exports execution metadata; **Clear history** permanently removes history rather than disconnecting providers. Make a deliberate export/backup before deleting data.

Native controls can be unavailable in a browser-only development view. A Vite page does not supply a native file picker, tray, or the Tauri lifecycle. Use [Development and testing](Development-and-testing.md) for that distinction.

## Daily maintenance checklist

Verify account state after changing upstream authentication, review routing before a new class of workload, inspect observation timestamps before treating quotas as current, and check the selected usage source before comparing charts. Use [Troubleshooting](Troubleshooting.md) to repair a specific failure; database deletion is rarely the first useful step.
