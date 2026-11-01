# Routing and fallback

The router makes a deterministic decision from the request, discovered model inventory, saved preferences, known limits, and available execution history. It does not call a separate model to decide where a prompt should go.

## Preview before execution

```bash
magpie route 'Find the bug in this function' --task debugging --preset best_quality
magpie --json route 'Summarise these meeting notes' --preset economical
```

The native equivalent is `POST /v1/route` with a normal native request body. It requires `read` and performs classification/selection without inference. For agent models, include `agent.working_dir` in the native preview; the CLI `route` command has no `--cwd` flag.

A successful decision includes classification, preset, ranked candidates, reasons, score components, rejected candidates, and fallback policy. If no candidate survives, inspect the returned error and provider/model state instead of executing repeated test prompts.

## How selection proceeds

1. Validate input and derive task/capability/context requirements.
2. Resolve explicit model selection or the configured manual model.
3. Exclude disabled models, unavailable accounts, providers outside the request filter, unmet capabilities, insufficient context, and blocking known limits.
4. Enforce billing/cost/reserve preferences where the necessary data exists.
5. Score remaining candidates and apply task, provider, model, and fallback-order preferences.
6. Return candidates with explanations; the engine executes the first eligible choice.

An earlier position in an order cannot make an unavailable or incompatible model eligible. A model's existence in the catalogue does not prove account entitlement.

## Presets

| Preset            | Emphasis                                       | Common use                          |
| ----------------- | ---------------------------------------------- | ----------------------------------- |
| `automatic`       | Balance changes with estimated task complexity | General mixed workloads             |
| `best_quality`    | Stronger eligible models                       | Difficult reasoning or review       |
| `fastest`         | Speed and available measurements               | Short interactive work              |
| `economical`      | Lower estimated marginal cost                  | Routine summarisation/extraction    |
| `preserve_limits` | Known allowance headroom                       | Keeping premium allowance for later |
| `manual`          | Configured model preference                    | Deliberate default selection        |

Quality/speed catalogue metadata can be estimated. Measured history is used where available and enabled; it is not a universal benchmark. Premium reserves only use known, reported, unexpired allowance. Unknown quota cannot establish a precise remaining balance.

## Precedence and request overrides

For the preset, the request override wins, followed by a matching task-rule preset, followed by the saved default. Other optional request preferences inherit their saved counterpart when unset.

| Request preference  | Meaning                                                 |
| ------------------- | ------------------------------------------------------- |
| `preset`            | Override this request's preset                          |
| `allow_fallback`    | Permit/prevent moving to other candidates               |
| `allow_billable`    | Explicit request-level permission for billable accounts |
| `max_cost_usd`      | Maximum estimated request cost where price data exists  |
| `providers`         | Allow only these provider kinds                         |
| `exclude_providers` | Exclude these provider kinds                            |

Example restricted to local providers:

```json
{
  "model": "auto",
  "input": "Summarise these notes: ...",
  "task_type": "summarisation",
  "preferences": {
    "preset": "economical",
    "allow_billable": false,
    "providers": ["ollama", "lm_studio"],
    "allow_fallback": true
  }
}
```

Provider filters are kind-based. For exact account selection, use its model key rather than a provider filter.

## Model IDs and strict selection

Supported references include `auto`, an account/model key, a provider/model ID, and a bare model ID. Get these from `/v1/models` or the UI. A provider/model or bare name can match more than one connection.

Explicit model selection receives strong preference but can allow alternatives when fallback is enabled. To require one account/model, send its key with `preferences.allow_fallback: false`. The manual preset still applies eligibility rules. Pinning does not bypass tools, context, authentication, or explicit agent requirements.

## Task hints

Canonical task values are `simple_question`, `code_generation`, `debugging`, `repository_analysis`, `math_reasoning`, `planning`, `summarisation`, `tool_execution`, `large_context`, `data_extraction`, and `general`. Omit the hint or use `auto` to classify heuristically.

Use a task hint to express intent, not to grant capabilities. Setting `repository_analysis` does not authorise file access; only explicit agent context and the appropriate scope do that. Context estimates include input and expected output, so a request can be excluded even when the input alone appears to fit.

## Fallback versus retry

A retry attempts the same candidate again for a transient failure. Fallback changes the candidate. The engine limits total attempts and uses a conservative short delay for supported transient retries. Turning off fallback is about candidate switching; it is not a general guarantee that every transport failure has exactly one attempt.

| Situation                                              | Expected behaviour                                |
| ------------------------------------------------------ | ------------------------------------------------- |
| Compatible alternative exists before meaningful output | May fall back if error and policy allow           |
| Response already emitted meaningful output/tool events | Stop rather than blindly replay                   |
| Write-capable agent request may have changed files     | Stop rather than blindly replay                   |
| User/client cancellation                               | Stop; cancellation is not a reason to fail over   |
| No eligible alternative                                | Return failure with available diagnostic evidence |
| Subscription candidate would switch to metered API     | Requires explicit opt-in                          |

`routing_changed` identifies from/to models and a reason. Final records include attempts. Handle a terminal failure even if the initial HTTP response succeeded. A retry performed by your external client is a new decision and can duplicate work; inspect the original record before retrying side-effecting requests.

## Spending and allowance boundaries

Saved `allow_metered` controls automatic routing to metered accounts, while `allow_subscription_to_api` controls subscription-to-API switching. In the current router, an explicit pinned billable model can bypass the automatic-routing metered exclusion unless the request sets `allow_billable: false`. Use a request-level prohibition when that is the intended constraint.

Estimated cost limits depend on known prices and estimated tokens. Unknown pricing can prevent a reliable estimate. Spend notifications observe locally tracked API costs; they do not enforce a provider billing cap. Use provider-side billing controls for an actual provider budget.

Version 0.1.3 adds saved-credential eligibility rechecks and stricter consent when saved subscriptions are unavailable before routing. [Saved sign-ins](Saved-sign-ins.md) describes that version boundary.

## Diagnose an unexpected route

Check the request's overrides, saved preset, task rule, explicit model key, disabled state, provider filters, context estimate, capabilities, limits, billable flags, reserves, and history. Compare the selected and rejected reasons before changing preferences. Verify that you saved UI changes and are calling the same harness/data directory as the desktop.

Source: [router implementation](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-router/src/lib.rs). Related: [API reference](API-reference.md), [Troubleshooting](Troubleshooting.md#no-eligible-model).
