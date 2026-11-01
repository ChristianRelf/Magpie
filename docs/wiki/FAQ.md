# FAQ and glossary

## Is Magpie a chat application?

No chat screen is included. The desktop manages providers, models, routing, keys, and telemetry. Send prompts from a compatible client, script, CLI, SDK, or MCP client.

## Do I need a Magpie account?

No hosted Magpie account is required. You still need the authentication required by your chosen provider, or a working local model server.

## Does local mean every request stays on my computer?

The harness and local database run on your computer. Cloud provider adapters and official cloud-backed CLIs send request content to their provider. Restrict routing to actual local endpoints if that is your intended workflow; non-metered subscription usage is not necessarily local inference.

## Does Magpie include model weights or install provider CLIs?

You supply the local server/models or install an official provider CLI separately. Magpie discovers/connects those dependencies. It does not guarantee that a connected catalogue candidate is available to your account.

## Can I use a subscription as a general-purpose provider API key?

Magpie delegates supported work to the official CLI and its authentication. It does not extract consumer credentials or turn a subscription into unrestricted third-party API access. Use an API-key connection where an API is required.

## Why are there several kinds of keys?

An upstream key authenticates to the provider. A Magpie integration key authenticates a client to the local service. The private owner token grants local administrative access. See [Security and permissions](Security-and-permissions.md).

## Can the same provider be connected more than once?

API keys can have separate connections. Shared CLI connections follow the official CLI's active login. Version 0.1.3 adds separate Codex browser profiles and saved Claude tokens; Gemini CLI remains shared. Multiple credentials can still share upstream limits. See [Saved sign-ins](Saved-sign-ins.md).

## Why does Codex or Gemini reject an ordinary prompt?

Those CLI adapters are agents with local context/tools. Magpie requires explicit `agent.working_dir` and `execute` plus `agent` permission. A task name like `repository_analysis` does not grant file access.

## Is a working directory a sandbox?

No. It establishes task context for an official CLI running as your user. Its configuration/managed policies still apply. Grant agent/write access only to intended trusted workflows.

## Is Magpie's Responses endpoint identical to OpenAI Responses?

No. `/v1/responses` uses Magpie's native request/result/event contract. `/v1/chat/completions` implements a compatibility subset. Hosted tools, conversations, stored Responses retrieval, batches, audio generation, and image generation are outside that advertised surface.

## Which URL should my client use?

Use `http://127.0.0.1:7878` for the SDK/native origin and `http://127.0.0.1:7878/v1` for clients that append Chat Completions paths. Replace the port if configured differently. The upstream local model server URL is a different connection.

## Does route preview spend tokens?

Classification and selection are local and do not execute a model. `magpie run`, `client.respond`, and MCP `magpie_generate` do execute. Verification/report refresh may make supported metadata requests but do not issue an inference prompt to classify work.

## Does selecting a model guarantee it stays selected?

Use an account-specific model key and `preferences.allow_fallback: false` when you need strict candidate selection. An explicit preference with fallback enabled can move to another eligible model. Transient retry of the same candidate is a separate mechanism.

## Will a retry repeat file edits?

The engine avoids blind replay after meaningful output/tool events and for write-capable agent work. Your external client can still start a new request, so inspect failures before retrying. Cancellation/failure does not undo changes already made.

## Why do charts disagree with the provider's website?

Check whether you are viewing local Magpie requests or account-wide reports, UTC dates, report observation time, retention, and unsupported fields. Provider publication can lag. Do not sum overlapping account/local sources.

## Is an unknown quota zero or unlimited?

Neither. It means the supported source did not provide a usable value. Awaiting update means an earlier window expired and needs a fresh observation. A qualitative warning is not a precise percentage.

## Why is there a cost for subscription usage?

API-equivalent values are comparison estimates, not charged spend. Read provenance and billing mode. Local notifications and estimated routing budgets do not enforce a provider-side spending cap.

## Why is my year calendar mostly empty?

Local history defaults to 90-day retention and only contains retained harness work. Increase retention before expecting a full year. Unavailable account-report dates also differ from days with zero local retained activity.

## Why does a key work in curl but fail in a browser?

CORS/Host restrictions and protocol differences are separate from token validity. Use the correct allowed local/native integration. Do not put an administrative key in a public browser bundle.

## Can I close the app and keep using the API?

Enable and save Keep harness running after quit. The default stops the service. Minimise to tray and Launch harness at login are separate controls. The scoped CLI/MCP client does not launch a stopped service automatically.

## Does an installer include the CLI on PATH?

Do not assume that. The standalone `magpie` executable can be built/installed separately from the repository. See [CLI reference](CLI-reference.md#install-the-cli).

## Can I recover a lost integration key?

The complete token is displayed only when created; Magpie stores a hash. Revoke/replace the key rather than trying to retrieve the original secret from its metadata.

## Is Export all a full backup?

No. It exports local execution metadata and currently passes through a 5,000-record cap. It does not back up the database configuration, credentials, or CLI login. See [Usage and limits](Usage-and-limits.md) and [backup guidance](Settings-and-storage.md#backup-and-restore).

## Are builds signed and updates automatic?

The documented v0.1.3 builds are unsigned/unnotarised and use manual updates. The updater implementation needs a configured signed channel. A preference toggle cannot supply missing release credentials/configuration.

## Where should I report a bug?

Start with [Troubleshooting](Troubleshooting.md), then use the [issue template](Fixing-issues.md#write-a-useful-issue). Security-sensitive reports should follow the private [security policy](https://github.com/ChristianRelf/Magpie/blob/main/SECURITY.md).

## Glossary

| Term               | Meaning                                                                     |
| ------------------ | --------------------------------------------------------------------------- |
| Harness            | Magpie's independent local Rust service                                     |
| Adapter            | Translation between Magpie's contracts and a provider API/CLI               |
| Connection/account | One configured provider credential or login association                     |
| Model key          | Account-specific reference used to distinguish identical public model names |
| Candidate          | A model/connection that passed eligibility and was ranked                   |
| Rejection          | Explanation for excluding a model from selection                            |
| Preset             | A ranking emphasis such as quality, speed, or economy                       |
| Scope              | Permission carried by an integration key                                    |
| Provenance         | Whether a metric is reported, calculated, estimated, or unavailable         |
| Allowance window   | A reported limit with its scope and optional reset time                     |
| API-equivalent     | Estimated API comparison cost for subscription usage                        |
| SSE                | Server-Sent Events framing for live streams                                 |
| MCP                | Model Context Protocol; Magpie supplies a local stdio bridge                |
| TTFT               | Time to first token measured for execution                                  |
| WAL                | SQLite write-ahead log; relevant to consistent backups                      |
