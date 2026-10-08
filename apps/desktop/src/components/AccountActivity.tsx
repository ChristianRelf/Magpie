import { useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { useProviders, useProviderReports } from "@/lib/queries";
import { useClient } from "@/lib/harness";
import { useAction } from "@/lib/action";
import { accountPoints, accountRange, accountCsv, DAY } from "@/lib/account-activity";
import { compact, relative } from "@/lib/format";
import { saveTextFile } from "@/lib/desktop";
import { Button, Panel, PanelHeader } from "./ui/core";
import { Select } from "./ui/controls";
import { StatTile } from "./Page";
import { QueryState } from "./QueryState";
import { TokenActivity } from "./TokenActivity";
import { TimeSeriesChart, type Series } from "./charts";
import { useNow } from "./LimitWindows";
import type { RangeValue } from "./RangePicker";

const KEY = "magpie.usage-source";
const TOKEN_SERIES: Series[] = [{ key: "tokens", label: "Reported tokens", color: "var(--series-1)", kind: "bar" }];

export function useUsageSource() {
  const providers = useProviders();
  const reports = useProviderReports();
  const accounts = (providers.data?.accounts ?? []).filter((a) => a.kind === "codex_cli" && a.enabled);
  const [selected, setSelected] = useState(() => {
    try {
      return localStorage.getItem(KEY) ?? "auto";
    } catch {
      return "auto";
    }
  });
  const account = selected === "harness" ? undefined : (accounts.find((a) => a.id === selected) ?? accounts[0]);
  const value = account?.id ?? "harness";
  return {
    value,
    account,
    reports,
    report: reports.data?.reports.find((r) => r.account_id === account?.id),
    options: [
      ...accounts.map((a) => ({ value: a.id, label: `${a.label} account` })),
      { value: "harness", label: "Magpie requests" },
    ],
    select: (next: string) => {
      setSelected(next);
      try {
        localStorage.setItem(KEY, next);
      } catch {
        /* Private browsing. */
      }
    },
  };
}

export function UsageSourcePicker({ source }: { source: ReturnType<typeof useUsageSource> }) {
  return (
    <Select
      label="Usage source"
      className="w-48"
      value={source.value}
      onChange={source.select}
      options={source.options}
    />
  );
}

/** Account reports are deliberately separate from execution analytics: adding
 * them to Magpie totals would count the same provider requests twice. */
export function AccountActivity({
  source,
  range,
  onSelectRange,
}: {
  source: ReturnType<typeof useUsageSource>;
  range: RangeValue;
  onSelectRange: (from: number, to: number) => void;
}) {
  const client = useClient();
  const qc = useQueryClient();
  const action = useAction();
  const now = useNow();
  const { from, to } = accountRange(range, now);
  const points = accountPoints(source.report);
  const selected = points.filter((p) => p.t >= from && p.t < to);
  const total = selected.reduce((sum, p) => sum + p.tokens, 0);
  const today = points.find((p) => p.t === Math.floor(now / DAY) * DAY);
  const byDay = new Map(selected.map((p) => [p.t, p.tokens]));
  // Explicit nulls leave gaps instead of drawing invented zeros between reports.
  const plotted = Array.from({ length: Math.min(3660, Math.max(0, Math.ceil((to - from) / DAY))) }, (_, i) => {
    const t = from + i * DAY;
    return { t, tokens: byDay.get(t) ?? null };
  });
  return (
    <>
      <div className="flex flex-wrap items-center justify-between gap-2 text-2xs text-fg-subtle">
        <span>
          {source.account?.label} account · provider-reported ·{" "}
          {source.report ? `updated ${relative(source.report.observed_at, now)}` : "waiting for a report"}
        </span>
        <div className="flex gap-2">
          <Button
            size="xs"
            icon={<Download className="size-3" />}
            disabled={selected.length === 0}
            onClick={() => void action.run(() => saveTextFile("magpie-account-usage.csv", accountCsv(selected)))}
          >
            Export account CSV
          </Button>
          <Button
            size="xs"
            icon={<RefreshCw className="size-3" />}
            loading={action.busy}
            onClick={() =>
              void action.run(async () => {
                await client.refreshProvider(source.account!.id);
                const data = await client.providerReports();
                qc.setQueryData(["reports"], data);
                const updated = data.reports.find((r) => r.account_id === source.account!.id);
                if (!updated || updated.observed_at === source.report?.observed_at) {
                  throw new Error(
                    "Codex did not return a new usage report. Check its sign-in and CLI version, then try again.",
                  );
                }
              })
            }
          >
            Refresh usage
          </Button>
        </div>
      </div>
      <QueryState pending={source.reports.isLoading} error={source.reports.error} retry={source.reports.refetch} />
      <TokenActivity accountScope accountReport={source.report} onSelectRange={onSelectRange} />
      <Panel className="grid grid-cols-2 divide-border lg:grid-cols-4 lg:divide-x">
        <StatTile
          label="Reported in range"
          value={selected.length ? compact(total) : "Unavailable"}
          sub={`${selected.length} reported days · whole UTC days`}
        />
        <StatTile
          label="Today"
          value={today ? compact(today.tokens) : "Unavailable"}
          sub="UTC · latest reported total"
        />
        <StatTile
          label="Lifetime tokens"
          value={source.report?.lifetime_tokens == null ? "Unavailable" : compact(source.report.lifetime_tokens)}
          sub="Provider account total"
        />
        <StatTile
          label="Peak daily tokens"
          value={source.report?.peak_daily_tokens == null ? "Unavailable" : compact(source.report.peak_daily_tokens)}
          sub="Provider account record"
        />
      </Panel>
      <Panel>
        <PanelHeader
          title="Account token usage"
          subtitle="Daily totals include Codex usage outside Magpie · drag to select dates"
        />
        <div className="px-3 pt-3 pb-2">
          <TimeSeriesChart
            data={plotted}
            series={TOKEN_SERIES}
            bucketMs={DAY}
            height={240}
            utc
            onSelectRange={onSelectRange}
          />
        </div>
        <p className="border-t border-border px-4 py-3 text-2xs text-fg-subtle">
          {source.report
            ? "Codex reports daily account totals, which may update after a session finishes. Missing days are unavailable, not zero."
            : "No account usage report is available yet. Refresh after connecting Codex; older CLI versions or API-key authentication may not expose account history."}{" "}
          Magpie checks for account updates every two minutes when provider monitoring is enabled. Request counts,
          models, latency and costs are available under Magpie requests for executions routed through the harness.
        </p>
      </Panel>
    </>
  );
}
