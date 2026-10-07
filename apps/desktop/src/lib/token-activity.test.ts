import { describe, expect, it } from "vitest";
import { buildTokenActivity } from "./token-activity";
const DAY = 86_400_000;
const first = Date.UTC(2026, 9, 4); // Sunday
const point = (t: number, input: number, output: number) => ({
  t,
  input_tokens: input,
  output_tokens: output,
  requests: 1,
  failed: 0,
  cost_usd: 0,
  avg_duration_ms: null,
});
describe("token activity calendar", () => {
  it("fills UTC dates across a year boundary without dropping empty days", () => {
    const start = Date.UTC(2025, 11, 31);
    const cells = buildTokenActivity(
      [point(start, 3, 7)],
      start,
      start + 2 * DAY,
      "daily",
    );
    expect(cells.map((c) => c.value)).toEqual([10, 0, 0]);
    expect(cells.map((c) => c.day)).toEqual([3, 4, 5]);
  });
  it("adds duplicate provider buckets and groups weeks from Sunday", () => {
    const points = [
      point(first, 10, 20),
      point(first, 5, 5),
      point(first + DAY, 5, 5),
      point(first + 7 * DAY, 100, 0),
    ];
    const cells = buildTokenActivity(points, first, first + 7 * DAY, "weekly");
    expect(cells.slice(0, 7).map((c) => c.value)).toEqual(Array(7).fill(50));
    expect(cells[7].value).toBe(100);
  });
  it("cumulative values never double count missing days", () => {
    const cells = buildTokenActivity(
      [point(first, 10, 20), point(first + 2 * DAY, 0, 10)],
      first,
      first + 3 * DAY,
      "cumulative",
    );
    expect(cells.map((c) => c.value)).toEqual([30, 30, 40, 40]);
  });
});
