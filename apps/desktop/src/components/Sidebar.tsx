import * as Popover from "@radix-ui/react-popover";
import {
  Activity,
  Bell,
  Blocks,
  Boxes,
  ChartColumn,
  LayoutGrid,
  PanelLeft,
  PanelLeftClose,
  Plug,
  Route as RouteIcon,
  Settings2,
} from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useHarness } from "@/lib/harness";
import { useNav, type Route } from "@/lib/nav";
import { useNotifications, useSettings, useStatus } from "@/lib/queries";
import { useSaveSettings } from "@/lib/mutations";
import { relative } from "@/lib/format";
import { cn, Kbd, Tooltip } from "./ui/core";
import { MagpieMark } from "./ui/marks";

const ITEMS: { route: Route; label: string; icon: ReactNode }[] = [
  { route: "overview", label: "Overview", icon: <LayoutGrid /> },
  { route: "providers", label: "Providers", icon: <Plug /> },
  { route: "models", label: "Models", icon: <Boxes /> },
  { route: "activity", label: "Activity", icon: <Activity /> },
  { route: "analytics", label: "Analytics", icon: <ChartColumn /> },
  { route: "routing", label: "Routing", icon: <RouteIcon /> },
  { route: "integrations", label: "Integrations", icon: <Blocks /> },
  { route: "settings", label: "Settings", icon: <Settings2 /> },
];

const mod =
  typeof navigator !== "undefined" && /Mac/.test(navigator.platform)
    ? "⌘"
    : "Ctrl+";

function HarnessIndicator({ collapsed }: { collapsed: boolean }) {
  const { phase, connection } = useHarness();
  const status = useStatus().data;
  const running = phase === "ready";
  const label = running
    ? "Harness running"
    : phase === "connecting"
      ? "Connecting"
      : "Harness offline";
  const tip =
    running && connection ? (
      <div className="space-y-0.5">
        <div className="font-medium">{label}</div>
        <div className="font-mono text-2xs text-fg-muted">{connection.url}</div>
        {status && (
          <div className="text-2xs text-fg-muted">
            {status.active_executions} active · v{status.version}
          </div>
        )}
      </div>
    ) : (
      label
    );
  return (
    <Tooltip content={tip} side="right">
      <div
        className={cn(
          "flex h-7 items-center gap-2 rounded-md px-2 text-xs text-fg-muted",
          collapsed && "justify-center px-0",
        )}
      >
        <span className="relative flex size-2.5 items-center justify-center">
          {running ? (
            <>
              <span className="absolute size-2.5 animate-[pulse-soft_2.4s_ease-in-out_infinite] rounded-full bg-fg-subtle opacity-40" />
              <span className="size-1.5 rounded-full bg-fg" />
            </>
          ) : (
            <span className="size-2 rounded-full border border-fg-subtle" />
          )}
        </span>
        {!collapsed && <span className="truncate">{label}</span>}
      </div>
    </Tooltip>
  );
}

function Notifications({ collapsed }: { collapsed: boolean }) {
  const { data } = useNotifications();
  const { client } = useHarness();
  const qc = useQueryClient();
  const items = data?.notifications ?? [];
  const unread = items.filter((n) => !n.read).length;
  return (
    <Popover.Root
      onOpenChange={(open) => {
        if (!open && unread > 0 && client)
          void client
            .markNotificationsRead()
            .then(() => qc.invalidateQueries({ queryKey: ["notifications"] }));
      }}
    >
      <Tooltip content="Notifications" side="right">
        <Popover.Trigger asChild>
          <button
            aria-label={`Notifications${unread ? `, ${unread} unread` : ""}`}
            className={cn(
              "relative flex h-7 w-full items-center gap-2 rounded-md px-2 text-xs text-fg-subtle hover:bg-surface-2 hover:text-fg",
              collapsed && "justify-center px-0",
            )}
          >
            <Bell className="size-4" />
            {!collapsed && (
              <span className="flex-1 text-left">Notifications</span>
            )}
            {unread > 0 &&
              (collapsed ? (
                <span className="absolute top-1 right-2 size-1.5 rounded-full bg-fg" />
              ) : (
                <span className="rounded-full bg-fg px-1.5 font-mono text-[10px] leading-4 text-bg">
                  {unread}
                </span>
              ))}
          </button>
        </Popover.Trigger>
      </Tooltip>
      <Popover.Portal>
        <Popover.Content
          side="right"
          align="end"
          sideOffset={8}
          className="fade-in z-50 w-80 rounded-lg border border-border-strong bg-surface-2 shadow-panel"
        >
          <div className="border-b border-border px-3 py-2 text-xs font-medium">
            Notifications
          </div>
          <div className="max-h-96 overflow-y-auto">
            {items.length === 0 ? (
              <p className="px-3 py-6 text-center text-xs text-fg-subtle">
                Nothing to report.
              </p>
            ) : (
              items.slice(0, 40).map((n) => (
                <div
                  key={n.id}
                  className="flex gap-2.5 border-b border-border px-3 py-2.5 last:border-0"
                >
                  <span
                    className={cn(
                      "mt-1.5 size-1.5 shrink-0 rounded-full",
                      n.read ? "bg-transparent" : "bg-fg",
                    )}
                    aria-hidden
                  />
                  <div className="min-w-0 flex-1">
                    <p className="text-xs font-medium">{n.title}</p>
                    <p className="mt-0.5 text-2xs text-fg-subtle">{n.body}</p>
                    <p className="mt-1 text-2xs text-fg-faint">
                      {relative(n.created_at)}
                    </p>
                  </div>
                </div>
              ))
            )}
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

export function Sidebar() {
  const { route, navigate } = useNav();
  const settingsQ = useSettings();
  const save = useSaveSettings();
  const settings = settingsQ.data?.settings;
  const collapsed = settings?.general.sidebar_collapsed ?? false;
  const toggle = () =>
    settings &&
    save.mutate({
      ...settings,
      general: { ...settings.general, sidebar_collapsed: !collapsed },
    });

  return (
    <nav
      aria-label="Main"
      className={cn(
        "flex shrink-0 flex-col border-r border-border bg-bg-subtle transition-[width] duration-200 ease-out-soft",
        collapsed ? "w-[52px]" : "w-[188px]",
      )}
    >
      <div
        className={cn(
          "flex h-12 items-center gap-2.5 px-3.5",
          collapsed && "justify-center px-0",
        )}
      >
        <MagpieMark size={18} className="shrink-0 text-fg" />
        {!collapsed && (
          <span className="text-[13.5px] font-semibold tracking-tight">
            Magpie
          </span>
        )}
      </div>
      <div className="flex flex-1 flex-col gap-0.5 px-2 pt-1">
        {ITEMS.map((item, i) => {
          const active = route === item.route;
          return (
            <Tooltip
              key={item.route}
              content={
                collapsed ? (
                  <span className="flex items-center gap-2">
                    {item.label}{" "}
                    <Kbd>
                      {mod}
                      {i + 1}
                    </Kbd>
                  </span>
                ) : undefined
              }
              side="right"
            >
              <button
                onClick={() => navigate(item.route)}
                aria-current={active ? "page" : undefined}
                aria-label={item.label}
                className={cn(
                  "group flex h-7 items-center gap-2.5 rounded-md px-2 text-[12.5px] transition-colors duration-150 [&_svg]:size-4 [&_svg]:shrink-0",
                  active
                    ? "bg-surface-3 font-medium text-fg"
                    : "text-fg-subtle hover:bg-surface-2 hover:text-fg",
                  collapsed && "justify-center px-0",
                )}
              >
                {item.icon}
                {!collapsed && <span className="truncate">{item.label}</span>}
              </button>
            </Tooltip>
          );
        })}
      </div>
      <div className="flex flex-col gap-0.5 border-t border-border p-2">
        <Notifications collapsed={collapsed} />
        <HarnessIndicator collapsed={collapsed} />
        <Tooltip
          content={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          side="right"
        >
          <button
            onClick={toggle}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            className={cn(
              "flex h-7 items-center gap-2 rounded-md px-2 text-xs text-fg-subtle hover:bg-surface-2 hover:text-fg",
              collapsed && "justify-center px-0",
            )}
          >
            {collapsed ? (
              <PanelLeft className="size-4" />
            ) : (
              <PanelLeftClose className="size-4" />
            )}
            {!collapsed && <span>Collapse</span>}
          </button>
        </Tooltip>
      </div>
    </nav>
  );
}
