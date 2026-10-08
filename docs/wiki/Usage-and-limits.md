# Usage, limits, costs, and provenance

Always identify the source before interpreting a number. Magpie distinguishes work executed by the harness, account-wide reports supplied by a provider, reported allowance windows, and locally estimated/calculated values.

## Two activity sources

| Source                       | Includes                                                                                   | Does not establish                                    |
| ---------------------------- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------- |
| Magpie requests              | Retained executions routed through this harness                                            | All activity performed directly in provider apps/CLIs |
| Reported Codex account usage | Supported provider-reported daily token buckets, including external clients where supplied | Per-request model, latency, request counts, or costs  |

Select the source in **Analytics**. Their totals are not added because account usage may already include Magpie's Codex requests. A different date range, retention policy, account, or provider publication delay can also make totals differ.

`magpie usage` and `/v1/usage/summary`, `/timeseries`, `/breakdown` describe local harness activity. `/v1/usage/provider-reports` returns the separate supported account snapshots. Account reports are not reconstructed from CLI transcripts.

## Token fields

| Field            | Interpretation                               |
| ---------------- | -------------------------------------------- |
| Input tokens     | Normalised prompt/input usage                |
| Output tokens    | Reported/estimated generated tokens          |
| Cached input     | Cached subset of input where reported        |
| Cache writes     | Provider-reported cache creation information |
| Reasoning tokens | Subset of output where reported              |
| Provenance       | How the number was obtained                  |

Do not add cached tokens to a total that already includes them, or reasoning tokens to output again. The Anthropic adapter normalises cache read/write counts into input totals while retaining the separate cache fields.

When neither input nor output usage is supplied for an execution, the engine can estimate tokens from text length and label them `estimated`. That is different from fabricating a provider-reported count. Partial/absent fields and failure cases still need careful interpretation.

## Provenance vocabulary

- **Reported:** returned by an authoritative provider response, official CLI event, or supported account endpoint.
- **Calculated:** computed from available inputs, such as measured timing or token counts and known prices.
- **Estimated:** based on incomplete data or catalogue assumptions.
- **Unavailable:** there is no supported value to show.

Catalogue capabilities/context/prices are not the same evidence as live provider reports. Preserve these distinctions in exports, dashboards, and bug reports.

## Cost is not always spend

Execution costs may be reported, calculated, or estimated. Subscription usage can have an **API-equivalent** comparison value; it is not a bill for that execution. Unknown model pricing means no dependable cost estimate. Do not infer a zero price merely because a value is unavailable.

The CLI prints separate reported/calculated/estimated/API-equivalent totals. In your own dashboards, keep those categories distinguishable. Daily spend notifications use locally tracked API costs; they cannot include arbitrary activity outside Magpie or enforce provider billing.

Routing `max_cost_usd` is an estimate-based selection preference, not a hard provider budget. Actual provider billing and account-side controls remain authoritative.

## Allowance windows

Providers can supply multiple windows with different scopes and reset times: request/token limits, short subscription windows, weekly windows, or model-specific allowances. A request can be blocked by the most restrictive applicable window even when another bar looks healthy.

Known reported unexpired windows inform routing and reserves. Missing percentages/reset timestamps remain unknown. A qualitative warning does not justify inventing a numeric percentage. Expired windows display **Awaiting update** until new information arrives; that label is not proof the full allowance has been restored.

Inspect the provider, window label, scope, exhausted state, provenance, reset time, and observation time together. A new client key or duplicate account label does not create new upstream allowance.

## Refresh and polling

| Mechanism                            | Baseline behaviour                                                                  |
| ------------------------------------ | ----------------------------------------------------------------------------------- |
| UI telemetry polling                 | Default 5 seconds; user-configurable                                                |
| Supported active provider monitoring | Approximately 120 seconds                                                           |
| Supported idle provider monitoring   | Approximately 900 seconds                                                           |
| Codex account monitoring             | Treated as active even when Magpie is idle, because external clients may be running |
| Failed provider polling              | Backoff, starting at 240 seconds and increasing up to 1 hour                        |
| Claude local status-line import      | Checked on the approximately 5-second background tick                               |

These are scheduling intervals, not provider freshness guarantees. The **Refresh provider limits** preference controls supported remote polling. Claude snapshot import is a local reporting path. Unsupported provider endpoints cannot become available by lowering the UI refresh interval.

Monitoring uses supported allowance-free metadata/report interfaces; it does not generate prompts. For Claude shared-login percentages, enable the bridge and use Claude normally so a supported response can supply the official status-line fields. See [Claude Code](Claude-Code.md).

## Calendar and dates

Calendar buckets use UTC. An action near local midnight can therefore appear on a different date than your local wall clock. For comparisons, use the same UTC bounds and source.

Daily, weekly, and cumulative local modes are different presentations of retained activity. A blank local-history day means no retained records for that day; a provider account day can instead be unknown because it was not supplied. Do not replace unknown account dates with fabricated zeroes.

Default local retention is 90 days. A year-shaped calendar does not promise a year of retained records. Increase retention before you need the longer period. Clearing/deleting old history cannot be undone by increasing the setting later.

## Export responsibly

Use Analytics for the selected usage source's export, or Settings for local execution metadata export. Native local export:

```bash
curl --fail-with-body "$MAGPIE_URL/v1/usage/export?range=7d&format=csv" \
  -H "Authorization: Bearer $MAGPIE_API_KEY" \
  -o magpie-usage.csv
```

JSON is selected with `format=json`. Metadata exports omit provider secrets and prompt/response contents. They still contain account/model/client labels and execution details; review before sharing.

The current export path goes through the store's 5,000-record list cap despite requesting a larger internal limit. It also does not apply all aggregate filters. For a high-volume period, split by custom date bounds and verify counts against the summary/list endpoints. Treat **Export all** as a convenience export, not a guaranteed complete database backup. A stopped-service database backup is described in [Settings and storage](Settings-and-storage.md).

## When numbers look wrong

Check source, account, UTC range, retention, last observation, provenance, cache/reasoning subset handling, and provider support before filing a bug. For external Codex activity, choose account reports; for an execution's route/latency/cost, choose local requests. For a stale Claude allowance, check its reporting bridge and next normal observation.

References: [usage contracts](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-core/src/usage.rs), [monitoring schedule](https://github.com/ChristianRelf/Magpie/blob/main/crates/magpie-engine/src/background.rs), [Troubleshooting](Troubleshooting.md).
