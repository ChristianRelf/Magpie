import { useState } from "react";
import { Download } from "lucide-react";
import type { GroupBy } from "@magpie/sdk";
import { TokenActivity } from "@/components/TokenActivity";
import { AccountActivity, UsageSourcePicker, useUsageSource } from "@/components/AccountActivity";
import { Page, StatTile } from "@/components/Page";
import { RangePicker, rangeLabel, toQuery, type RangeValue } from "@/components/RangePicker";
import { BarList, Legend, TimeSeriesChart, type Series } from "@/components/charts";
import { Button, Panel, PanelHeader } from "@/components/ui/core";
import { Select, Segmented } from "@/components/ui/controls";
import { QueryState } from "@/components/QueryState";
import { useBreakdown, useModels, useProviders, useTimeseries, useUsageSummary } from "@/lib/queries";
import { useClient } from "@/lib/harness";
import { useAction } from "@/lib/action";
import { compact, ms, percent, providerName, usd } from "@/lib/format";
import { saveTextFile } from "@/lib/desktop";

const tokenSeries: Series[] = [
  {
    key: "input_tokens",
    label: "Input",
    color: "var(--series-2)",
    kind: "bar",
  },
  {
    key: "output_tokens",
    label: "Output",
    color: "var(--series-1)",
    kind: "bar",
  },
];
export function Analytics() {
  const source = useUsageSource();
  const [range, setRange] = useState<RangeValue>({ kind: "7d" });
  const [provider, setProvider] = useState("all");
  const [model, setModel] = useState("all");
  const [metric, setMetric] = useState("tokens");
  const [group, setGroup] = useState<GroupBy>("provider");
  const q = {
    ...toQuery(range),
    provider: provider === "all" ? undefined : provider,
    model_key: model === "all" ? undefined : model,
  };
  const summary = useUsageSummary(q);
  const series = useTimeseries(q);
  const breakdown = useBreakdown({ ...q, group_by: group });
  const providers = useProviders();
  const models = useModels();
  const client = useClient();
  const action = useAction();
  const current = summary.data?.current;
  const selectRange = (from: number, to: number) => setRange({ kind: "custom", from, to });
  const plotted =
    metric === "tokens"
      ? tokenSeries
      : [
          {
            key: metric,
            label: metric === "requests" ? "Requests" : metric === "cost_usd" ? "Cost (mixed provenance)" : "Latency",
            color: "var(--series-1)",
            kind: "line" as const,
            format: metric === "cost_usd" ? usd : metric === "avg_duration_ms" ? ms : compact,
          },
        ];
  if (source.account)
    return (
      <Page
        title="Analytics"
        subtitle="Provider-reported account usage"
        actions={
          <>
            <UsageSourcePicker source={source} />
            <RangePicker value={range} onChange={setRange} daily />
          </>
        }
      >
        <div className="space-y-4 p-5">
          <AccountActivity source={source} range={range} onSelectRange={selectRange} />
        </div>
      </Page>
    );
  return (
    <Page
      title="Analytics"
      subtitle={rangeLabel(range)}
      actions={
        <>
          <UsageSourcePicker source={source} />
          <RangePicker value={range} onChange={setRange} />
          <Button
            icon={<Download className="size-3.5" />}
            loading={action.busy}
            onClick={() =>
              void action.run(async () => {
                const data = await client.exportUsage({ ...q, format: "csv" });
                await saveTextFile("magpie-usage.csv", data);
              })
            }
          >
            Export CSV
          </Button>
        </>
      }
      toolbar={
        <>
          <Select
            label="Analytics provider"
            className="w-44"
            value={provider}
            onChange={(p) => {
              setProvider(p);
              setModel("all");
            }}
            options={[
              { value: "all", label: "All providers" },
              ...[...new Set(providers.data?.accounts.map((a) => a.kind) ?? [])].map((p) => ({
                value: p,
                label: providerName(p),
              })),
            ]}
          />
          <Select
            label="Analytics model"
            className="w-60"
            value={model}
            onChange={setModel}
            options={[
              { value: "all", label: "All models" },
              ...(models.data ?? [])
                .filter((m) => provider === "all" || m.provider === provider)
                .map((m) => ({
                  value: m.key,
                  label: `${m.display_name} · ${m.account_label}`,
                })),
            ]}
          />
          <span className="ml-auto text-2xs text-fg-subtle">Local harness usage</span>
        </>
      }
    >
      <QueryState
        pending={summary.isLoading}
        error={summary.error || series.error || breakdown.error}
        retry={() => {
          void summary.refetch();
          void series.refetch();
          void breakdown.refetch();
        }}
      />
      <div className="space-y-4 p-5">
        <TokenActivity provider={q.provider} modelKey={q.model_key} onSelectRange={selectRange} />
        <Panel className="grid grid-cols-2 divide-border lg:grid-cols-4 lg:divide-x">
          <StatTile
            label="Tokens"
            value={compact(current ? current.input_tokens + current.output_tokens : undefined)}
            sub={
              current
                ? `${current.tokens_reported_requests} reported · ${current.tokens_estimated_requests} estimated requests`
                : ""
            }
          />
          <StatTile
            label="Requests"
            value={compact(current?.requests)}
            sub={current ? `${current.failed} failed · ${current.cancelled} cancelled` : ""}
          />
          <StatTile
            label="Success rate"
            value={percent(
              current && current.succeeded + current.failed + current.cancelled > 0
                ? current.succeeded / (current.succeeded + current.failed + current.cancelled)
                : null,
            )}
            sub="Calculated from completed requests"
          />
          <StatTile
            label="Average latency"
            value={ms(current?.avg_duration_ms)}
            sub={`p95 ${ms(current?.p95_duration_ms)} · locally measured`}
          />
        </Panel>
        <Panel>
          <PanelHeader
            title="Usage over time"
            subtitle="Drag across the graph to inspect a time range"
            actions={
              <Segmented
                label="Graph metric"
                value={metric}
                onChange={setMetric}
                options={[
                  { value: "tokens", label: "Tokens" },
                  { value: "requests", label: "Requests" },
                  { value: "cost_usd", label: "Cost" },
                  { value: "avg_duration_ms", label: "Latency" },
                ]}
              />
            }
          />
          <div className="p-4">
            <Legend series={plotted} />
            <TimeSeriesChart
              data={(series.data?.points ?? []).map((p) => ({
                ...p,
                group: undefined,
              }))}
              series={plotted}
              height={260}
              bucketMs={series.data?.bucket_ms ?? 3_600_000}
              stacked={metric === "tokens"}
              onSelectRange={selectRange}
              yFormat={metric === "cost_usd" ? usd : metric === "avg_duration_ms" ? ms : compact}
              dimmed={series.isFetching}
            />
          </div>
        </Panel>
        <div className="grid gap-4 lg:grid-cols-2">
          <Panel>
            <PanelHeader
              title="Token distribution"
              actions={
                <Select
                  size="sm"
                  label="Group usage"
                  value={group}
                  onChange={(g) => setGroup(g as GroupBy)}
                  options={[
                    { value: "provider", label: "Provider" },
                    { value: "model", label: "Model" },
                    { value: "account", label: "Account" },
                    { value: "task", label: "Task" },
                  ]}
                />
              }
            />
            <div className="p-4">
              <BarList
                items={(breakdown.data?.rows ?? []).map((r) => ({
                  key: r.key,
                  label: group === "provider" ? providerName(r.key) : r.label,
                  value: r.input_tokens + r.output_tokens,
                  secondary: `${r.requests} requests`,
                  detail: `${r.failed} failed · ${ms(r.avg_duration_ms)} average · ${usd(r.cost_usd)}`,
                }))}
              />
            </div>
          </Panel>
          <Panel>
            <PanelHeader
              title="Cost accounting"
              subtitle="USD · keep reported, calculated and estimated figures separate"
            />
            <div className="grid grid-cols-2 gap-y-2 py-2">
              <StatTile label="Provider-reported" value={usd(current?.cost_reported_usd)} />
              <StatTile label="Calculated" value={usd(current?.cost_calculated_usd)} />
              <StatTile label="Estimated" value={usd(current?.cost_estimated_usd)} />
              <StatTile
                label="Subscription equivalent"
                value={usd(current?.api_equivalent_usd)}
                sub="Comparison only; not a bill"
              />
            </div>
            <p className="border-t border-border px-4 py-3 text-2xs text-fg-subtle">
              Missing prices are excluded. Costs cover Magpie executions only; this is not your provider invoice.
            </p>
          </Panel>
        </div>
        <Panel className="grid grid-cols-2 divide-border lg:grid-cols-4 lg:divide-x">
          <StatTile label="Cached input tokens" value={compact(current?.cached_tokens)} />
          <StatTile label="Reasoning tokens" value={compact(current?.reasoning_tokens)} />
          <StatTile label="Requests per minute" value={current?.requests_per_minute.toFixed(2) ?? "—"} />
          <StatTile label="Fallbacks" value={compact(current?.fallbacks)} />
        </Panel>
        <Panel>
          <PanelHeader title="Request outcomes" subtitle="Calculated locally" />
          <div className="p-4">
            <TimeSeriesChart
              data={(series.data?.points ?? []).map((p) => ({
                t: p.t,
                requests: p.requests,
                failed: p.failed,
              }))}
              series={[
                {
                  key: "requests",
                  label: "Requests",
                  kind: "bar",
                  color: "var(--series-2)",
                },
                {
                  key: "failed",
                  label: "Failures",
                  kind: "line",
                  color: "var(--series-1)",
                },
              ]}
              bucketMs={series.data?.bucket_ms ?? 3_600_000}
              height={160}
              onSelectRange={selectRange}
            />
          </div>
        </Panel>
      </div>
    </Page>
  );
}
