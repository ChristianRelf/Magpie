import { createContext, useCallback, useContext, useState, type ReactNode } from "react";
import {
  AlertCircle,
  CheckCircle2,
  CircleDashed,
  CircleSlash,
  Clock3,
  KeyRound,
  PowerOff,
  Unplug,
  X,
  CircleDot,
  Circle,
  BadgeCheck,
  Calculator,
  CircleHelp,
} from "lucide-react";
import type { ConnectionStatus, ExecutionStatus, LimitState, Provenance } from "@magpie/sdk";
import { cn, Tooltip } from "./core";
import { CONNECTION_LABELS, LIMIT_STATE_LABELS, PROVENANCE_HELP, PROVENANCE_LABELS } from "@/lib/format";

// ------------------------------------------------------------------ toasts

interface Toast {
  id: number;
  title: string;
  description?: string;
  tone: "info" | "success" | "error";
}

const ToastContext = createContext<(t: Omit<Toast, "id">) => void>(() => {});

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const push = useCallback((t: Omit<Toast, "id">) => {
    const id = Date.now() + Math.random();
    setToasts((all) => [...all.slice(-3), { ...t, id }]);
    setTimeout(() => setToasts((all) => all.filter((x) => x.id !== id)), t.tone === "error" ? 7000 : 3500);
  }, []);
  return (
    <ToastContext.Provider value={push}>
      {children}
      <div
        className="pointer-events-none fixed right-4 bottom-4 z-[60] flex w-80 flex-col gap-2"
        role="status"
        aria-live="polite"
      >
        {toasts.map((t) => (
          <div
            key={t.id}
            className="rise-in pointer-events-auto flex items-start gap-2.5 rounded-lg border border-border-strong bg-surface-2 px-3 py-2.5 shadow-panel"
          >
            {t.tone === "error" ? (
              <AlertCircle className="mt-px size-4 shrink-0" />
            ) : t.tone === "success" ? (
              <CheckCircle2 className="mt-px size-4 shrink-0" />
            ) : (
              <CircleDot className="mt-px size-4 shrink-0 text-fg-muted" />
            )}
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium">{t.title}</p>
              {t.description && <p className="mt-0.5 text-xs break-words text-fg-subtle">{t.description}</p>}
            </div>
            <button
              onClick={() => setToasts((all) => all.filter((x) => x.id !== t.id))}
              className="text-fg-subtle hover:text-fg"
              aria-label="Dismiss"
            >
              <X className="size-3.5" />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  return useContext(ToastContext);
}

export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

// ----------------------------------------------------------- status glyphs

/** Limit state: shape and fill carry meaning, never colour. */
export function LimitGlyph({ state, className }: { state: LimitState; className?: string }) {
  const c = cn("size-3.5 shrink-0", className);
  switch (state) {
    case "available":
      return (
        <span className={cn(c, "inline-block rounded-full bg-fg-muted")} style={{ width: 9, height: 9 }} aria-hidden />
      );
    case "approaching":
      return (
        <svg viewBox="0 0 12 12" className={c} aria-hidden>
          <circle cx="6" cy="6" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
          <path d="M6 1.5 A4.5 4.5 0 0 1 6 10.5 Z" fill="currentColor" />
        </svg>
      );
    case "limited":
      return (
        <svg viewBox="0 0 12 12" className={c} aria-hidden>
          <circle cx="6" cy="6" r="4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
          <path d="M6 1.5 A4.5 4.5 0 1 1 1.5 6 L6 6 Z" fill="currentColor" />
        </svg>
      );
    case "exhausted":
      return <CircleSlash className={c} aria-hidden />;
    case "reset_pending":
      return <Clock3 className={c} aria-hidden />;
    default:
      return <CircleDashed className={cn(c, "text-fg-faint")} aria-hidden />;
  }
}

export function LimitStatus({ state, compact }: { state: LimitState; compact?: boolean }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-xs text-fg-muted">
      <LimitGlyph state={state} />
      {!compact && LIMIT_STATE_LABELS[state]}
    </span>
  );
}

export function ConnectionGlyph({ status, className }: { status: ConnectionStatus; className?: string }) {
  const c = cn("size-3.5 shrink-0", className);
  switch (status) {
    case "connected":
      return <CheckCircle2 className={c} aria-hidden />;
    case "needs_auth":
      return <KeyRound className={c} aria-hidden />;
    case "unavailable":
      return <Unplug className={c} aria-hidden />;
    case "error":
      return <AlertCircle className={c} aria-hidden />;
    case "disabled":
      return <PowerOff className={cn(c, "text-fg-faint")} aria-hidden />;
    default:
      return <CircleDashed className={cn(c, "animate-[spin_3s_linear_infinite] text-fg-subtle")} aria-hidden />;
  }
}

export function ConnectionStatusLabel({ status, message }: { status: ConnectionStatus; message?: string }) {
  return (
    <Tooltip content={status !== "connected" ? message : undefined}>
      <span className="inline-flex items-center gap-1.5 text-xs text-fg-muted">
        <ConnectionGlyph status={status} />
        {CONNECTION_LABELS[status]}
      </span>
    </Tooltip>
  );
}

export function ExecutionGlyph({ status }: { status: ExecutionStatus }) {
  switch (status) {
    case "succeeded":
      return <CheckCircle2 className="size-3.5 text-fg-muted" aria-label="Succeeded" />;
    case "failed":
      return <AlertCircle className="size-3.5 text-fg" aria-label="Failed" />;
    case "cancelled":
      return <CircleSlash className="size-3.5 text-fg-subtle" aria-label="Cancelled" />;
    default:
      return (
        <Circle
          className="size-3.5 animate-[pulse-soft_1.2s_ease-in-out_infinite] fill-current text-fg"
          aria-label="Running"
        />
      );
  }
}

/** Marks the origin of a figure. Estimated values are always labelled. */
export function ProvenanceTag({
  provenance,
  className,
  observedAt,
}: {
  provenance: Provenance;
  className?: string;
  observedAt?: string;
}) {
  const Icon =
    provenance === "reported"
      ? BadgeCheck
      : provenance === "calculated"
        ? Calculator
        : provenance === "estimated"
          ? CircleDashed
          : CircleHelp;
  return (
    <Tooltip
      content={
        <>
          <span className="font-medium">{PROVENANCE_LABELS[provenance]}.</span> {PROVENANCE_HELP[provenance]}
          {observedAt && (
            <span className="mt-1 block text-fg-muted">Updated {new Date(observedAt).toLocaleString()}</span>
          )}
        </>
      }
    >
      <span
        className={cn("inline-flex size-4 shrink-0 items-center justify-center text-fg-subtle", className)}
        aria-label={PROVENANCE_LABELS[provenance]}
        tabIndex={0}
      >
        <Icon className="size-3.5" aria-hidden />
      </span>
    </Tooltip>
  );
}

/** Horizontal meter. The unfilled track is a lighter step of the same grey. */
export function Meter({
  value,
  className,
  label,
}: {
  value: number | null | undefined;
  className?: string;
  label?: string;
}) {
  if (value === null || value === undefined) {
    return (
      <div
        className={cn(
          "h-1 rounded-full bg-[repeating-linear-gradient(90deg,var(--surface-3)_0_3px,transparent_3px_6px)]",
          className,
        )}
        aria-label={label ? `${label}: unknown` : "Unknown"}
      />
    );
  }
  const v = Math.max(0, Math.min(1, value));
  return (
    <div
      className={cn("h-1 overflow-hidden rounded-full bg-surface-3", className)}
      role="meter"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(v * 100)}
      aria-label={label}
    >
      <div
        className={cn(
          "h-full rounded-full transition-[width] duration-500 ease-out-soft",
          v >= 0.95 ? "bg-fg" : v >= 0.8 ? "bg-fg-muted" : "bg-fg-subtle",
        )}
        style={{ width: `${v * 100}%` }}
      />
    </div>
  );
}
