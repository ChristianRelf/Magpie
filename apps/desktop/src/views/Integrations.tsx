import { useState } from "react";
import { Copy, KeyRound, Plus, Terminal, Shield } from "lucide-react";
import { Page } from "@/components/Page";
import { Badge, Button, EmptyState, Field, Input, Mono, Panel, PanelHeader } from "@/components/ui/core";
import { Dialog, Select } from "@/components/ui/controls";
import { QueryState } from "@/components/QueryState";
import { useKeys } from "@/lib/queries";
import { useHarness } from "@/lib/harness";
import { useAction } from "@/lib/action";
import { copyText } from "@/lib/desktop";
import { dateTime, relative } from "@/lib/format";

function CodeBlock({ code }: { code: string }) {
  const action = useAction();
  return (
    <div className="relative rounded-md border border-border bg-bg p-3">
      <Button
        className="absolute top-2 right-2"
        size="xs"
        variant="ghost"
        icon={<Copy className="size-3" />}
        onClick={() => void action.run(() => copyText(code), "Copied")}
      >
        Copy
      </Button>
      <pre className="overflow-x-auto pr-14 font-mono text-[11.5px] leading-relaxed text-fg-muted">{code}</pre>
    </div>
  );
}

export function Integrations() {
  const { client, connection } = useHarness();
  const query = useKeys();
  const action = useAction();
  const [create, setCreate] = useState(false);
  const [name, setName] = useState("");
  const [access, setAccess] = useState("execute,read");
  const [token, setToken] = useState<string | null>(null);
  const [revoke, setRevoke] = useState<string | null>(null);
  const [example, setExample] = useState("curl");
  const url = connection?.url ?? "http://127.0.0.1:7878";
  const examples: Record<string, string> = {
    curl: `curl ${url}/v1/responses \\\n  -H "Authorization: Bearer $MAGPIE_API_KEY" \\\n  -H 'Content-Type: application/json' \\\n  -d '{"model":"auto","input":"Inspect this function for bugs","task_type":"code_generation"}'`,
    sdk: `import { MagpieClient } from '@magpie/sdk';\n\nconst magpie = new MagpieClient({\n  baseUrl: '${url}',\n  apiKey: process.env.MAGPIE_API_KEY,\n});\n\nfor await (const event of magpie.stream({\n  model: 'auto',\n  input: 'Summarise the supplied document',\n  task_type: 'summarisation',\n})) {\n  if (event.type === 'text_delta') process.stdout.write(event.text);\n  if (event.type === 'failed') throw new Error(event.error.message);\n}`,
    openai: `// OpenAI-compatible Chat Completions subset\nimport OpenAI from 'openai';\n\nconst client = new OpenAI({\n  baseURL: '${url}/v1',\n  apiKey: process.env.MAGPIE_API_KEY,\n});\nconst result = await client.chat.completions.create({\n  model: 'auto',\n  messages: [{ role: 'user', content: 'Explain this algorithm' }],\n});`,
    mcp: `// MCP client configuration (Magpie CLI on PATH)\n{\n  "mcpServers": {\n    "magpie": {\n      "command": "magpie",\n      "args": ["mcp"],\n      "env": {\n        "MAGPIE_API_KEY": "<your scoped key>",\n        "MAGPIE_URL": "${url}"\n      }\n    }\n  }\n}`,
  };
  return (
    <Page
      title="Integrations"
      subtitle="Give your tools one local endpoint"
      actions={
        <Button variant="primary" icon={<Plus className="size-3.5" />} onClick={() => setCreate(true)}>
          Create access key
        </Button>
      }
    >
      <div className="mx-auto max-w-5xl space-y-5 p-5">
        <Panel>
          <PanelHeader title="Local API" actions={<Badge>Authenticated · API v{connection?.api_version ?? 1}</Badge>} />
          <div className="flex items-center gap-3 p-4">
            <Terminal className="size-5 text-fg-subtle" />
            <div className="flex-1">
              <Mono>{url}/v1</Mono>
              <p className="mt-1 text-xs text-fg-subtle">
                For tools that support a custom endpoint. Use a separate, revocable key for each integration.
              </p>
            </div>
            <Button
              icon={<Copy className="size-3" />}
              onClick={() => void action.run(() => copyText(`${url}/v1`), "Endpoint copied")}
            >
              Copy
            </Button>
          </div>
        </Panel>
        <Panel>
          <PanelHeader
            title="Access keys"
            subtitle="Secret keys are shown once. Only hashes are retained by the harness."
          />
          <QueryState pending={query.isLoading} error={query.error} retry={query.refetch} />
          {!query.data?.keys.length && query.isSuccess && (
            <EmptyState
              icon={<KeyRound />}
              title="No integrations connected"
              description="Create an access key for your IDE, script, or local agent."
            />
          )}
          <div className="divide-y divide-border">
            {query.data?.keys.map((key) => (
              <div key={key.id} className="flex items-center gap-3 p-4">
                <Shield className="size-4 text-fg-subtle" />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium">{key.name}</span>
                    {key.revoked_at && <Badge>Revoked</Badge>}
                  </div>
                  <p className="mt-1 font-mono text-2xs text-fg-subtle">
                    {key.prefix}… · {key.scopes.join(", ")}
                  </p>
                </div>
                <div className="text-right text-2xs text-fg-subtle">
                  <p>Created {dateTime(key.created_at)}</p>
                  <p>{key.last_used_at ? `Used ${relative(key.last_used_at)}` : "Never used"}</p>
                </div>
                <Button variant="ghost" disabled={!!key.revoked_at} onClick={() => setRevoke(key.id)}>
                  Revoke
                </Button>
              </div>
            ))}
          </div>
        </Panel>
        <Panel>
          <PanelHeader
            title="Connect an application"
            actions={
              <Select
                label="Integration example"
                size="sm"
                value={example}
                onChange={setExample}
                options={[
                  { value: "curl", label: "HTTP / cURL" },
                  { value: "sdk", label: "TypeScript SDK" },
                  { value: "openai", label: "OpenAI SDK" },
                  { value: "mcp", label: "MCP" },
                ]}
              />
            }
          />
          <div className="space-y-3 p-4">
            <CodeBlock code={examples[example]} />
            <p className="text-xs text-fg-subtle">
              Replace the key in your application's environment. Magpie's native /v1/responses uses Magpie event types;
              it is not the OpenAI Responses wire protocol. The TypeScript SDK source is included in this workspace.
            </p>
          </div>
        </Panel>
        <Panel>
          <PanelHeader title="Command-line utility" />
          <div className="p-4">
            <CodeBlock
              code={`magpie status\nmagpie providers\nmagpie models\nmagpie usage\nmagpie limits\nmagpie route "Review this function"\nmagpie serve`}
            />
          </div>
        </Panel>
      </div>
      <Dialog
        open={create}
        onOpenChange={setCreate}
        title="Create an integration key"
        description="Grant only the access this application needs."
        footer={
          <>
            <Button onClick={() => setCreate(false)}>Cancel</Button>
            <Button
              variant="primary"
              loading={action.busy}
              disabled={!name.trim()}
              onClick={() =>
                void action.run(
                  async () => {
                    const result = await client!.createKey(name.trim(), access.split(","));
                    setToken(result.token);
                    setCreate(false);
                    setName("");
                  },
                  undefined,
                  ["keys"],
                )
              }
            >
              Create key
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <Field label="Application name">
            <Input
              autoFocus
              maxLength={80}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Repository automation"
            />
          </Field>
          <Field label="Permissions">
            <Select
              label="Key permissions"
              value={access}
              onChange={setAccess}
              options={[
                { value: "execute,read", label: "Execute and read telemetry" },
                { value: "execute", label: "Execute only" },
                { value: "read", label: "Read telemetry only" },
                { value: "execute,read,agent", label: "Agent access (local files and execution)" },
              ]}
            />
          </Field>
          {access.includes("agent") && (
            <p className="text-xs text-fg-muted">
              Agent access permits official CLIs to read local files and, when requested, modify them. Grant this only
              to trusted applications. Working directories are not filesystem sandboxes.
            </p>
          )}
        </div>
      </Dialog>
      <Dialog
        open={!!token}
        onOpenChange={(o) => !o && setToken(null)}
        title="Save your access key"
        description="This key will not be displayed again. Store it in the consuming application's secret store."
        footer={<Button onClick={() => setToken(null)}>I have saved the key</Button>}
      >
        <div className="space-y-3">
          <code className="selectable block break-all rounded-md border border-border bg-bg p-3 text-xs">{token}</code>
          <Button
            icon={<Copy className="size-3" />}
            onClick={() => void action.run(() => copyText(token!), "Key copied")}
          >
            Copy key
          </Button>
        </div>
      </Dialog>
      <Dialog
        open={!!revoke}
        onOpenChange={(o) => !o && setRevoke(null)}
        title="Revoke this integration?"
        description="New requests using this key will be rejected immediately."
        footer={
          <>
            <Button onClick={() => setRevoke(null)}>Cancel</Button>
            <Button
              variant="danger"
              loading={action.busy}
              onClick={() =>
                void action.run(
                  async () => {
                    await client!.revokeKey(revoke!);
                    setRevoke(null);
                  },
                  "Key revoked",
                  ["keys"],
                )
              }
            >
              Revoke key
            </Button>
          </>
        }
      >
        <p className="text-xs text-fg-muted">Create a new key to reconnect this application later.</p>
      </Dialog>
    </Page>
  );
}
