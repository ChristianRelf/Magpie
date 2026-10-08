# Provider support and limitations

Reviewed against official documentation on 2026-10-08. Automated checks use isolated fixtures. No live model request was made or billed during this implementation; protocol support below is implemented, not a claim of live certification for every provider/version.

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

Magpie never collects account passwords, extracts browser cookies, reads undocumented CLI tokens, impersonates official clients or sends subscription credentials to a third-party API. The official CLI keeps its authentication lifecycle. Connect API authentication when direct subscription access is unsupported. Existing CLI connections continue to use their shared local login. Separate Codex profiles and saved Claude Code tokens can hold additional authorised credentials as described below; Gemini CLI remains a single shared login.

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

## Codex account activity

Overview and Analytics use the official Codex app-server `account/usage/read` report for connected Codex accounts. The Usage source selector switches between account history (including external Codex clients) and requests executed through Magpie. These totals are never added together. Account reports contain daily token buckets, not per-request model, latency or cost data; dates are UTC and unavailable dates remain unknown. Today is a reported snapshot, not a live token stream. Refresh usage requests a fresh report; automatic provider monitoring checks every two minutes with backoff. Older Codex versions and API-key-only authentication may not support account history.

Reference: [Codex app-server account usage](https://learn.chatgpt.com/docs/app-server#7-token-usage-chatgpt). A read-only metadata check succeeded with Codex CLI 0.156.1 on the development machine; no model execution was used for this check.

## Saved authentication profiles

Each Magpie connection represents one credential/profile. Give connections names such as Work, Personal or Backup. **Providers → Manage → Add another credential** opens a new connection for the same provider. Disabling a connection keeps its saved credential; disconnecting removes only that connection's credential. Existing connections need no migration.

- **Codex:** select **Separate browser sign-in**. The official app-server owns browser consent, token refresh and logout. Every profile gets its own private `CODEX_HOME`, and Magpie requires `cli_auth_credentials_store="keyring"`; an unavailable OS credential manager causes an error rather than a plaintext fallback. Browser sign-in completion runs in the harness even when the window closes. No existing CLI credential files are read or copied. This flow does not sign the shared terminal/IDE account out. Only one browser sign-in should be completed at a time because the official CLI uses a local callback listener.
- **Claude Code:** select **Saved Claude Code token**, run the official `claude setup-token` browser flow and paste its token. Magpie stores it in the OS credential manager and injects it into the selected official CLI process as `CLAUDE_CODE_OAUTH_TOKEN`, with a separate `CLAUDE_CONFIG_DIR`. Inherited authentication overrides and project/user authentication settings are excluded; provider-managed policies still apply. CLI status confirms configuration only; provider validity is checked during execution. Identity, plan and account-wide allowance are unavailable for these tokens. Tokens must be replaced when expired/revoked; Magpie does not claim it can silently renew a setup-token. The status-line bridge is only offered for the shared login so its quotas cannot be attributed to a saved token.
- **API keys:** add one connection for each key. Credentials remain separately stored and independently replaceable/revocable. Multiple keys on the same provider account may share the same quota and spending limits.

Automatic fallback uses eligible saved connections for requests sent through Magpie, subject to Routing preferences, capability checks and reported limits. A rejected/expired credential or account-wide quota is excluded before choosing the next candidate. Routing changes are emitted to the requesting client and recorded in the execution inspector. Already-streamed output and write-capable agent tasks are never replayed. Subscription-to-billable switching still requires opt-in. Magpie does not change the login of unrelated running CLI/IDE sessions or invent additional allowance for another key.

References: [Codex configuration directories](https://learn.chatgpt.com/docs/config-file/environment-variables), [secure Codex authentication storage](https://learn.chatgpt.com/docs/auth), [official app-server browser authentication](https://learn.chatgpt.com/docs/app-server#3-log-in-with-chatgpt-browser-flow), [Claude Code setup tokens](https://code.claude.com/docs/en/authentication#generate-a-long-lived-token), and [Claude configuration directories](https://code.claude.com/docs/en/env-vars).
