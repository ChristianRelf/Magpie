# Provider support and limitations

Reviewed against official documentation on 2026-10-08. Automated checks use isolated fixtures. No live account was used or billed during this implementation; protocol support below is implemented, not a claim of live certification for every provider/version.

| Connection              | Authentication and execution                                      | Discovery                                         | Usage and quotas                                                                |
| ----------------------- | ----------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------- |
| OpenAI API              | Key in OS store; Chat Completions                                 | `/v1/models`, filtered to supported text families | Response usage and rate-limit headers; subscription quotas unavailable          |
| Codex CLI               | Official CLI owns ChatGPT/API authentication; app-server JSON-RPC | `model/list`                                      | `account/rateLimits/read` and usage reports when supported by the installed CLI |
| Anthropic API           | API key; Messages streaming                                       | Models API                                        | Response usage and documented rate-limit headers                                |
| Claude Code             | Official `claude auth login`; headless CLI                        | CLI model aliases, labelled catalogue metadata    | Stream/result tokens and limit events; opt-in status-line reporting for 5-hour/weekly percentages and resets |
| Gemini API              | API key; generateContent streaming                                | Models API                                        | Response usage; quota failure/retry metadata when provided                      |
| Gemini CLI              | Official Google-account login; headless CLI                       | Configured model plus catalogue candidates        | Result usage when emitted; remaining quota/reset often unavailable              |
| OpenRouter              | Key; compatible chat endpoint                                     | Models API including returned prices/context      | Returned usage and key information when available                               |
| Groq, Mistral, DeepSeek | API keys; compatible chat endpoint                                | Provider models endpoints, supported families     | Returned usage/headers only                                                     |
| Ollama, LM Studio       | Local compatible endpoint                                         | Endpoint discovery                                | Returned usage, no fabricated subscription quotas                               |
| Custom endpoint         | Optional key; compatible endpoint                                 | `/models`                                         | Only fields the endpoint actually returns                                       |

## Claude Code allowance reporting

In Magpie 0.1.1, open **Providers → Claude Code → Manage → Enable usage reporting**. This opt-in integration updates the official CLI's `statusLine` setting, forwards its documented `rate_limits.five_hour` and `rate_limits.seven_day` fields into a private Magpie quota snapshot, and preserves any existing status-line command/output. Disable it in the same panel to restore the previous setting. `CLAUDE_CONFIG_DIR` is honoured; only one Claude CLI profile/connection can receive this reporting at a time. Project or managed status-line settings can override the user setting; workspace trust and hook policies must allow it. Magpie does not disable those protections.

Claude supplies these fields after a normal API response on supported subscriptions (documented for Pro/Max). Signing in or clicking Refresh does not fetch an allowance. Use Claude Code normally after enabling reporting; Magpie imports new local reports within approximately five seconds without issuing extra model requests. Missing percentages/reset times remain unknown. Observation timestamps are shown, and expired windows display **Awaiting update** until a fresh report arrives. Request-stream limit events are also recorded immediately, including when execution fails.

The bridge does not read Claude credentials or session transcripts, call private usage endpoints, or retain unrelated status-line fields. It continues to collect the reported quota fields while the Magpie window/service is closed. If you move a portable AppImage, disable and re-enable reporting from its new location. Restart an older background harness from Settings after upgrading Magpie.

Source: [Claude Code's documented status-line data and rate-limit usage](https://code.claude.com/docs/en/statusline#rate-limit-usage). This integration is tested with official-shape local fixtures; live account reporting still depends on the installed CLI, plan and settings.

## Subscription boundaries

Magpie never collects account passwords, extracts browser cookies, reads undocumented CLI tokens, impersonates official clients or sends subscription credentials to a third-party API. The official CLI keeps its own authentication and session lifecycle. Connect API authentication when direct subscription access is unsupported. CLI connections generally share the CLI's active local account; they do not create independent simultaneous subscription identities.

Codex and Gemini are coding agents with access to CLI-owned context/tools. Magpie requires explicit `agent.working_dir` and an agent-scoped client key to execute them. Ordinary text requests are not automatically routed through them. Claude Code can do plain generation with its tools disabled, strict MCP configuration and non-managed hooks disabled. Provider-managed policy remains authoritative. No CLI working-directory setting is advertised as a complete filesystem sandbox.

Subscription-backed CLI executions are not silently switched to metered API accounts. Subscription-to-API fallback is disabled until opted in. Known subscription billing environments are removed from delegated Claude/Gemini commands where appropriate. An unknown billing mode stays unknown; do not infer a subscription from the mere presence of a CLI.

## Capability limits

- The OpenAI API adapter uses Chat Completions. Codex-named API models are filtered because they require Responses. Codex CLI is the supported agent path; OpenAI's full Responses protocol is not implemented.
- CLI model aliases/candidates and catalogue-derived context, speed, quality and prices may be estimated or stale. Discovery does not prove a particular account is entitled to every candidate; actual execution errors update state.
- Gemini CLI verification detects configuration and official authentication-file presence without reading secrets. It cannot prove token freshness without executing a request. An expired session is surfaced on execution; the verification detail must be read accordingly.
- Anthropic cache read/write counts are included in input totals and separately recorded. Estimated API-equivalent costs from subscription CLIs are not actual charged spend.
- Unknown model prices mean unavailable cost. Configured spending limits based on estimated prices are routing preferences, not a provider-enforced billing cap. Provider billing controls remain authoritative.
- Tool calling and agent execution are distinct capabilities. External tools run in the client; Magpie never assumes their side effects are safe to repeat. CLI agent retries are conservative.
- CLIs may keep their own history/configuration outside Magpie. Magpie disables supported session persistence where possible; its retention setting does not delete provider CLI history.
- Custom endpoints can differ substantially. A compatible JSON shape alone does not establish full tool/vision/structured-output support.

## Official references

- [Codex authentication](https://developers.openai.com/codex/auth/) and [app-server protocol](https://developers.openai.com/codex/app-server/).
- [OpenAI Chat Completions](https://platform.openai.com/docs/api-reference/chat) and [model documentation](https://developers.openai.com/api/docs/models).
- [Claude Code headless execution](https://code.claude.com/docs/en/headless), [CLI flags](https://code.claude.com/docs/en/cli-reference), and [hook controls](https://code.claude.com/docs/en/hooks#disable-or-remove-hooks).
- [Gemini CLI authentication](https://geminicli.com/docs/get-started/authentication/), [headless execution](https://geminicli.com/docs/cli/headless/), and [policy engine](https://geminicli.com/docs/reference/policy-engine/).

Real-provider smoke tests are deliberately opt-in:

```bash
# Set MAGPIE_API_KEY to an integration key and MAGPIE_SMOKE_MODEL to a connected API/local model.
node scripts/provider-smoke.mjs --allow-real-provider
```

This makes one short request, disables fallback and can consume allowance or incur cost. CLI authentication and account-specific quotas additionally need manual verification on each supported operating system before release.
