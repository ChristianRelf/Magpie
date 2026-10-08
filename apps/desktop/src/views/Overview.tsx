import { ArrowUpRight, ChartColumn, Clock3, Gauge, Plus, Plug, Settings2 } from "lucide-react";
import type { LimitWindow, ProviderAccount } from "@magpie/sdk";
import { Page } from "@/components/Page";
import { QueryState } from "@/components/QueryState";
import { LimitWindowRow, resetText, useNow } from "@/components/LimitWindows";
import { Badge, Button, EmptyState, IconButton, Panel, PanelHeader } from "@/components/ui/core";
import { ConnectionStatusLabel, ExecutionGlyph } from "@/components/ui/feedback";
import { ProviderMark } from "@/components/ui/marks";
import { useNav } from "@/lib/nav";
import { useActive, useExecutions, useLimits, useProviders } from "@/lib/queries";
import { BILLING_LABELS, ms, relative, TASK_LABELS } from "@/lib/format";

function PlanCard({ account, windows, now }: { account: ProviderAccount; windows: LimitWindow[]; now: number }) {
  const { navigate } = useNav();
  return (
    <Panel role="article" aria-label={`${account.label} plan`} className="flex flex-col">
      <div className="flex items-start gap-3.5 p-5">
        <ProviderMark kind={account.kind} size={40} />
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-base font-medium">{account.label}</h3>
          <p className="mt-1 text-sm text-fg-subtle">{account.plan ?? BILLING_LABELS[account.billing_mode]}</p>
        </div>
        <IconButton label={`Manage ${account.label}`} onClick={() => navigate("providers", account.id)}>
          <Settings2 className="size-4" />
        </IconButton>
      </div>
      <div className="flex-1 space-y-5 px-5 pt-1 pb-5">
        {windows.length ? (
          windows.map((window) => <LimitWindowRow key={window.key} w={window} now={now} />)
        ) : (
          <div className="flex min-h-24 items-center gap-3 rounded-lg bg-surface-2 px-4 py-3 text-sm text-fg-subtle">
            <Gauge className="size-5 shrink-0" />
            {account.billing_mode === "local" ? "No subscription allowance" : "Allowance not reported"}
          </div>
        )}
      </div>
      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-border px-5 py-3">
        <ConnectionStatusLabel
          status={account.enabled ? account.status : "disabled"}
          message={account.status_message}
        />
        <span className="text-xs text-fg-subtle">
          {account.available_model_count} {account.available_model_count === 1 ? "model" : "models"}
        </span>
      </div>
    </Panel>
  );
}

export function Overview() {
  const { navigate } = useNav();
  const now = useNow();
  const providers = useProviders();
  const limits = useLimits();
  const recent = useExecutions({ limit: 5 });
  const active = useActive().data?.data ?? [];
  const accounts = [...(providers.data?.accounts ?? [])].sort(
    (a, b) =>
      Number(b.enabled) - Number(a.enabled) ||
      Number(b.billing_mode === "subscription") - Number(a.billing_mode === "subscription"),
  );
  const windowsFor = (account: ProviderAccount) =>
    limits.data?.accounts.find((l) => l.account_id === account.id)?.windows ?? account.limits?.windows ?? [];
  const resets = accounts
    .filter((a) => a.enabled)
    .flatMap((account) => windowsFor(account).map((window) => ({ account, window })))
    .filter(({ window }) => window.resets_at && Date.parse(window.resets_at) > now)
    .sort((a, b) => Date.parse(a.window.resets_at!) - Date.parse(b.window.resets_at!))
    .slice(0, 5);
  const connected = accounts.filter((a) => a.enabled && a.status === "connected").length;

  return (
    <Page
      title="Overview"
      actions={
        <>
          <IconButton label="Open analytics" onClick={() => navigate("analytics")}>
            <ChartColumn className="size-4" />
          </IconButton>
          <Button variant="primary" icon={<Plus className="size-4" />} onClick={() => navigate("providers", "connect")}>
            Connect provider
          </Button>
        </>
      }
    >
      <QueryState pending={providers.isLoading} error={providers.error} retry={providers.refetch} />
      {providers.isSuccess && !accounts.length && (
        <EmptyState
          className="h-full"
          icon={<Plug />}
          title="Your plans, in one place"
          description="Connect a provider to see its plan, available allowance, and next reset."
          action={
            <Button variant="primary" size="md" onClick={() => navigate("providers", "connect")}>
              Connect your first provider
            </Button>
          }
        />
      )}
      {!!accounts.length && (
        <div className="mx-auto max-w-[1600px] space-y-7 p-6">
          <section aria-labelledby="current-plans" className="space-y-4">
            <div className="flex items-center gap-3">
              <h2 id="current-plans" className="text-lg font-medium tracking-tight">
                Current plans
              </h2>
              <Badge>{connected} connected</Badge>
            </div>
            {limits.isError && (
              <div role="alert" className="flex items-center gap-3 text-xs text-fg-muted">
                Could not refresh allowances.
                <Button size="xs" variant="ghost" onClick={() => void limits.refetch()}>
                  Try again
                </Button>
              </div>
            )}
            <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,360px),1fr))] gap-4">
              {accounts.map((account) => (
                <PlanCard key={account.id} account={account} windows={windowsFor(account)} now={now} />
              ))}
            </div>
          </section>

          <div className="grid items-start gap-4 xl:grid-cols-2">
            <Panel>
              <PanelHeader title="Upcoming resets" />
              {resets.length ? (
                <div className="divide-y divide-border">
                  {resets.map(({ account, window }) => (
                    <button
                      key={`${account.id}-${window.key}`}
                      className="flex w-full items-center gap-3 px-5 py-4 text-left transition-colors hover:bg-surface-2"
                      aria-label={`${account.label}, ${window.label}, ${resetText(window, now)}`}
                      onClick={() => navigate("providers", account.id)}
                    >
                      <Clock3 className="size-4 shrink-0 text-fg-subtle" />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm">{account.label}</p>
                        <p className="mt-0.5 truncate text-xs text-fg-subtle">{window.label}</p>
                      </div>
                      <span className="text-right text-xs text-fg-muted tnum">{resetText(window, now)}</span>
                    </button>
                  ))}
                </div>
              ) : (
                <div className="flex items-center gap-3 px-5 py-8 text-sm text-fg-subtle">
                  <Clock3 className="size-4 shrink-0" />
                  No reset times reported
                </div>
              )}
            </Panel>
            <Panel>
              <PanelHeader
                title={
                  <span className="flex items-center gap-2">
                    Recent activity {active.length > 0 && <Badge>{active.length} running</Badge>}
                  </span>
                }
                actions={
                  <IconButton label="View all activity" onClick={() => navigate("activity")}>
                    <ArrowUpRight className="size-4" />
                  </IconButton>
                }
              />
              <QueryState pending={recent.isLoading} error={recent.error} retry={recent.refetch} />
              {recent.isSuccess && !recent.data.data.length && (
                <p className="px-5 py-8 text-sm text-fg-subtle">Your requests will appear here.</p>
              )}
              <div className="divide-y divide-border">
                {(recent.data?.data ?? []).map((execution) => (
                  <button
                    key={execution.id}
                    className="flex w-full items-center gap-3 px-5 py-4 text-left transition-colors hover:bg-surface-2"
                    onClick={() => navigate("activity", execution.id)}
                  >
                    <ExecutionGlyph status={execution.status} />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm">{execution.model?.display_name ?? "Not routed"}</p>
                      <p className="mt-0.5 truncate text-xs text-fg-subtle">
                        {TASK_LABELS[execution.task]} · {relative(execution.created_at, now)}
                      </p>
                    </div>
                    <span className="text-xs text-fg-muted tnum">{ms(execution.duration_ms)}</span>
                  </button>
                ))}
              </div>
            </Panel>
          </div>
        </div>
      )}
    </Page>
  );
}
