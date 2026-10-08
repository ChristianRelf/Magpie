import type { LimitState, LimitWindow } from "@magpie/sdk";
import { useEffect, useState } from "react";
import { compact, duration, usd } from "@/lib/format";
import { LimitGlyph, Meter, ProvenanceTag } from "./ui/feedback";
import { Tooltip } from "./ui/core";

export function windowState(w: LimitWindow, now = Date.now()): LimitState {
  const resetPassed = w.resets_at ? Date.parse(w.resets_at) <= now : false;
  if (resetPassed) return "reset_pending";
  const used = usedFraction(w);
  if (w.exhausted || w.remaining === 0 || used === 1) return "exhausted";
  if (w.approaching && !resetPassed && used === null) return "approaching";
  if (used === null) {
    if (w.metric === "credits" && w.remaining !== undefined) return w.remaining <= 0 ? "exhausted" : "available";
    return "unknown";
  }
  if (used >= 0.95) return "limited";
  if (used >= 0.8) return "approaching";
  return "available";
}

export function usedFraction(w: LimitWindow): number | null {
  if (w.used_percent !== undefined && w.used_percent !== null) return Math.max(0, Math.min(1, w.used_percent / 100));
  if (w.limit !== undefined && w.remaining !== undefined && w.limit > 0)
    return Math.max(0, Math.min(1, (w.limit - w.remaining) / w.limit));
  return null;
}

/** Re-render every `ms` so countdowns stay current. */
export function useNow(ms = 30_000): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(id);
  }, [ms]);
  return now;
}

function windowValue(w: LimitWindow): string {
  if (w.metric === "credits") return w.remaining !== undefined ? `${usd(w.remaining)} left` : "—";
  if (w.metric === "spend") return w.used !== undefined ? `${usd(w.used)} spent` : "—";
  if (w.used_percent !== undefined) return `${Math.round(w.used_percent)}% used`;
  if (w.limit !== undefined && w.remaining !== undefined)
    return `${compact(w.limit - w.remaining)} / ${compact(w.limit)}`;
  if (w.exhausted) return "Limit reached";
  return "—";
}

export function resetText(w: LimitWindow, now: number): string | null {
  if (!w.resets_at) return null;
  const secs = (Date.parse(w.resets_at) - now) / 1000;
  if (secs <= 0) return "Reset due";
  return `Resets in ${duration(secs)}`;
}

export function LimitWindowRow({ w, now }: { w: LimitWindow; now: number }) {
  const state = windowState(w, now);
  const reset = resetText(w, now);
  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2 text-xs">
        <Tooltip content={state.replace("_", " ")}>
          <span className="text-fg-muted">
            <LimitGlyph state={state} />
          </span>
        </Tooltip>
        <span className="min-w-0 flex-1 truncate text-fg">{w.label}</span>
        <span className="font-mono text-[11.5px] text-fg-muted tnum">
          {state === "reset_pending" ? "Awaiting update" : windowValue(w)}
        </span>
        <ProvenanceTag provenance={w.provenance} />
      </div>
      {w.metric !== "spend" && <Meter value={state === "reset_pending" ? null : usedFraction(w)} label={w.label} />}
      <div className="text-2xs text-fg-subtle" title={new Date(w.observed_at).toLocaleString()}>
        Observed{" "}
        {new Date(w.observed_at).toLocaleString([], {
          month: "short",
          day: "numeric",
          hour: "2-digit",
          minute: "2-digit",
        })}
      </div>
      {(reset || w.model_scope) && (
        <div className="flex justify-between text-2xs text-fg-subtle">
          <span className="font-mono">{w.model_scope ?? ""}</span>
          <span>{reset}</span>
        </div>
      )}
    </div>
  );
}
