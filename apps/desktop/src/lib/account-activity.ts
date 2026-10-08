import type { ProviderUsageReport } from "@magpie/sdk";
import type { RangeValue } from "@/components/RangePicker";

export const DAY = 86_400_000;
export interface AccountPoint {
  t: number;
  tokens: number;
}

/** Provider snapshots replace previous totals; they are never increments. */
export function accountPoints(report?: ProviderUsageReport): AccountPoint[] {
  const days = new Map<number, number>();
  for (const row of report?.daily ?? []) {
    if (!/^\d{4}-\d{2}-\d{2}$/.test(row.date)) continue;
    const t = Date.parse(`${row.date}T00:00:00Z`);
    if (!Number.isFinite(t) || new Date(t).toISOString().slice(0, 10) !== row.date) continue;
    if (!Number.isSafeInteger(row.tokens) || row.tokens < 0) continue;
    days.set(t, row.tokens);
  }
  return [...days].sort(([a], [b]) => a - b).map(([t, tokens]) => ({ t, tokens }));
}

export function accountRange(range: RangeValue, now: number): { from: number; to: number } {
  if (range.kind === "custom")
    return {
      from: Math.floor(range.from / DAY) * DAY,
      to: Math.ceil(range.to / DAY) * DAY,
    };
  const to = Math.floor(now / DAY) * DAY + DAY;
  return { from: to - (range.kind === "30d" ? 30 : range.kind === "7d" ? 7 : 1) * DAY, to };
}

export function accountCsv(points: AccountPoint[]): string {
  return (
    "date_utc,tokens,provenance,scope\n" +
    points.map((p) => `${new Date(p.t).toISOString().slice(0, 10)},${p.tokens},reported,provider_account`).join("\n") +
    "\n"
  );
}
