import { describe, expect, it } from "vitest";
import type { LimitWindow } from "@magpie/sdk";
import { windowState } from "../components/LimitWindows";

describe("reported allowance freshness", () => {
  it("waits for confirmation after a reset instead of claiming availability", () => {
    const window: LimitWindow = {
      account_id: "claude",
      key: "claude.five_hour",
      label: "5-hour session",
      metric: "usage_percent",
      used_percent: 37,
      exhausted: false,
      provenance: "reported",
      observed_at: "2026-10-08T01:00:00Z",
      resets_at: "2026-10-08T02:00:00Z",
    };
    expect(windowState(window, Date.parse("2026-10-08T01:30:00Z"))).toBe("available");
    expect(windowState(window, Date.parse("2026-10-08T02:00:00Z"))).toBe("reset_pending");
  });
});
