import { useMemo, useState } from "react";
import {
  Cell,
  ResponsiveContainer,
  Scatter,
  ScatterChart,
  Tooltip as ChartTooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Panel, PanelHeader } from "./ui/core";
import { Segmented } from "./ui/controls";
import { QueryState } from "./QueryState";
import { useNow } from "./LimitWindows";
import { useTimeseries } from "@/lib/queries";
import { compact, integer } from "@/lib/format";
import { buildTokenActivity, type ActivityCell } from "@/lib/token-activity";

const shades = [
  "var(--surface-3)",
  "var(--fg-faint)",
  "var(--fg-subtle)",
  "var(--fg-muted)",
  "var(--fg)",
];
const monthFormat = new Intl.DateTimeFormat(undefined, { month: "short", timeZone: "UTC" });
function CellShape(props: {
  cx?: number;
  cy?: number;
  fill?: string;
  payload?: ActivityCell;
  size: number;
}) {
  return (
    <rect
      x={(props.cx ?? 0) - props.size / 2}
      y={(props.cy ?? 0) - props.size / 2}
      width={props.size}
      height={props.size}
      rx={3}
      fill={props.fill}
    />
  );
}
function ActivityTooltip({
  active,
  payload,
}: {
  active?: boolean;
  payload?: { payload: ActivityCell }[];
}) {
  if (!active || !payload?.[0]) return null;
  const p = payload[0].payload;
  return (
    <div className="rounded-md border border-border-strong bg-surface-2 px-3 py-2 text-xs shadow-panel">
      <p className="font-medium">{integer(p.value)} tokens</p>
      <p className="mt-1 text-fg-subtle">{p.label}</p>
      <p className="mt-1 text-2xs text-fg-subtle">
        {p.requests} requests on this day · UTC
      </p>
    </div>
  );
}

export function TokenActivity({
  provider,
  modelKey,
  onSelectRange,
}: {
  provider?: string;
  modelKey?: string;
  onSelectRange?: (from: number, to: number) => void;
}) {
  const [mode, setMode] = useState<"daily" | "weekly" | "cumulative">("daily");
  const anchor = useNow();
  const [chartWidth, setChartWidth] = useState(800);
  const end = Math.floor(anchor / 86_400_000) * 86_400_000;
  const start = end - 364 * 86_400_000;
  const query = useTimeseries({
    range: "custom",
    from: start,
    to: end + 86_400_000,
    bucket_ms: 86_400_000,
    provider,
    model_key: modelKey,
  });
  const cells = useMemo(
    () => buildTokenActivity(query.data?.points ?? [], start, end, mode),
    [query.data, start, end, mode],
  );
  const max = Math.max(1, ...cells.map((c) => c.value));
  const weeks = Math.max(1, ...cells.map((c) => c.week));
  // Keep square cells and the same 3px gutter on both axes at every width.
  const pitch = (chartWidth - 10) / (weeks + 1);
  const cellSize = Math.max(1, pitch - 3);
  const months = [
    ...new Map(
      cells
        .filter((c) => new Date(c.t).getUTCDate() <= 7)
        .map((c) => [
          c.week,
          monthFormat.format(c.t),
        ]),
    ).entries(),
  ].filter((_, i, all) => i === 0 || all[i - 1][1] !== all[i][1]);
  const total = (query.data?.points ?? []).reduce(
    (sum, p) => sum + p.input_tokens + p.output_tokens,
    0,
  );
  return (
    <Panel>
      <PanelHeader
        title="Token activity"
        actions={
          <Segmented
            label="Token activity aggregation"
            size="xs"
            value={mode}
            onChange={setMode}
            options={[
              { value: "daily", label: "Daily" },
              { value: "weekly", label: "Weekly" },
              { value: "cumulative", label: "Cumulative" },
            ]}
          />
        }
      />
      <QueryState
        pending={query.isLoading}
        error={query.error}
        retry={query.refetch}
      />
      {query.isSuccess && (
        <div className="px-4 pt-3 pb-4">
          <div className="overflow-x-auto">
            <div
              className="min-w-[800px]"
              role="img"
              aria-label={`Token activity over the last year: ${integer(total)} tokens. ${mode} view. Hover a cell for details or click a date to filter analytics.`}
            >
              <ResponsiveContainer
                width="100%"
                height={pitch * 7 + 32}
                onResize={(width) => setChartWidth(width)}
              >
                <ScatterChart margin={{ top: 5, right: 5, bottom: 5, left: 5 }}>
                  <XAxis
                    type="number"
                    dataKey="week"
                    domain={[-0.5, weeks + 0.5]}
                    ticks={months.map(([w]) => w)}
                    tickFormatter={(w: number) =>
                      months.find(([week]) => week === w)?.[1] ?? ""
                    }
                    tickLine={false}
                    axisLine={false}
                    interval={0}
                    height={22}
                    tickMargin={8}
                  />
                  <YAxis
                    type="number"
                    dataKey="day"
                    domain={[-0.5, 6.5]}
                    reversed
                    hide
                  />
                  <ChartTooltip
                    cursor={false}
                    content={<ActivityTooltip />}
                    isAnimationActive={false}
                  />
                  <Scatter
                    data={cells}
                    shape={<CellShape size={cellSize} />}
                    isAnimationActive={false}
                    onClick={(p) => {
                      const cell = p.payload as ActivityCell;
                      if (cell) onSelectRange?.(cell.t, cell.t + 86_400_000);
                    }}
                    cursor={onSelectRange ? "pointer" : "default"}
                  >
                    {cells.map((c) => (
                      <Cell
                        key={c.t}
                        fill={
                          shades[
                            c.value === 0
                              ? 0
                              : 1 + Math.min(3, Math.floor((c.value / max) * 3))
                          ]
                        }
                      />
                    ))}
                  </Scatter>
                </ScatterChart>
              </ResponsiveContainer>
            </div>
          </div>
          <div className="mt-2 flex items-center justify-between text-2xs text-fg-subtle">
            <span>
              {compact(total)} tokens in the last year · local harness usage
            </span>
            <span className="inline-flex items-center gap-1.5">
              Less{" "}
              {shades.map((fill) => (
                <span
                  key={fill}
                  className="size-2 rounded-[2px]"
                  style={{ background: fill }}
                />
              ))}{" "}
              More
            </span>
          </div>
          {total === 0 && (
            <p className="mt-2 text-2xs text-fg-subtle">
              No recorded token activity yet. Empty cells represent no recorded
              usage.
            </p>
          )}
          <details className="mt-2 text-2xs text-fg-subtle">
            <summary className="cursor-pointer">
              Accessible activity data
            </summary>
            <div className="mt-2 max-h-36 overflow-auto">
              {cells
                .filter((c) => c.requests > 0)
                .map((c) => (
                  <button
                    className="block w-full py-1 text-left hover:text-fg"
                    key={c.t}
                    onClick={() => onSelectRange?.(c.t, c.t + 86_400_000)}
                  >
                    {c.label}: {integer(c.value)} tokens
                  </button>
                ))}
              {!cells.some((c) => c.requests > 0) && "No activity recorded."}
            </div>
          </details>
        </div>
      )}
    </Panel>
  );
}
