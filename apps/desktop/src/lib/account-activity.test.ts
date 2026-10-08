import { describe, expect, it } from "vitest";
import type { ProviderUsageReport } from "@magpie/sdk";
import { accountPoints, accountRange, accountCsv, DAY } from "./account-activity";
import { buildTokenActivity } from "./token-activity";

function report(daily: ProviderUsageReport["daily"]): ProviderUsageReport {
  return { account_id: "codex-test", daily, provenance: "reported", observed_at: "2026-10-08T12:00:00Z" };
}
describe("reported account activity", () => {
  it("replaces duplicate day snapshots instead of double counting and rejects invalid data", () => {
    const points = accountPoints(
      report([
        { date: "2026-10-08", tokens: 70 },
        { date: "2026-10-08", tokens: 100 },
        { date: "2026-10-07", tokens: 0 },
        { date: "2026-02-30", tokens: 99 },
        { date: "2026-10-06", tokens: -1 },
        { date: "2026-10-05", tokens: 1.5 },
      ]),
    );
    expect(points).toEqual([
      { t: Date.UTC(2026, 9, 7), tokens: 0 },
      { t: Date.UTC(2026, 9, 8), tokens: 100 },
    ]);
    expect(accountCsv(points)).toContain("2026-10-08,100,reported,provider_account");
  });
  it("distinguishes an unreported date from an explicitly reported zero and never invents requests", () => {
    const start = Date.UTC(2026, 9, 6);
    const points = accountPoints(
      report([
        { date: "2026-10-07", tokens: 0 },
        { date: "2026-10-08", tokens: 50 },
      ]),
    );
    const cells = buildTokenActivity(points, start, start + 2 * DAY, "daily", true);
    expect(cells.map((c) => [c.known, c.value, c.requests])).toEqual([
      [false, 0, null],
      [true, 0, null],
      [true, 50, null],
    ]);
    expect(buildTokenActivity([], start, start, "cumulative", true)[0].known).toBe(false);
  });
  it("uses whole UTC days and excludes the exclusive upper boundary", () => {
    const now = Date.UTC(2026, 9, 8, 15);
    expect(accountRange({ kind: "24h" }, now)).toEqual({ from: Date.UTC(2026, 9, 8), to: Date.UTC(2026, 9, 9) });
    const r = accountRange({ kind: "7d" }, now);
    expect(r.to - r.from).toBe(7 * DAY);
    expect(accountRange({ kind: "custom", from: now, to: Date.UTC(2026, 9, 9) }, now)).toEqual({
      from: Date.UTC(2026, 9, 8),
      to: Date.UTC(2026, 9, 9),
    });
  });
});
