import { useState } from "react";
import * as Popover from "@radix-ui/react-popover";
import { CalendarRange, Check } from "lucide-react";
import type { RangeQuery } from "@magpie/sdk";
import { Button, cn } from "./ui/core";

export type RangeValue =
  | { kind: "1h" | "24h" | "7d" | "30d" }
  | { kind: "custom"; from: number; to: number };

export const PRESETS: {
  value: "1h" | "24h" | "7d" | "30d";
  label: string;
  long: string;
}[] = [
  { value: "1h", label: "1H", long: "Last hour" },
  { value: "24h", label: "24H", long: "Last 24 hours" },
  { value: "7d", label: "7D", long: "Last 7 days" },
  { value: "30d", label: "30D", long: "Last 30 days" },
];

export function toQuery(r: RangeValue): RangeQuery {
  return r.kind === "custom"
    ? { range: "custom", from: r.from, to: r.to }
    : { range: r.kind };
}

export function rangeLabel(r: RangeValue): string {
  if (r.kind !== "custom") return PRESETS.find((p) => p.value === r.kind)!.long;
  const f = (t: number) =>
    new Date(t).toLocaleString([], {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  return `${f(r.from)} – ${f(r.to)}`;
}

function toLocalInput(t: number): string {
  const d = new Date(t);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function RangePicker({
  value,
  onChange,
}: {
  value: RangeValue;
  onChange: (v: RangeValue) => void;
}) {
  const [open, setOpen] = useState(false);
  const now = Date.now();
  const [from, setFrom] = useState(
    toLocalInput(value.kind === "custom" ? value.from : now - 86_400_000),
  );
  const [to, setTo] = useState(
    toLocalInput(value.kind === "custom" ? value.to : now),
  );
  const fromMs = Date.parse(from);
  const toMs = Date.parse(to);
  const valid =
    Number.isFinite(fromMs) && Number.isFinite(toMs) && toMs > fromMs;
  return (
    <div
      className="inline-flex items-center gap-1"
      role="group"
      aria-label="Time range"
    >
      <div className="inline-flex rounded-md border border-border bg-bg-subtle p-0.5">
        {PRESETS.map((p) => {
          const active = value.kind === p.value;
          return (
            <button
              key={p.value}
              title={p.long}
              aria-pressed={active}
              onClick={() => onChange({ kind: p.value })}
              className={cn(
                "h-6 rounded-[5px] px-2 font-mono text-2xs font-medium transition-colors",
                active
                  ? "bg-surface-3 text-fg shadow-[inset_0_0_0_1px_var(--border-strong)]"
                  : "text-fg-subtle hover:text-fg",
              )}
            >
              {p.label}
            </button>
          );
        })}
      </div>
      <Popover.Root open={open} onOpenChange={setOpen}>
        <Popover.Trigger asChild>
          <button
            aria-label="Custom range"
            title="Custom range"
            className={cn(
              "inline-flex h-7 items-center gap-1.5 rounded-md border px-2 text-2xs",
              value.kind === "custom"
                ? "border-border-strong bg-surface-3 text-fg"
                : "border-border text-fg-subtle hover:text-fg",
            )}
          >
            <CalendarRange className="size-3.5" />
            {value.kind === "custom" && (
              <span className="max-w-56 truncate">{rangeLabel(value)}</span>
            )}
          </button>
        </Popover.Trigger>
        <Popover.Portal>
          <Popover.Content
            align="end"
            sideOffset={6}
            className="fade-in z-50 w-64 rounded-lg border border-border-strong bg-surface-2 p-1 shadow-panel"
          >
            {PRESETS.map((p) => (
              <button
                key={p.value}
                onClick={() => {
                  onChange({ kind: p.value });
                  setOpen(false);
                }}
                className="flex w-full items-center gap-2 rounded-[5px] px-2 py-1.5 text-left text-xs hover:bg-surface-3"
              >
                <span className="w-4">
                  {value.kind === p.value && (
                    <Check className="size-3.5" strokeWidth={2.75} />
                  )}
                </span>
                {p.long}
              </button>
            ))}
            <div className="mt-1 space-y-2 border-t border-border p-2">
              <div className="text-2xs font-medium text-fg-subtle">
                Custom range
              </div>
              <input
                type="datetime-local"
                value={from}
                onChange={(e) => setFrom(e.target.value)}
                className="h-7 w-full rounded-md border border-border-strong bg-bg px-2 font-mono text-2xs text-fg"
                aria-label="From"
              />
              <input
                type="datetime-local"
                value={to}
                onChange={(e) => setTo(e.target.value)}
                className="h-7 w-full rounded-md border border-border-strong bg-bg px-2 font-mono text-2xs text-fg"
                aria-label="To"
              />
              <Button
                size="xs"
                variant="primary"
                className="w-full"
                disabled={!valid}
                onClick={() => {
                  onChange({ kind: "custom", from: fromMs, to: toMs });
                  setOpen(false);
                }}
              >
                Apply
              </Button>
            </div>
          </Popover.Content>
        </Popover.Portal>
      </Popover.Root>
    </div>
  );
}
