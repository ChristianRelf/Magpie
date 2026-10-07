import { useEffect, useState } from "react";
import {
  Plus,
  RefreshCw,
  Plug,
  ShieldCheck,
  ExternalLink,
  Settings2,
} from "lucide-react";
import type { ProviderAccount } from "@magpie/sdk";
import { Page } from "@/components/Page";
import {
  Button,
  Badge,
  EmptyState,
  Field,
  Input,
  Panel,
  PanelHeader,
} from "@/components/ui/core";
import { Dialog, Drawer, SettingRow, Switch } from "@/components/ui/controls";
import { ConnectionStatusLabel, LimitStatus } from "@/components/ui/feedback";
import { ProviderMark } from "@/components/ui/marks";
import { LimitWindowRow, useNow } from "@/components/LimitWindows";
import { QueryState } from "@/components/QueryState";
import { useProviders } from "@/lib/queries";
import { useClient } from "@/lib/harness";
import { useNav } from "@/lib/nav";
import { useAction } from "@/lib/action";
import { BILLING_LABELS, dateTime, titleCase } from "@/lib/format";
import { openExternal } from "@/lib/desktop";
import { ConnectDialog } from "./ConnectDialog";

function AccountInspector({
  account,
  onClose,
}: {
  account: ProviderAccount;
  onClose: () => void;
}) {
  const client = useClient();
  const action = useAction();
  const now = useNow();
  const [label, setLabel] = useState(account.label);
  const [key, setKey] = useState("");
  const [remove, setRemove] = useState(false);
  const invalidations = ["providers", "models", "limits", "status"];
  return (
    <>
      <Drawer
        open
        onOpenChange={(open) => !open && onClose()}
        title={account.label}
      >
        <div className="space-y-5 p-5">
          <div className="flex items-center gap-3">
            <ProviderMark kind={account.kind} size={40} />
            <div>
              <div className="font-medium">{account.descriptor.name}</div>
              <ConnectionStatusLabel
                status={account.status}
                message={account.status_message}
              />
            </div>
          </div>
          {account.status_message && (
            <p className="rounded-md border border-border p-3 text-xs text-fg-muted">
              {account.status_message}
            </p>
          )}
          <Panel>
            <SettingRow
              title="Available for routing"
              description="Disabled accounts keep their credentials and history."
            >
              <Switch
                checked={account.enabled}
                disabled={action.busy}
                label="Enable account"
                onCheckedChange={(enabled) =>
                  void action.run(
                    () => client.updateProvider(account.id, { enabled }),
                    undefined,
                    invalidations,
                  )
                }
              />
            </SettingRow>
          </Panel>
          <Field label="Account name">
            <Input
              value={label}
              maxLength={80}
              onChange={(e) => setLabel(e.target.value)}
            />
          </Field>
          {account.auth_method === "api_key" && (
            <Field
              label="Replace API key"
              hint="Leave blank to keep the current key. Stored in the operating system credential manager."
            >
              <Input
                type="password"
                autoComplete="off"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                placeholder="New API key"
              />
            </Field>
          )}
          <Button
            loading={action.busy}
            disabled={!label.trim() || (label === account.label && !key.trim())}
            onClick={() =>
              void action.run(
                async () => {
                  await client.updateProvider(account.id, {
                    label: label.trim(),
                    api_key: key.trim() || undefined,
                  });
                  setKey("");
                },
                "Account updated",
                invalidations,
              )
            }
          >
            Save account
          </Button>
          <dl className="grid grid-cols-[140px_1fr] gap-x-4 gap-y-3 text-xs">
            <dt className="text-fg-subtle">Authentication</dt>
            <dd>{titleCase(account.auth_method)}</dd>
            <dt className="text-fg-subtle">Identity</dt>
            <dd>{account.identity ?? "Unavailable"}</dd>
            <dt className="text-fg-subtle">Billing</dt>
            <dd>
              {BILLING_LABELS[account.billing_mode]}
              {!account.billing_reported && " · configured"}
            </dd>
            <dt className="text-fg-subtle">Plan</dt>
            <dd>{account.plan ?? "Unavailable"}</dd>
            <dt className="text-fg-subtle">Last verified</dt>
            <dd>
              {account.last_verified_at
                ? dateTime(account.last_verified_at)
                : "Never"}
            </dd>
            <dt className="text-fg-subtle">Credential storage</dt>
            <dd>
              {account.auth_method === "cli_delegated"
                ? "Managed by official CLI"
                : account.has_secret
                  ? (account.secret_store ?? "OS credential manager")
                  : "No credential"}
            </dd>
            <dt className="text-fg-subtle">Endpoint</dt>
            <dd className="selectable break-all font-mono">
              {account.base_url ??
                account.descriptor.default_base_url ??
                "Official CLI"}
            </dd>
          </dl>
          <Panel>
            <PanelHeader title="Reported allowances" />
            <div className="space-y-4 p-4">
              {account.limits?.windows.length ? (
                account.limits.windows.map((w) => (
                  <LimitWindowRow key={w.key} w={w} now={now} />
                ))
              ) : (
                <p className="text-xs text-fg-subtle">
                  Unavailable. This provider has not reported an allowance or
                  reset time.
                </p>
              )}
            </div>
          </Panel>
          <p className="text-xs text-fg-subtle">
            Activity records cover requests made through Magpie. They do not
            represent total account usage.
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              loading={action.busy}
              icon={<ShieldCheck className="size-3.5" />}
              onClick={() =>
                void action.run(
                  () => client.verifyProvider(account.id),
                  "Verification complete",
                  invalidations,
                )
              }
            >
              Verify connection
            </Button>
            <Button
              loading={action.busy}
              icon={<RefreshCw className="size-3.5" />}
              onClick={() =>
                void action.run(
                  () => client.refreshProvider(account.id),
                  "Models refreshed",
                  invalidations,
                )
              }
            >
              Refresh models
            </Button>
          </div>
          <div className="flex items-center justify-between border-t border-border pt-4">
            <Button
              variant="ghost"
              icon={<ExternalLink className="size-3.5" />}
              onClick={() =>
                void action.run(() => openExternal(account.descriptor.docs_url))
              }
            >
              Provider documentation
            </Button>
            <Button variant="danger" onClick={() => setRemove(true)}>
              Disconnect
            </Button>
          </div>
        </div>
      </Drawer>
      <Dialog
        open={remove}
        onOpenChange={setRemove}
        title="Disconnect this account?"
        description="Magpie will delete its stored credential and stop routing new requests to this account. Your history is kept. CLI sign-in is managed separately by the provider."
        footer={
          <>
            <Button onClick={() => setRemove(false)}>Cancel</Button>
            <Button
              variant="danger"
              loading={action.busy}
              onClick={() =>
                void action.run(
                  async () => {
                    await client.deleteProvider(account.id);
                    onClose();
                  },
                  "Account disconnected",
                  invalidations,
                )
              }
            >
              Disconnect account
            </Button>
          </>
        }
      >
        <p className="text-sm">{account.label}</p>
      </Dialog>
    </>
  );
}

export function Providers() {
  const providers = useProviders();
  const { param, navigate } = useNav();
  const [connecting, setConnecting] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const now = useNow();
  useEffect(() => {
    if (param === "connect") {
      setConnecting(true);
      navigate("providers");
    }
  }, [param, navigate]);
  const accounts = providers.data?.accounts ?? [];
  const account = accounts.find((a) => a.id === selected);
  return (
    <Page
      title="Providers"
      subtitle={`${accounts.length} connected account${accounts.length === 1 ? "" : "s"}`}
      actions={
        <Button
          variant="primary"
          icon={<Plus className="size-3.5" />}
          onClick={() => setConnecting(true)}
        >
          Connect provider
        </Button>
      }
    >
      <QueryState
        pending={providers.isLoading}
        error={providers.error}
        retry={providers.refetch}
      />
      {providers.isSuccess && !accounts.length && (
        <EmptyState
          icon={<Plug />}
          title="Your accounts, one harness"
          description="Connect an official CLI, an API account, or a local model server. Magpie discovers its models and makes them available to your tools."
          action={
            <Button onClick={() => setConnecting(true)}>
              Connect your first provider
            </Button>
          }
        />
      )}
      <div className="grid gap-4 p-5 lg:grid-cols-2 xl:grid-cols-3">
        {accounts.map((a) => (
          <Panel key={a.id} className="flex flex-col">
            <div className="flex items-center gap-3 p-4">
              <ProviderMark kind={a.kind} size={34} />
              <div className="min-w-0 flex-1">
                <h2 className="truncate text-sm font-medium">{a.label}</h2>
                <p className="truncate text-xs text-fg-subtle">
                  {a.identity ?? a.descriptor.name}
                </p>
              </div>
              <Badge>{BILLING_LABELS[a.billing_mode]}</Badge>
            </div>
            <div className="flex items-center justify-between border-y border-border px-4 py-2.5">
              <ConnectionStatusLabel
                status={a.status}
                message={a.status_message}
              />
              <span className="text-xs text-fg-subtle">
                {a.available_model_count} / {a.model_count} models
              </span>
            </div>
            <div className="flex-1 space-y-4 p-4">
              {a.limits?.windows.length ? (
                a.limits.windows
                  .slice(0, 3)
                  .map((w) => <LimitWindowRow key={w.key} w={w} now={now} />)
              ) : (
                <div className="space-y-2">
                  <LimitStatus state="unknown" />
                  <p className="text-xs text-fg-subtle">
                    Allowance and reset data unavailable.
                  </p>
                </div>
              )}
            </div>
            <div className="flex items-center justify-between border-t border-border px-4 py-2.5">
              <span className="text-2xs text-fg-subtle">
                {titleCase(a.auth_method)}
              </span>
              <Button
                variant="ghost"
                size="xs"
                icon={<Settings2 className="size-3" />}
                onClick={() => setSelected(a.id)}
              >
                Manage
              </Button>
            </div>
          </Panel>
        ))}
      </div>
      <ConnectDialog open={connecting} onOpenChange={setConnecting} />
      {account && (
        <AccountInspector
          key={account.id}
          account={account}
          onClose={() => setSelected(null)}
        />
      )}
    </Page>
  );
}
