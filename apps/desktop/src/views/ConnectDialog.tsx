import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Copy,
  ExternalLink,
  Eye,
  EyeOff,
  RefreshCw,
  ShieldCheck,
  Terminal,
} from "lucide-react";
import type {
  BillingMode,
  CliStatus,
  ProviderDescriptor,
  ProviderKind,
} from "@magpie/sdk";
import { Button, Field, Input, cn } from "@/components/ui/core";
import { Dialog, Select } from "@/components/ui/controls";
import { ProviderMark } from "@/components/ui/marks";
import { errorMessage, useToast } from "@/components/ui/feedback";
import { useHarness } from "@/lib/harness";
import { useClis, useProviders } from "@/lib/queries";
import { copyText, isTauri, openCliLogin, openExternal } from "@/lib/desktop";

const GROUPS: { title: string; note: string; kinds: ProviderKind[] }[] = [
  {
    title: "Subscriptions",
    note: "Signed in through the provider's official CLI",
    kinds: ["claude_code", "codex_cli", "gemini_cli"],
  },
  {
    title: "API keys",
    note: "Metered usage billed by the provider",
    kinds: [
      "anthropic",
      "openai",
      "gemini",
      "open_router",
      "groq",
      "mistral",
      "deep_seek",
    ],
  },
  {
    title: "Local and custom",
    note: "Models on this machine or any compatible endpoint",
    kinds: ["ollama", "lm_studio", "open_ai_compatible"],
  },
];

function CopyLine({ text }: { text: string }) {
  const toast = useToast();
  return (
    <div className="flex items-center gap-2 rounded-md border border-border-strong bg-bg py-1 pr-1 pl-2.5">
      <code className="min-w-0 flex-1 truncate font-mono text-[12px] text-fg">
        {text}
      </code>
      <Button
        size="xs"
        variant="ghost"
        icon={<Copy className="size-3" />}
        onClick={() =>
          void copyText(text).then(() =>
            toast({ title: "Copied", tone: "success" }),
          )
        }
      >
        Copy
      </Button>
    </div>
  );
}

function CliSetup({
  d,
  cli,
  onRecheck,
  checking,
}: {
  d: ProviderDescriptor;
  cli?: CliStatus;
  onRecheck: () => void;
  checking: boolean;
}) {
  const toast = useToast();
  if (!cli?.installed) {
    return (
      <div className="space-y-3">
        <div className="rounded-lg border border-border bg-bg-subtle p-3 text-xs text-fg-muted">
          {d.name} needs the official{" "}
          <span className="font-mono text-fg">{d.cli_binary}</span> command-line
          tool. Install it with its supported distribution method, then check
          again.
        </div>
        {d.cli_install && <CopyLine text={d.cli_install} />}
        <div className="flex gap-2">
          <Button
            variant="outline"
            icon={<RefreshCw className="size-3.5" />}
            loading={checking}
            onClick={onRecheck}
          >
            Check again
          </Button>
          <Button
            variant="ghost"
            icon={<ExternalLink className="size-3.5" />}
            onClick={() => void openExternal(d.docs_url)}
          >
            Installation guide
          </Button>
        </div>
      </div>
    );
  }
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-xs text-fg-muted">
        <ShieldCheck className="size-4 text-fg" />
        <span>
          Found <span className="font-mono text-fg">{cli.binary}</span>{" "}
          {cli.version && <span className="font-mono">{cli.version}</span>}
        </span>
      </div>
      <div className="rounded-lg border border-border bg-bg-subtle p-3 text-xs leading-relaxed text-fg-muted">
        The CLI signs in directly with{" "}
        {d.vendor === "anthropic"
          ? "Anthropic"
          : d.vendor === "openai"
            ? "OpenAI"
            : "Google"}
        . Magpie never sees your password or tokens; it runs the official CLI on
        your behalf and respects your plan's usage limits.
      </div>
      <div>
        <div className="mb-1.5 text-xs font-medium text-fg-muted">
          If you are not signed in yet
        </div>
        {d.cli_login && <CopyLine text={d.cli_login} />}
        {isTauri && (
          <Button
            className="mt-2"
            variant="outline"
            icon={<Terminal className="size-3.5" />}
            onClick={() =>
              void openCliLogin(d.kind).catch((e) =>
                toast({
                  title: "Could not open a terminal",
                  description: errorMessage(e),
                  tone: "error",
                }),
              )
            }
          >
            Sign in using a terminal
          </Button>
        )}
      </div>
    </div>
  );
}

export function ConnectDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const { client } = useHarness();
  const qc = useQueryClient();
  const toast = useToast();
  const providers = useProviders().data;
  const clis = useClis();
  const [kind, setKind] = useState<ProviderKind | null>(null);
  const [label, setLabel] = useState("");
  const [key, setKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [baseUrl, setBaseUrl] = useState("");
  const [billing, setBilling] = useState<BillingMode>("unknown");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setKind(null);
      setKey("");
      setShowKey(false);
      setError(null);
    }
  }, [open]);

  const descriptors = useMemo(
    () => new Map((providers?.kinds ?? []).map((k) => [k.kind, k])),
    [providers],
  );
  const d = kind ? descriptors.get(kind) : undefined;
  const connectedKinds = new Set(
    (providers?.accounts ?? []).map((a) => a.kind),
  );
  const cliFor = (k: ProviderKind) => clis.data?.clis.find((c) => c.kind === k);

  const choose = (k: ProviderKind) => {
    const desc = descriptors.get(k);
    setKind(k);
    setLabel("");
    setKey("");
    setBaseUrl(desc?.default_base_url ?? "");
    setBilling(desc?.default_billing ?? "unknown");
    setError(null);
    if (desc?.auth_method === "cli_delegated") void clis.refetch();
  };

  const submit = async () => {
    if (!client || !d) return;
    setBusy(true);
    setError(null);
    try {
      const account = await client.connectProvider({
        kind: d.kind,
        label: label.trim() || undefined,
        api_key: key.trim() || undefined,
        base_url:
          baseUrl.trim() && baseUrl.trim() !== d.default_base_url
            ? baseUrl.trim()
            : d.kind === "open_ai_compatible"
              ? baseUrl.trim()
              : undefined,
        billing_mode: d.kind === "open_ai_compatible" ? billing : undefined,
      });
      toast({
        title: `${account.label} connected`,
        description: account.plan ? `${account.plan} plan` : undefined,
        tone: "success",
      });
      qc.invalidateQueries({ queryKey: ["providers"] });
      qc.invalidateQueries({ queryKey: ["models"] });
      onOpenChange(false);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const needsKey =
    d?.auth_method === "api_key" && d.kind !== "open_ai_compatible";
  const canSubmit =
    !!d &&
    (!needsKey || key.trim().length > 8) &&
    (d.kind !== "open_ai_compatible" || baseUrl.trim().length > 0) &&
    (d.auth_method !== "cli_delegated" || !!cliFor(d.kind)?.installed);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      width="w-[560px]"
      title={
        d ? (
          <span className="flex items-center gap-2.5">
            <button
              onClick={() => setKind(null)}
              className="-ml-1 rounded p-0.5 text-fg-subtle hover:bg-surface-2 hover:text-fg"
              aria-label="Back"
            >
              <ArrowLeft className="size-4" />
            </button>
            Connect {d.name}
          </span>
        ) : (
          "Connect a provider"
        )
      }
      description={
        d
          ? d.summary
          : "Use the AI accounts you already have. Credentials stay on this device."
      }
      footer={
        d ? (
          <>
            <Button variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="md"
              loading={busy}
              disabled={!canSubmit}
              onClick={() => void submit()}
            >
              {d.auth_method === "cli_delegated"
                ? "Verify and connect"
                : "Connect"}
            </Button>
          </>
        ) : undefined
      }
    >
      {!d ? (
        <div className="space-y-4">
          {GROUPS.map((g) => (
            <div key={g.title}>
              <div className="mb-1.5 flex items-baseline gap-2">
                <span className="text-xs font-medium">{g.title}</span>
                <span className="text-2xs text-fg-subtle">{g.note}</span>
              </div>
              <div className="grid grid-cols-2 gap-1.5">
                {g.kinds.map((k) => {
                  const desc = descriptors.get(k);
                  if (!desc) return null;
                  const taken = !desc.allow_multiple && connectedKinds.has(k);
                  const cli = cliFor(k);
                  return (
                    <button
                      key={k}
                      disabled={taken}
                      onClick={() => choose(k)}
                      className="flex items-center gap-2.5 rounded-lg border border-border px-2.5 py-2 text-left transition-colors hover:border-border-strong hover:bg-surface-2 disabled:opacity-40"
                    >
                      <ProviderMark kind={k} size={26} />
                      <div className="min-w-0">
                        <div className="truncate text-[12.5px] font-medium">
                          {desc.name}
                        </div>
                        <div className="truncate text-2xs text-fg-subtle">
                          {taken
                            ? "Connected"
                            : desc.auth_method === "cli_delegated"
                              ? cli
                                ? cli.installed
                                  ? "CLI detected"
                                  : "CLI not installed"
                                : "Checking…"
                              : desc.is_local
                                ? "Local server"
                                : "API key"}
                        </div>
                      </div>
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <ProviderMark kind={d.kind} size={34} />
            <div className="text-xs text-fg-muted">{d.summary}</div>
          </div>
          {d.auth_method === "cli_delegated" && (
            <CliSetup
              d={d}
              cli={cliFor(d.kind)}
              checking={clis.isFetching}
              onRecheck={() => void clis.refetch()}
            />
          )}
          {d.auth_method !== "cli_delegated" && (
            <div className="space-y-3">
              {d.auth_method === "api_key" && (
                <Field
                  label={
                    d.kind === "open_ai_compatible"
                      ? "API key (optional)"
                      : "API key"
                  }
                  hint={
                    d.key_url ? (
                      <button
                        className="inline-flex items-center gap-1 hover:text-fg"
                        onClick={() => void openExternal(d.key_url!)}
                      >
                        Create a key <ExternalLink className="size-3" />
                      </button>
                    ) : (
                      "Stored in your operating system's credential manager."
                    )
                  }
                >
                  <div className="relative">
                    <Input
                      mono
                      type={showKey ? "text" : "password"}
                      value={key}
                      onChange={(e) => setKey(e.target.value)}
                      placeholder="Paste key"
                      autoComplete="off"
                      spellCheck={false}
                      autoFocus
                    />
                    <button
                      type="button"
                      onClick={() => setShowKey((s) => !s)}
                      className="absolute top-1/2 right-2 -translate-y-1/2 text-fg-subtle hover:text-fg"
                      aria-label={showKey ? "Hide key" : "Show key"}
                    >
                      {showKey ? (
                        <EyeOff className="size-3.5" />
                      ) : (
                        <Eye className="size-3.5" />
                      )}
                    </button>
                  </div>
                </Field>
              )}
              <Field
                label="Base URL"
                hint={
                  d.is_local
                    ? "Start the server first; Magpie checks that it is reachable."
                    : d.kind === "open_ai_compatible"
                      ? "The endpoint's /v1 base, e.g. http://localhost:8000/v1"
                      : "Change only for proxies or regional endpoints."
                }
              >
                <Input
                  mono
                  value={baseUrl}
                  onChange={(e) => setBaseUrl(e.target.value)}
                  placeholder="https://…"
                  spellCheck={false}
                />
              </Field>
              {d.kind === "open_ai_compatible" && (
                <Field
                  label="Billing"
                  hint="Determines whether routing treats this endpoint as billable."
                >
                  <Select
                    label="Billing"
                    value={billing}
                    onChange={(v) => setBilling(v as BillingMode)}
                    options={[
                      { value: "local", label: "Free / self-hosted" },
                      { value: "metered", label: "Metered" },
                      {
                        value: "unknown",
                        label: "Unknown (treated as billable)",
                      },
                    ]}
                  />
                </Field>
              )}
              {d.default_billing === "metered" ||
              d.default_billing === "credits" ? (
                <p className="text-2xs text-fg-subtle">
                  Requests routed to this account are billed by the provider.
                  Automatic fallback from subscriptions to billable APIs stays
                  off unless you enable it in Routing.
                </p>
              ) : null}
            </div>
          )}
          <Field label="Name" hint="Shown throughout Magpie.">
            <Input
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder={d.name}
              maxLength={80}
            />
          </Field>
          {error && (
            <div
              className={cn(
                "rounded-md border border-border-strong bg-surface-2 px-3 py-2 text-xs text-fg",
              )}
            >
              {error}
            </div>
          )}
        </div>
      )}
    </Dialog>
  );
}
