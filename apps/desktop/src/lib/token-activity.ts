import type { TimeseriesPoint } from "@magpie/sdk";
export interface ActivityCell {
  t: number;
  week: number;
  day: number;
  value: number;
  requests: number;
  label: string;
}
const DAY = 86_400_000;
const dateFormat = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "short",
  year: "numeric",
  timeZone: "UTC",
});
/** UTC day buckets match SQLite telemetry. Missing days are local zero activity. */
export function buildTokenActivity(
  points: TimeseriesPoint[],
  start: number,
  end: number,
  mode: "daily" | "weekly" | "cumulative",
): ActivityCell[] {
  const first = Math.floor(start / DAY) * DAY;
  const gridStart = first - new Date(first).getUTCDay() * DAY;
  const byDay = new Map<number, { tokens: number; requests: number }>();
  for (const p of points) {
    const day = Math.floor(p.t / DAY) * DAY;
    const existing = byDay.get(day) ?? { tokens: 0, requests: 0 };
    byDay.set(day, {
      tokens: existing.tokens + p.input_tokens + p.output_tokens,
      requests: existing.requests + p.requests,
    });
  }
  const weekly = new Map<number, number>();
  for (const [day, data] of byDay) {
    if (day < first || day > end) continue;
    const week = Math.floor((day - gridStart) / (7 * DAY));
    weekly.set(week, (weekly.get(week) ?? 0) + data.tokens);
  }
  let cumulative = 0;
  const result: ActivityCell[] = [];
  for (let t = first; t <= end; t += DAY) {
    const day = byDay.get(t) ?? { tokens: 0, requests: 0 };
    const week = Math.floor((t - gridStart) / (7 * DAY));
    cumulative += day.tokens;
    const date = dateFormat.format(t);
    result.push({
      t,
      week,
      day: new Date(t).getUTCDay(),
      value: mode === "weekly" ? (weekly.get(week) ?? 0) : mode === "cumulative" ? cumulative : day.tokens,
      requests: day.requests,
      label:
        mode === "weekly" ? `Week containing ${date}` : mode === "cumulative" ? `Cumulative through ${date}` : date,
    });
  }
  return result;
}
