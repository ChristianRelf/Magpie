import { useEffect, useState } from "react";
import type { Settings } from "@magpie/sdk";
import { Download, FolderOpen, Power, RotateCw } from "lucide-react";
import { Page } from "@/components/Page";
import { Button, Input, Panel, PanelHeader } from "@/components/ui/core";
import { Dialog, Select, SettingRow, Switch } from "@/components/ui/controls";
import { QueryState } from "@/components/QueryState";
import { useSettings, useStatus } from "@/lib/queries";
import { useHarness } from "@/lib/harness";
import { useAction } from "@/lib/action";
import {
  appInfo,
  checkForUpdates,
  installUpdate,
  isTauri,
  revealPath,
  saveTextFile,
  type UpdateInfo,
} from "@/lib/desktop";
import { titleCase } from "@/lib/format";

function SettingsEditor({ initial }: { initial: Settings }) {
  const [draft, setDraft] = useState(initial);
  const [saved, setSaved] = useState(initial);
  const [clear, setClear] = useState(false);
  const [info, setInfo] = useState<Awaited<ReturnType<typeof appInfo>>>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const { client, phase, stop, restart, start } = useHarness();
  const action = useAction();
  const status = useStatus().data;
  useEffect(() => {
    void appInfo()
      .then(setInfo)
      .catch(() => {});
  }, []);
  const patch = <K extends keyof Settings>(
    section: K,
    value: Partial<Settings[K]>,
  ) =>
    setDraft({
      ...draft,
      [section]: { ...(draft[section] as object), ...(value as object) },
    });
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const toggle = <
    K extends "general" | "analytics" | "security" | "notifications",
  >(
    section: K,
    key: keyof Settings[K],
    title: string,
    description?: string,
    disabled = false,
  ) => (
    <SettingRow
      key={`${section}.${String(key)}`}
      title={title}
      description={description}
    >
      <Switch
        label={title}
        disabled={disabled}
        checked={Boolean(draft[section][key])}
        onCheckedChange={(v) =>
          patch(section, { [key]: v } as Partial<Settings[K]>)
        }
      />
    </SettingRow>
  );
  return (
    <Page
      title="Settings"
      subtitle={dirty ? "Unsaved changes" : "Local preferences"}
      actions={
        <>
          <Button disabled={!dirty} onClick={() => setDraft(saved)}>
            Discard
          </Button>
          <Button
            variant="primary"
            disabled={!dirty}
            loading={action.busy}
            onClick={() =>
              void action.run(
                async () => {
                  const result = await client!.updateSettings(draft);
                  setDraft(result.settings);
                  setSaved(result.settings);
                },
                "Settings saved",
                ["settings"],
              )
            }
          >
            Save changes
          </Button>
        </>
      }
    >
      <div className="mx-auto max-w-4xl space-y-5 p-5">
        <Panel>
          <PanelHeader
            title="Harness service"
            subtitle={
              phase === "ready"
                ? `Running · v${status?.version ?? "0.1.0"}`
                : "Stopped or reconnecting"
            }
          />
          <SettingRow
            title="Background runtime"
            description="The harness is a separate process. Background operation is opt-in."
          >
            <Button
              disabled={!isTauri && phase !== "ready"}
              loading={action.busy}
              icon={<Power className="size-3.5" />}
              onClick={() => void action.run(phase === "ready" ? stop : start)}
            >
              {phase === "ready" ? "Stop" : "Start"}
            </Button>
            <Button
              disabled={!isTauri}
              loading={action.busy}
              icon={<RotateCw className="size-3.5" />}
              onClick={() => void action.run(restart, "Harness restarted")}
            >
              Restart
            </Button>
          </SettingRow>
        </Panel>
        <Panel>
          <PanelHeader title="General" />
          <div className="divide-y divide-border">
            {toggle(
              "general",
              "launch_on_startup",
              "Launch harness at login",
              "Register a local login item. No window is opened.",
            )}
            {toggle(
              "general",
              "minimise_to_tray",
              "Minimise to tray",
              "Closing the window keeps Magpie available in the system tray.",
              !isTauri,
            )}
            {toggle(
              "general",
              "keep_harness_running",
              "Keep harness running after quit",
              "External tools can continue executing while the desktop app is closed.",
              !isTauri,
            )}
            <SettingRow title="Appearance">
              <Select
                label="Theme"
                className="w-36"
                value={draft.general.theme}
                options={[
                  { value: "system", label: "System" },
                  { value: "dark", label: "Dark" },
                  { value: "light", label: "Light" },
                ]}
                onChange={(theme) =>
                  patch("general", {
                    theme: theme as Settings["general"]["theme"],
                  })
                }
              />
            </SettingRow>
            {toggle(
              "general",
              "reduced_motion",
              "Reduce motion",
              "The system accessibility preference is also respected.",
            )}
          </div>
        </Panel>
        <Panel>
          <PanelHeader title="Analytics and privacy" />
          <div className="divide-y divide-border">
            {toggle(
              "analytics",
              "local_history",
              "Keep local execution history",
              "Store metadata in the local SQLite database.",
            )}
            <SettingRow
              title="History retention"
              description="0 keeps all records; cleanup runs automatically."
            >
              <Input
                aria-label="Retention days"
                className="w-24"
                type="number"
                min={0}
                max={3650}
                value={draft.analytics.retention_days}
                onChange={(e) =>
                  patch("analytics", { retention_days: Number(e.target.value) })
                }
              />
              <span className="text-xs text-fg-subtle">days</span>
            </SettingRow>
            <SettingRow title="Telemetry refresh interval">
              <Select
                className="w-36"
                label="Refresh interval"
                value={String(draft.analytics.refresh_interval_secs)}
                onChange={(v) =>
                  patch("analytics", { refresh_interval_secs: Number(v) })
                }
                options={[5, 10, 30, 60].map((n) => ({
                  value: String(n),
                  label: `${n} seconds`,
                }))}
              />
            </SettingRow>
            {toggle(
              "analytics",
              "poll_provider_limits",
              "Refresh provider limits",
              "Adaptive monitoring only where supported; no model calls.",
            )}
            <SettingRow
              title="Daily spend notification"
              description="Notify when locally tracked API costs reach this USD amount."
            >
              <Input
                aria-label="Daily spend alert"
                className="w-28"
                type="number"
                min={0}
                step={0.1}
                placeholder="Disabled"
                value={draft.analytics.spend_alert_usd ?? ""}
                onChange={(e) =>
                  patch("analytics", {
                    spend_alert_usd: e.target.value
                      ? Number(e.target.value)
                      : null,
                  })
                }
              />
            </SettingRow>
            {toggle(
              "security",
              "retain_request_content",
              "Retain request and response content",
              "Off by default. When enabled, prompts and outputs are stored locally with future execution records.",
            )}
            {toggle(
              "security",
              "diagnostic_logging",
              "Diagnostic logging",
              "Verbose local diagnostics. Requires a harness restart.",
            )}
            {toggle(
              "security",
              "require_scopes",
              "Require read permissions",
              "Require an explicit read scope for telemetry endpoints. API authentication always remains enabled.",
            )}
            <SettingRow
              title="Credential storage"
              description={
                status?.secret_store === "file"
                  ? "Development file store is active; this is not an encrypted credential manager."
                  : "Provider credentials use the operating system credential manager."
              }
            >
              <span className="font-mono text-xs">
                {status?.secret_store ?? "Unavailable"}
              </span>
            </SettingRow>
            <SettingRow title="Usage history">
              <Button
                loading={action.busy}
                icon={<Download className="size-3.5" />}
                onClick={() =>
                  void action.run(async () => {
                    await saveTextFile(
                      "magpie-history.json",
                      await client!.exportUsage({
                        range: "custom",
                        from: 0,
                        to: Date.now(),
                        format: "json",
                      }),
                    );
                  })
                }
              >
                Export all
              </Button>
              <Button variant="danger" onClick={() => setClear(true)}>
                Clear history
              </Button>
            </SettingRow>
          </div>
        </Panel>
        <Panel>
          <PanelHeader title="Notifications" />
          <div className="divide-y divide-border">
            {toggle(
              "notifications",
              "enabled",
              "Enable notifications",
              "In-app notifications and desktop notifications while Magpie is open.",
            )}
            {(
              Object.keys(
                draft.notifications,
              ) as (keyof Settings["notifications"])[]
            )
              .filter((key) => key !== "enabled")
              .map((key) =>
                toggle(
                  "notifications",
                  key,
                  titleCase(key),
                  undefined,
                  !draft.notifications.enabled,
                ),
              )}
          </div>
        </Panel>
        <Panel>
          <PanelHeader
            title="Local API"
            subtitle="Changes take effect after restarting the harness"
          />
          <div className="divide-y divide-border">
            <SettingRow
              title="Port"
              description="Loopback interface only. Integration clients must use the same port."
            >
              <Input
                aria-label="API port"
                className="w-28"
                type="number"
                min={1024}
                max={65535}
                value={draft.server.port}
                onChange={(e) =>
                  patch("server", { port: Number(e.target.value) })
                }
              />
            </SettingRow>
            <SettingRow title="Request timeout">
              <Input
                aria-label="Request timeout seconds"
                className="w-28"
                type="number"
                min={1}
                max={3600}
                value={draft.server.request_timeout_secs}
                onChange={(e) =>
                  patch("server", {
                    request_timeout_secs: Number(e.target.value),
                  })
                }
              />
              <span className="text-xs text-fg-subtle">seconds</span>
            </SettingRow>
            <SettingRow title="Concurrent executions">
              <Input
                aria-label="Maximum concurrent executions"
                className="w-28"
                type="number"
                min={1}
                max={128}
                value={draft.server.max_concurrency}
                onChange={(e) =>
                  patch("server", { max_concurrency: Number(e.target.value) })
                }
              />
            </SettingRow>
          </div>
        </Panel>
        <Panel>
          <PanelHeader
            title="Application"
            subtitle={`Magpie ${info?.version ?? status?.version ?? "0.1.0"}`}
          />
          <div className="divide-y divide-border">
            {toggle(
              "general",
              "automatic_updates",
              "Automatically check for updates",
              "Checks signed releases on launch when an update channel is configured.",
              !isTauri,
            )}
            <SettingRow
              title="Software updates"
              description={
                update?.message ??
                "Installer updates require a configured signing key and release channel."
              }
            >
              <Button
                loading={action.busy}
                disabled={!isTauri}
                onClick={() =>
                  void action.run(async () =>
                    setUpdate(await checkForUpdates()),
                  )
                }
              >
                Check for updates
              </Button>
              {update?.available && (
                <Button
                  loading={action.busy}
                  onClick={() => void action.run(installUpdate)}
                >
                  Install {update.version}
                </Button>
              )}
            </SettingRow>
            {info && (
              <SettingRow title="Application data" description={info.data_dir}>
                <Button
                  icon={<FolderOpen className="size-3.5" />}
                  onClick={() =>
                    void action.run(() => revealPath(info.data_dir))
                  }
                >
                  Open folder
                </Button>
              </SettingRow>
            )}
          </div>
        </Panel>
      </div>
      <Dialog
        open={clear}
        onOpenChange={setClear}
        title="Clear all local history?"
        description="This permanently removes completed execution history and usage analytics. Provider credentials and routing preferences are retained."
        footer={
          <>
            <Button onClick={() => setClear(false)}>Cancel</Button>
            <Button
              variant="danger"
              loading={action.busy}
              onClick={() =>
                void action.run(
                  async () => {
                    await client!.clearHistory();
                    setClear(false);
                  },
                  "History cleared",
                  ["executions", "usage"],
                )
              }
            >
              Clear history
            </Button>
          </>
        }
      >
        <p className="text-xs text-fg-muted">
          Export your history first if you need a copy.
        </p>
      </Dialog>
    </Page>
  );
}
export function SettingsView() {
  const query = useSettings();
  return query.data ? (
    <SettingsEditor initial={query.data.settings} />
  ) : (
    <Page title="Settings">
      <QueryState
        pending={query.isLoading}
        error={query.error}
        retry={query.refetch}
      />
    </Page>
  );
}
