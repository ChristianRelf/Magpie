import type { ReactNode } from "react";
import { cn, Tooltip } from "./ui/core";

export function Page({
  title,
  subtitle,
  actions,
  toolbar,
  children,
  scroll = true,
}: {
  title: string;
  subtitle?: ReactNode;
  actions?: ReactNode;
  toolbar?: ReactNode;
  children: ReactNode;
  scroll?: boolean;
}) {
  return (
    <div className="flex h-full flex-col">
      <header className="flex min-h-[72px] shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-b border-border px-6 py-3">
        <div className="flex min-w-0 flex-1 items-baseline gap-2.5">
          <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
          {subtitle && <span className="hidden truncate text-xs text-fg-subtle md:inline">{subtitle}</span>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
      </header>
      {toolbar && (
        <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border px-6 py-3">{toolbar}</div>
      )}
      <div className={cn("min-h-0 flex-1", scroll ? "overflow-y-auto" : "flex flex-col")}>{children}</div>
    </div>
  );
}

export function StatTile({
  label,
  value,
  sub,
  help,
  trend,
  className,
}: {
  label: string;
  value: ReactNode;
  sub?: ReactNode;
  help?: ReactNode;
  trend?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("flex min-w-0 flex-col justify-between gap-2 px-4 py-3", className)}>
      <div className="flex items-center gap-1.5 text-xs text-fg-subtle">
        <Tooltip content={help}>
          <span
            className={cn(
              help && "cursor-help decoration-fg-faint decoration-dotted underline-offset-4 hover:underline",
            )}
          >
            {label}
          </span>
        </Tooltip>
      </div>
      <div className="flex items-end justify-between gap-2">
        <div className="min-w-0">
          <div className="truncate text-2xl leading-none font-semibold tracking-tight">{value}</div>
          {sub && <div className="mt-1.5 truncate text-2xs text-fg-subtle">{sub}</div>}
        </div>
        {trend}
      </div>
    </div>
  );
}

export function Delta({
  current,
  previous,
  upIsGood = true,
}: {
  current: number;
  previous: number;
  upIsGood?: boolean;
}) {
  if (!previous) return null;
  const change = (current - previous) / previous;
  if (!Number.isFinite(change) || Math.abs(change) < 0.005) return <span className="text-fg-subtle">no change</span>;
  const up = change > 0;
  return (
    <span className={cn(up === upIsGood ? "text-fg-muted" : "text-fg")}>
      {up ? "+" : "−"}
      {Math.abs(change * 100).toFixed(0)}% vs previous
    </span>
  );
}
