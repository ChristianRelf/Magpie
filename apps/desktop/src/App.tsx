import { useEffect, useRef, type ReactNode } from "react";
import { Power, RotateCw } from "lucide-react";
import { useHarness } from "./lib/harness";
import { NavProvider, useNav, ROUTES } from "./lib/nav";
import { useSettings } from "./lib/queries";
import { checkForUpdates, setWindowBehaviour } from "./lib/desktop";
import { Sidebar } from "./components/Sidebar";
import { Button, Spinner } from "./components/ui/core";
import { MagpieMark } from "./components/ui/marks";
import { Onboarding } from "./views/Onboarding";
import { Overview } from "./views/Overview";
import { Providers } from "./views/Providers";
import { Models } from "./views/Models";
import { Activity } from "./views/Activity";
import { Analytics } from "./views/Analytics";
import { Routing } from "./views/Routing";
import { Integrations } from "./views/Integrations";
import { SettingsView } from "./views/Settings";

/** Applies theme and motion preferences to the document. */
function useAppearance() {
  const settings = useSettings().data?.settings;
  const updateChecked = useRef(false);
  useEffect(() => {
    if (settings?.general.automatic_updates && !updateChecked.current) {
      updateChecked.current = true;
      void checkForUpdates().catch(() => {});
    }
  }, [settings?.general.automatic_updates]);
  const theme = settings?.general.theme ?? "dark";
  const reduce = settings?.general.reduced_motion ?? false;
  useEffect(() => {
    const root = document.documentElement;
    const apply = () => {
      const dark =
        theme === "dark" ||
        (theme === "system" &&
          window.matchMedia("(prefers-color-scheme: dark)").matches);
      root.classList.toggle("dark", dark);
    };
    apply();
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);
  useEffect(() => {
    document.documentElement.classList.toggle("reduce-motion", reduce);
  }, [reduce]);
  useEffect(() => {
    if (settings)
      void setWindowBehaviour(
        settings.general.minimise_to_tray,
        settings.general.keep_harness_running,
      );
  }, [settings]);
}

function CenterScreen({ children }: { children: ReactNode }) {
  return (
    <div className="fade-in flex h-full flex-col items-center justify-center gap-4 p-8 text-center">
      <MagpieMark size={28} className="text-fg" />
      {children}
    </div>
  );
}

function Disconnected() {
  const { error, start, phase } = useHarness();
  return (
    <CenterScreen>
      <div>
        <p className="text-base font-medium">The harness is not running</p>
        <p className="mt-1 max-w-sm text-xs text-fg-subtle">{error}</p>
      </div>
      <Button
        variant="primary"
        size="md"
        icon={<Power className="size-3.5" />}
        loading={phase === "connecting"}
        onClick={() => void start()}
      >
        Start harness
      </Button>
    </CenterScreen>
  );
}

function OfflineBanner() {
  const { phase, error, start } = useHarness();
  if (phase !== "offline") return null;
  return (
    <div className="flex h-9 shrink-0 items-center gap-3 border-b border-border bg-surface-2 px-4 text-xs">
      <Power className="size-3.5 text-fg-muted" />
      <span className="flex-1 text-fg-muted">
        {error ?? "Harness offline"} Showing the last known state.
      </span>
      <Button
        size="xs"
        variant="outline"
        icon={<RotateCw className="size-3" />}
        onClick={() => void start()}
      >
        Start harness
      </Button>
    </div>
  );
}

function Shortcuts() {
  const { navigate } = useNav();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
      const n = Number(e.key);
      if (n >= 1 && n <= ROUTES.length) {
        e.preventDefault();
        navigate(ROUTES[n - 1]);
      } else if (e.key === ",") {
        e.preventDefault();
        navigate("settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);
  return null;
}

function CurrentView() {
  const { route } = useNav();
  switch (route) {
    case "overview":
      return <Overview />;
    case "providers":
      return <Providers />;
    case "models":
      return <Models />;
    case "activity":
      return <Activity />;
    case "analytics":
      return <Analytics />;
    case "routing":
      return <Routing />;
    case "integrations":
      return <Integrations />;
    case "settings":
      return <SettingsView />;
  }
}

function Shell() {
  useAppearance();
  const settings = useSettings();
  if (settings.isLoading) {
    return (
      <CenterScreen>
        <Spinner />
      </CenterScreen>
    );
  }
  if (settings.data && !settings.data.settings.onboarding_complete) {
    return <Onboarding />;
  }
  return (
    <NavProvider>
      <Shortcuts />
      <div className="flex h-full">
        <Sidebar />
        <main className="flex min-w-0 flex-1 flex-col bg-bg">
          <OfflineBanner />
          <div className="min-h-0 flex-1">
            <CurrentView />
          </div>
        </main>
      </div>
    </NavProvider>
  );
}

export function App() {
  const { phase, client } = useHarness();
  if (!client) {
    if (phase === "connecting") {
      return (
        <CenterScreen>
          <div className="flex items-center gap-2 text-xs text-fg-subtle">
            <Spinner /> Starting harness
          </div>
        </CenterScreen>
      );
    }
    return <Disconnected />;
  }
  return <Shell />;
}
