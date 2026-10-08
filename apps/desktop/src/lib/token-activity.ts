export interface ActivityPoint {
  t: number;
  tokens?: number;
  input_tokens?: number;
  output_tokens?: number;
  requests?: number;
}
export interface ActivityCell {
  t: number;
  week: number;
  day: number;
  value: number;
  requests: number | null;
  known: boolean;
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
  points: ActivityPoint[],
  start: number,
  end: number,
  mode: "daily" | "weekly" | "cumulative",
  missingIsUnknown = false,
): ActivityCell[] {
  const first = Math.floor(start / DAY) * DAY;
  const gridStart = first - new Date(first).getUTCDay() * DAY;
  const byDay = new Map<number, { tokens: number; requests: number }>();
  for (const p of points) {
    const day = Math.floor(p.t / DAY) * DAY;
    const existing = byDay.get(day) ?? { tokens: 0, requests: 0 };
    byDay.set(day, {
      tokens: existing.tokens + (p.tokens ?? (p.input_tokens ?? 0) + (p.output_tokens ?? 0)),
      requests: existing.requests + (p.requests ?? 0),
    });
  }
  const weekly = new Map<number, number>();
  for (const [day, data] of byDay) {
    if (day < first || day > end) continue;
    const week = Math.floor((day - gridStart) / (7 * DAY));
    weekly.set(week, (weekly.get(week) ?? 0) + data.tokens);
  }
  let cumulative = 0;
  let anyKnown = false;
  const result: ActivityCell[] = [];
  for (let t = first; t <= end; t += DAY) {
    const day = byDay.get(t) ?? { tokens: 0, requests: 0 };
    const week = Math.floor((t - gridStart) / (7 * DAY));
    cumulative += day.tokens;
    anyKnown ||= byDay.has(t);
    const date = dateFormat.format(t);
    result.push({
      t,
      week,
      day: new Date(t).getUTCDay(),
      value: mode === "weekly" ? (weekly.get(week) ?? 0) : mode === "cumulative" ? cumulative : day.tokens,
      requests: missingIsUnknown ? null : day.requests,
      known:
        !missingIsUnknown || (mode === "weekly" ? weekly.has(week) : mode === "cumulative" ? anyKnown : byDay.has(t)),
      label:
        mode === "weekly" ? `Week containing ${date}` : mode === "cumulative" ? `Cumulative through ${date}` : date,
    });
  }
  return result;
}
