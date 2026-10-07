import { useMemo, useState, type ReactNode } from "react";
import {
  Area,
  Bar,
  CartesianGrid,
  ComposedChart,
  ReferenceArea,
  ResponsiveContainer,
  Tooltip as RTooltip,
  XAxis,
  YAxis,
} from "recharts";
import { compact, shortDate } from "@/lib/format";
import { cn, Tooltip } from "./ui/core";

export interface Series {
  key: string;
  label: string;
  /** CSS colour, normally one of the validated series tokens. */
  color: string;
  kind: "bar" | "line";
  format?: (v: number) => string;
}

type Datum = { t: number } & Record<string, number | null | undefined>;

function ChartTooltip({
  active,
  payload,
  label,
  series,
  bucketMs,
}: {
  active?: boolean;
  payload?: { dataKey: string; value: number }[];
  label?: number;
  series: Series[];
  bucketMs: number;
}) {
  if (!active || !payload?.length || label === undefined) return null;
  return (
    <div className="min-w-36 rounded-md border border-border-strong bg-surface-2 px-2.5 py-2 text-xs shadow-panel">
      <div className="mb-1.5 text-2xs text-fg-subtle">
        {shortDate(label, bucketMs)}
      </div>
      <div className="space-y-1">
        {series.map((s) => {
          const p = payload.find((x) => x.dataKey === s.key);
          const v = p?.value;
          return (
            <div key={s.key} className="flex items-center gap-2">
              <span
                className="h-0.5 w-3 shrink-0 rounded-full"
                style={{ background: s.color }}
                aria-hidden
              />
              <span className="font-mono font-medium text-fg tnum">
                {v === null || v === undefined ? "—" : (s.format ?? compact)(v)}
              </span>
              <span className="text-fg-subtle">{s.label}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export function Legend({ series }: { series: Series[] }) {
  if (series.length < 2) return null;
  return (
    <div className="flex flex-wrap items-center gap-3 text-2xs text-fg-subtle">
      {series.map((s) => (
        <span key={s.key} className="inline-flex items-center gap-1.5">
          {s.kind === "bar" ? (
            <span
              className="size-2 rounded-[2px]"
              style={{ background: s.color }}
            />
          ) : (
            <span
              className="h-0.5 w-3 rounded-full"
              style={{ background: s.color }}
            />
          )}
          {s.label}
        </span>
      ))}
    </div>
  );
}

/**
 * Time series with one shared y-axis. Drag across the plot to select a
 * range when `onSelectRange` is provided.
 */
export function TimeSeriesChart({
  data,
  series,
  bucketMs,
  height = 220,
  stacked = false,
  yFormat = compact,
  onSelectRange,
  dimmed,
}: {
  data: Datum[];
  series: Series[];
  bucketMs: number;
  height?: number;
  stacked?: boolean;
  yFormat?: (v: number) => string;
  onSelectRange?: (from: number, to: number) => void;
  dimmed?: boolean;
}) {
  const [drag, setDrag] = useState<{ a: number; b: number } | null>(null);
  const barSize = useMemo(
    () =>
      Math.max(2, Math.min(24, Math.floor(640 / Math.max(1, data.length)) - 2)),
    [data.length],
  );
  return (
    <div
      className={cn(
        "w-full transition-opacity duration-150",
        dimmed && "refetching",
      )}
      style={{ height }}
    >
      <ResponsiveContainer width="100%" height="100%">
        <ComposedChart
          data={data}
          margin={{ top: 6, right: 4, bottom: 0, left: 0 }}
          barCategoryGap={1}
          onMouseDown={(e) =>
            onSelectRange &&
            e?.activeLabel !== undefined &&
            setDrag({ a: Number(e.activeLabel), b: Number(e.activeLabel) })
          }
          onMouseMove={(e) =>
            drag &&
            e?.activeLabel !== undefined &&
            setDrag({ ...drag, b: Number(e.activeLabel) })
          }
          onMouseUp={() => {
            if (drag && onSelectRange && drag.a !== drag.b) {
              const from = Math.min(drag.a, drag.b);
              const to = Math.max(drag.a, drag.b) + bucketMs;
              onSelectRange(from, to);
            }
            setDrag(null);
          }}
          onMouseLeave={() => setDrag(null)}
        >
          <CartesianGrid vertical={false} />
          <XAxis
            dataKey="t"
            type="category"
            tickFormatter={(t: number) => shortDate(t, bucketMs)}
            tickLine={false}
            axisLine={false}
            minTickGap={48}
            tickMargin={8}
            height={22}
          />
          <YAxis
            tickFormatter={(v: number) => yFormat(v)}
            tickLine={false}
            axisLine={false}
            width={44}
            tickMargin={4}
            allowDecimals={false}
          />
          <RTooltip
            cursor={{
              stroke: "var(--fg-faint)",
              strokeWidth: 1,
              fill: "var(--surface-3)",
              fillOpacity: 0.35,
            }}
            content={<ChartTooltip series={series} bucketMs={bucketMs} />}
            isAnimationActive={false}
          />
          {series.map((s) =>
            s.kind === "bar" ? (
              <Bar
                key={s.key}
                dataKey={s.key}
                stackId={stacked ? "a" : undefined}
                fill={s.color}
                stroke="var(--surface)"
                strokeWidth={stacked ? 1 : 0}
                maxBarSize={barSize}
                radius={stacked ? 0 : [2, 2, 0, 0]}
                isAnimationActive={false}
              />
            ) : (
              <Area
                key={s.key}
                dataKey={s.key}
                type="monotone"
                stroke={s.color}
                strokeWidth={2}
                fill={s.color}
                fillOpacity={0.08}
                dot={false}
                activeDot={{
                  r: 4,
                  stroke: "var(--surface)",
                  strokeWidth: 2,
                  fill: s.color,
                }}
                connectNulls
                isAnimationActive={false}
              />
            ),
          )}
          {drag && drag.a !== drag.b && (
            <ReferenceArea
              x1={Math.min(drag.a, drag.b)}
              x2={Math.max(drag.a, drag.b)}
              fill="var(--fg)"
              fillOpacity={0.06}
              stroke="var(--fg-faint)"
            />
          )}
        </ComposedChart>
      </ResponsiveContainer>
    </div>
  );
}

export interface BarListItem {
  key: string;
  label: ReactNode;
  value: number;
  detail?: ReactNode;
  secondary?: ReactNode;
}

/** Sorted horizontal bars for a single measure across categories. */
export function BarList({
  items,
  format = compact,
  empty,
  onSelect,
  selected,
}: {
  items: BarListItem[];
  format?: (v: number) => string;
  empty?: ReactNode;
  onSelect?: (key: string) => void;
  selected?: string | null;
}) {
  const max = Math.max(1, ...items.map((i) => i.value));
  const total = items.reduce((a, b) => a + b.value, 0);
  if (!items.length)
    return (
      <div className="py-6 text-center text-xs text-fg-subtle">
        {empty ?? "No data in this range."}
      </div>
    );
  return (
    <div className="flex flex-col gap-2.5">
      {items.map((it) => {
        const share = total > 0 ? it.value / total : 0;
        const row = (
          <button
            key={it.key}
            type="button"
            onClick={() => onSelect?.(it.key)}
            disabled={!onSelect}
            className={cn(
              "group block w-full text-left disabled:cursor-default",
              selected && selected !== it.key && "opacity-45",
            )}
          >
            <div className="mb-1 flex items-baseline gap-2 text-xs">
              <span className="min-w-0 flex-1 truncate text-fg">
                {it.label}
              </span>
              {it.secondary && (
                <span className="text-2xs text-fg-subtle">{it.secondary}</span>
              )}
              <span className="font-mono text-[11.5px] text-fg tnum">
                {format(it.value)}
              </span>
              <span className="w-9 text-right font-mono text-2xs text-fg-subtle tnum">
                {Math.round(share * 100)}%
              </span>
            </div>
            <div className="h-1.5 rounded-full bg-surface-3">
              <div
                className="h-full rounded-full bg-fg-muted transition-[width] duration-500 ease-out-soft group-hover:bg-fg"
                style={{ width: `${(it.value / max) * 100}%` }}
              />
            </div>
          </button>
        );
        return it.detail ? (
          <Tooltip key={it.key} content={it.detail} side="left">
            {row}
          </Tooltip>
        ) : (
          row
        );
      })}
    </div>
  );
}

/** Tiny trend line for stat tiles. */
export function Sparkline({
  values,
  width = 84,
  height = 22,
}: {
  values: number[];
  width?: number;
  height?: number;
}) {
  if (values.length < 2)
    return <svg width={width} height={height} aria-hidden />;
  const max = Math.max(...values, 1);
  const step = width / (values.length - 1);
  const pts = values.map(
    (v, i) =>
      `${(i * step).toFixed(1)},${(height - 2 - (v / max) * (height - 4)).toFixed(1)}`,
  );
  return (
    <svg width={width} height={height} aria-hidden className="overflow-visible">
      <polyline
        points={pts.join(" ")}
        fill="none"
        stroke="var(--fg-subtle)"
        strokeWidth="1.5"
        strokeLinejoin="round"
        strokeLinecap="round"
      />
      <circle
        cx={(values.length - 1) * step}
        cy={height - 2 - (values[values.length - 1] / max) * (height - 4)}
        r="2.5"
        fill="var(--fg)"
      />
    </svg>
  );
}
