import * as Popover from "@radix-ui/react-popover";
import {
  Activity,
  Bell,
  Blocks,
  Boxes,
  ChartColumn,
  LayoutGrid,
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

const mod = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "⌘" : "Ctrl+";

function HarnessIndicator({ collapsed }: { collapsed: boolean }) {
  const { phase, connection } = useHarness();
  const status = useStatus().data;
  const running = phase === "ready";
  const label = running ? "Harness running" : phase === "connecting" ? "Connecting" : "Harness offline";
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
      <div className={cn("sidebar-item text-xs text-fg-muted")}>
        <span className="sidebar-icon relative flex items-center justify-center">
          {running ? (
            <>
              <span className="absolute size-2.5 animate-[pulse-soft_2.4s_ease-in-out_infinite] rounded-full bg-fg-subtle opacity-40" />
              <span className="size-1.5 rounded-full bg-fg" />
            </>
          ) : (
            <span className="size-2 rounded-full border border-fg-subtle" />
          )}
        </span>
        <span className="sidebar-label" aria-hidden={collapsed}>
          {label}
        </span>
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
          void client.markNotificationsRead().then(() => qc.invalidateQueries({ queryKey: ["notifications"] }));
      }}
    >
      <Tooltip content="Notifications" side="right">
        <Popover.Trigger asChild>
          <button
            aria-label={`Notifications${unread ? `, ${unread} unread` : ""}`}
            className={cn("sidebar-item w-full text-sm text-fg-subtle hover:bg-surface-2 hover:text-fg")}
          >
            <Bell className="size-4" />
            <span className="sidebar-label flex-1" aria-hidden={collapsed}>
              Notifications
            </span>
            {unread > 0 && <span className="absolute top-2 left-7 size-1.5 rounded-full bg-fg" />}
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
          <div className="border-b border-border px-3 py-2 text-xs font-medium">Notifications</div>
          <div className="max-h-96 overflow-y-auto">
            {items.length === 0 ? (
              <p className="px-3 py-6 text-center text-xs text-fg-subtle">Nothing to report.</p>
            ) : (
              items.slice(0, 40).map((n) => (
                <div key={n.id} className="flex gap-2.5 border-b border-border px-3 py-2.5 last:border-0">
                  <span
                    className={cn("mt-1.5 size-1.5 shrink-0 rounded-full", n.read ? "bg-transparent" : "bg-fg")}
                    aria-hidden
                  />
                  <div className="min-w-0 flex-1">
                    <p className="text-xs font-medium">{n.title}</p>
                    <p className="mt-0.5 text-2xs text-fg-subtle">{n.body}</p>
                    <p className="mt-1 text-2xs text-fg-faint">{relative(n.created_at)}</p>
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
    !save.isPending &&
    save.mutate({
      ...settings,
      general: { ...settings.general, sidebar_collapsed: !collapsed },
    });

  return (
    <nav
      aria-label="Main"
      data-collapsed={collapsed}
      className="sidebar flex shrink-0 flex-col border-r border-border bg-bg-subtle"
    >
      <div className="flex h-[72px] shrink-0 items-center gap-3.5 px-6">
        <MagpieMark size={24} className="shrink-0 text-fg" />
        <span className="sidebar-label text-lg font-semibold tracking-tight" aria-hidden={collapsed}>
          Magpie
        </span>
      </div>
      <div
        id="sidebar-links"
        className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto overflow-x-hidden px-3 py-3"
      >
        {ITEMS.map((item, i) => {
          const active = route === item.route;
          return (
            <Tooltip
              key={item.route}
              content={
                <span className="flex items-center gap-2">
                  {item.label}{" "}
                  <Kbd>
                    {mod}
                    {i + 1}
                  </Kbd>
                </span>
              }
              side="right"
            >
              <button
                onClick={() => navigate(item.route)}
                aria-current={active ? "page" : undefined}
                aria-label={item.label}
                className={cn(
                  "sidebar-item text-sm",
                  active ? "bg-surface-3 font-medium text-fg" : "text-fg-subtle hover:bg-surface-2 hover:text-fg",
                  item.route === "settings" && "mt-auto",
                )}
              >
                {item.icon}
                <span className="sidebar-label" aria-hidden={collapsed}>
                  {item.label}
                </span>
              </button>
            </Tooltip>
          );
        })}
      </div>
      <div className="flex shrink-0 flex-col gap-1 border-t border-border p-3">
        <Notifications collapsed={collapsed} />
        <HarnessIndicator collapsed={collapsed} />
        <Tooltip content={collapsed ? "Expand sidebar" : "Collapse sidebar"} side="right">
          <button
            onClick={toggle}
            disabled={save.isPending}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            aria-expanded={!collapsed}
            aria-controls="sidebar-links"
            className="sidebar-item text-fg-subtle hover:bg-surface-2 hover:text-fg"
          >
            <PanelLeftClose className={cn("transition-transform duration-300", collapsed && "rotate-180")} />
          </button>
        </Tooltip>
      </div>
    </nav>
  );
}
