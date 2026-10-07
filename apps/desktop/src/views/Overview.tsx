import { useMemo, useState } from "react";
import { ArrowUpRight, Plug } from "lucide-react";
import type { LimitState, LimitWindow } from "@magpie/sdk";
import { TokenActivity } from "@/components/TokenActivity";
import { Page, StatTile, Delta } from "@/components/Page";
import {
  RangePicker,
  toQuery,
  type RangeValue,
  rangeLabel,
} from "@/components/RangePicker";
import {
  BarList,
  Legend,
  Sparkline,
  TimeSeriesChart,
  type Series,
} from "@/components/charts";
import {
  LimitWindowRow,
  resetText,
  useNow,
  windowState,
} from "@/components/LimitWindows";
import {
  Button,
  EmptyState,
  Mono,
  Panel,
  PanelHeader,
  Skeleton,
} from "@/components/ui/core";
import {
  ConnectionGlyph,
  ExecutionGlyph,
  LimitGlyph,
} from "@/components/ui/feedback";
import { ProviderMark } from "@/components/ui/marks";
import { useNav } from "@/lib/nav";
import {
  useBreakdown,
  useExecutions,
  useLimits,
  useProviders,
  useStatus,
  useTimeseries,
  useUsageSummary,
  useActive,
} from "@/lib/queries";
import {
  compact,
  duration,
  ms,
  providerName,
  relative,
  TASK_LABELS,
  LIMIT_STATE_LABELS,
  BILLING_LABELS,
} from "@/lib/format";

const TOKEN_SERIES: Series[] = [
  {
    key: "input_tokens",
    label: "Input tokens",
    color: "var(--series-2)",
    kind: "bar",
  },
  {
    key: "output_tokens",
    label: "Output tokens",
    color: "var(--series-1)",
    kind: "bar",
  },
];

const STATE_RANK: Record<LimitState, number> = {
  unknown: 0,
  available: 1,
  reset_pending: 2,
  approaching: 3,
  limited: 4,
  exhausted: 5,
};

export function Overview() {
  const [range, setRange] = useState<RangeValue>({ kind: "24h" });
  const q = toQuery(range);
  const { navigate } = useNav();
  const now = useNow();
  const status = useStatus().data;
  const providers = useProviders();
  const limits = useLimits().data?.accounts ?? [];
  const summary = useUsageSummary(q);
  const series = useTimeseries(q);
  const byProvider = useBreakdown({ ...q, group_by: "provider" });
  const recent = useExecutions({ limit: 8 });
  const active = useActive().data?.data ?? [];

  const accounts = providers.data?.accounts ?? [];
  const cur = summary.data?.current;
  const prev = summary.data?.previous;
  const points = series.data?.points ?? [];
  const spark = useMemo(() => {
    const p = points.map((x) => x.input_tokens + x.output_tokens);
    const n = 16;
    if (p.length <= n) return p;
    const size = Math.ceil(p.length / n);
    const out: number[] = [];
    for (let i = 0; i < p.length; i += size)
      out.push(p.slice(i, i + size).reduce((a, b) => a + b, 0));
    return out;
  }, [points]);

  const allWindows: LimitWindow[] = limits.flatMap((l) => l.windows);
  const worst = allWindows.reduce<LimitState>((acc, w) => {
    const s = windowState(w, now);
    return STATE_RANK[s] > STATE_RANK[acc] ? s : acc;
  }, "unknown");
  const constrained = allWindows.filter((w) =>
    ["approaching", "limited", "exhausted"].includes(windowState(w, now)),
  ).length;
  const healthText =
    allWindows.length === 0
      ? "No limits reported"
      : constrained === 0
        ? "All reported allowances available"
        : `${constrained} window${constrained > 1 ? "s" : ""} constrained`;

  const resets = allWindows
    .filter((w) => w.resets_at && Date.parse(w.resets_at) > now)
    .sort((a, b) => Date.parse(a.resets_at!) - Date.parse(b.resets_at!))
    .slice(0, 6);
  const labelOf = (id: string) =>
    accounts.find((a) => a.id === id)?.label ?? id;

  if (providers.isSuccess && accounts.length === 0) {
    return (
      <Page title="Overview">
        <EmptyState
          className="h-full"
          icon={<Plug />}
          title="Connect your first provider"
          description="Magpie routes requests across the AI accounts you already have. Connect Claude Code, Codex, an API key or a local model server to begin."
          action={
            <Button
              variant="primary"
              size="md"
              onClick={() => navigate("providers", "connect")}
            >
              Connect a provider
            </Button>
          }
        />
      </Page>
    );
  }

  const totalTokens = cur ? cur.input_tokens + cur.output_tokens : 0;
  const prevTokens = prev ? prev.input_tokens + prev.output_tokens : 0;

  return (
    <Page
      title="Overview"
      subtitle={rangeLabel(range)}
      actions={<RangePicker value={range} onChange={setRange} />}
    >
      <div className="space-y-4 p-5">
        <TokenActivity
          onSelectRange={(from, to) => setRange({ kind: "custom", from, to })}
        />
        <Panel className="grid grid-cols-2 divide-border md:grid-cols-5 md:divide-x">
          <StatTile
            label="Harness"
            value={<span className="text-[18px]">Running</span>}
            sub={
              status
                ? `Up ${duration(status.uptime_secs)} · ${status.available_models} models`
                : " "
            }
            help="The local harness service. It keeps running independently of this window when background operation is enabled."
          />
          <StatTile
            label="Providers"
            value={
              <span>
                {status?.connected_accounts ?? "–"}
                <span className="text-fg-subtle">
                  /{status?.accounts ?? "–"}
                </span>
              </span>
            }
            sub="connected"
          />
          <StatTile
            label="Active"
            value={active.length}
            sub={
              active.length
                ? active
                    .map((a) => a.model?.display_name)
                    .filter(Boolean)
                    .slice(0, 2)
                    .join(", ")
                : "No executions running"
            }
          />
          <StatTile
            label="Tokens"
            value={
              cur ? compact(totalTokens) : <Skeleton className="h-6 w-16" />
            }
            sub={
              cur && prev ? (
                <Delta current={totalTokens} previous={prevTokens} upIsGood />
              ) : (
                "Through the harness"
              )
            }
            help="Input and output tokens through Magpie in the selected range. Account usage outside Magpie is not included."
            trend={<Sparkline values={spark} />}
          />
          <StatTile
            label="Usage health"
            value={
              <span className="flex items-center gap-2 text-[18px]">
                <LimitGlyph state={worst} className="size-4" />
                {LIMIT_STATE_LABELS[worst]}
              </span>
            }
            sub={healthText}
            help="The most constrained provider limit window. Unknown means no provider reports its allowance."
          />
        </Panel>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
          <Panel className="xl:col-span-2">
            <PanelHeader
              title="Token usage"
              subtitle={
                cur
                  ? `${compact(cur.input_tokens)} in · ${compact(cur.output_tokens)} out · ${cur.requests} requests · drag to zoom`
                  : undefined
              }
              actions={<Legend series={TOKEN_SERIES} />}
            />
            <div className="px-2 pt-3 pb-2">
              <TimeSeriesChart
                data={points as never}
                series={TOKEN_SERIES}
                stacked
                bucketMs={series.data?.bucket_ms ?? 60_000}
                height={236}
                dimmed={series.isFetching && series.isPlaceholderData}
                onSelectRange={(from, to) =>
                  setRange({ kind: "custom", from, to })
                }
              />
            </div>
          </Panel>
          <div className="flex flex-col gap-4">
            <Panel>
              <PanelHeader
                title="Provider utilisation"
                subtitle="Share of tokens through the harness"
              />
              <div className="p-3.5">
                <BarList
                  items={(byProvider.data?.rows ?? []).map((r) => ({
                    key: r.key,
                    label: providerName(r.key),
                    value: r.input_tokens + r.output_tokens,
                    secondary: `${r.requests} req`,
                    detail: `${compact(r.input_tokens)} input · ${compact(r.output_tokens)} output · ${r.failed} failed`,
                  }))}
                  empty="No executions in this range."
                />
              </div>
            </Panel>
            <Panel className="flex-1">
              <PanelHeader
                title="Limits"
                subtitle="Most constrained window per provider"
                actions={
                  <Button
                    size="xs"
                    variant="ghost"
                    onClick={() => navigate("providers")}
                  >
                    Details
                  </Button>
                }
              />
              <div className="space-y-3.5 p-3.5">
                {limits.length === 0 && (
                  <p className="text-xs text-fg-subtle">
                    No providers connected.
                  </p>
                )}
                {limits.map((l) => {
                  const w = [...l.windows].sort(
                    (a, b) =>
                      STATE_RANK[windowState(b, now)] -
                        STATE_RANK[windowState(a, now)] ||
                      (b.used_percent ?? 0) - (a.used_percent ?? 0),
                  )[0];
                  return (
                    <div key={l.account_id}>
                      <div className="mb-1 text-2xs text-fg-subtle">
                        {labelOf(l.account_id)}
                      </div>
                      {w ? (
                        <LimitWindowRow w={w} now={now} />
                      ) : (
                        <p className="text-xs text-fg-subtle">
                          Allowance not reported by provider
                        </p>
                      )}
                    </div>
                  );
                })}
              </div>
            </Panel>
          </div>
        </div>

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
          <Panel>
            <PanelHeader
              title="Connected providers"
              actions={
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() => navigate("providers")}
                >
                  Manage
                </Button>
              }
            />
            <div className="divide-y divide-border">
              {accounts.map((a) => (
                <button
                  key={a.id}
                  onClick={() => navigate("providers")}
                  className="flex w-full items-center gap-3 px-3.5 py-2.5 text-left hover:bg-surface-2"
                >
                  <ProviderMark kind={a.kind} size={26} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5 text-[12.5px] font-medium">
                      <span className="truncate">{a.label}</span>
                      <ConnectionGlyph
                        status={a.status}
                        className="size-3 text-fg-subtle"
                      />
                    </div>
                    <div className="truncate text-2xs text-fg-subtle">
                      {a.plan ? `${a.plan} · ` : ""}
                      {BILLING_LABELS[a.billing_mode]} ·{" "}
                      {a.available_model_count} models
                    </div>
                  </div>
                  <LimitGlyph
                    state={a.limits?.state ?? "unknown"}
                    className="text-fg-muted"
                  />
                </button>
              ))}
            </div>
          </Panel>
          <Panel>
            <PanelHeader
              title="Recent executions"
              actions={
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() => navigate("activity")}
                >
                  View all
                </Button>
              }
            />
            <div className="divide-y divide-border">
              {(recent.data?.data ?? []).length === 0 && (
                <p className="px-3.5 py-6 text-center text-xs text-fg-subtle">
                  No executions yet. Send a request to the harness to see it
                  here.
                </p>
              )}
              {(recent.data?.data ?? []).map((e) => (
                <button
                  key={e.id}
                  onClick={() => navigate("activity", e.id)}
                  className="flex w-full items-center gap-2.5 px-3.5 py-2 text-left hover:bg-surface-2"
                >
                  <ExecutionGlyph status={e.status} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[12.5px]">
                      {e.model?.display_name ?? "Not routed"}
                    </div>
                    <div className="truncate text-2xs text-fg-subtle">
                      {TASK_LABELS[e.task]} · {relative(e.created_at, now)}
                    </div>
                  </div>
                  <div className="text-right">
                    <Mono className="text-[11px] text-fg-muted">
                      {compact(
                        (e.usage.input_tokens ?? 0) +
                          (e.usage.output_tokens ?? 0),
                      )}
                    </Mono>
                    <div className="font-mono text-2xs text-fg-subtle">
                      {ms(e.duration_ms)}
                    </div>
                  </div>
                </button>
              ))}
            </div>
          </Panel>
          <Panel>
            <PanelHeader
              title="Upcoming resets"
              subtitle="Known provider reset times"
            />
            <div className="divide-y divide-border">
              {resets.length === 0 && (
                <p className="px-3.5 py-6 text-center text-xs text-fg-subtle">
                  No reset times reported.
                </p>
              )}
              {resets.map((w) => (
                <div
                  key={`${w.account_id}-${w.key}`}
                  className="flex items-center gap-2.5 px-3.5 py-2"
                >
                  <LimitGlyph
                    state={windowState(w, now)}
                    className="text-fg-muted"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[12.5px]">{w.label}</div>
                    <div className="truncate text-2xs text-fg-subtle">
                      {labelOf(w.account_id)}
                    </div>
                  </div>
                  <div className="text-right text-2xs text-fg-muted">
                    <div>{resetText(w, now)}</div>
                    <div className="font-mono text-fg-subtle">
                      {new Date(w.resets_at!).toLocaleString([], {
                        weekday: "short",
                        hour: "2-digit",
                        minute: "2-digit",
                      })}
                    </div>
                  </div>
                </div>
              ))}
            </div>
            <div className="border-t border-border px-3.5 py-2">
              <button
                className="inline-flex items-center gap-1 text-2xs text-fg-subtle hover:text-fg"
                onClick={() => navigate("analytics")}
              >
                Open analytics <ArrowUpRight className="size-3" />
              </button>
            </div>
          </Panel>
        </div>
      </div>
    </Page>
  );
}
