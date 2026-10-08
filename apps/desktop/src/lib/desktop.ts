// Bridge to the native shell. In a plain browser (development preview) the
// functions degrade gracefully.

import type { ProviderKind } from "@magpie/sdk";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export interface HarnessConnection {
  url: string;
  token: string;
  version: string;
  api_version: number;
  launched: boolean;
}

export interface AppInfo {
  version: string;
  platform: string;
  data_dir: string;
  log_file: string;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

/** Connect to the harness, starting it when it is not running. */
export async function connectHarness(start = true): Promise<HarnessConnection> {
  if (isTauri) return invoke<HarnessConnection>("harness_connection", { start });
  const url = import.meta.env.VITE_MAGPIE_URL ?? "http://127.0.0.1:7878";
  const token = import.meta.env.VITE_MAGPIE_TOKEN;
  if (!token) throw new Error("Set VITE_MAGPIE_TOKEN in .env.local to preview outside the desktop app.");
  const response = await fetch(`${url}/health`, {
    signal: AbortSignal.timeout(5000),
  });
  if (!response.ok) throw new Error("Harness health check failed");
  const health = await response.json();
  if (health.product !== "Magpie" || health.api_version !== 1) throw new Error("Incompatible harness version");
  return {
    url,
    token,
    version: health.version,
    api_version: health.api_version,
    launched: false,
  };
}

export async function stopHarness(): Promise<void> {
  if (isTauri) await invoke("harness_stop");
}

export async function restartHarness(): Promise<HarnessConnection> {
  if (isTauri) return invoke<HarnessConnection>("harness_restart");
  return connectHarness();
}

export async function appInfo(): Promise<AppInfo | null> {
  if (!isTauri) return null;
  return invoke<AppInfo>("app_info");
}

export interface ClaudeUsageReporting {
  enabled: boolean;
  account_id?: string | null;
  settings_path: string;
}

export async function claudeUsageStatus(): Promise<ClaudeUsageReporting> {
  return invoke<ClaudeUsageReporting>("claude_usage_status");
}

export async function setClaudeUsageReporting(accountId: string, enabled: boolean): Promise<ClaudeUsageReporting> {
  return invoke<ClaudeUsageReporting>("set_claude_usage_reporting", { accountId, enabled });
}

/** Open the provider's official sign-in command in a terminal window. */
export async function openCliLogin(kind: ProviderKind): Promise<void> {
  if (!isTauri) throw new Error("Terminal sign-in is only available in the desktop app.");
  await invoke("open_cli_login", { kind });
}

export async function setWindowBehaviour(minimiseToTray: boolean, keepHarnessRunning: boolean): Promise<void> {
  if (isTauri)
    await invoke("set_window_behaviour", {
      minimiseToTray,
      keepHarnessRunning,
    });
}

export async function openExternal(url: string): Promise<void> {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener");
  }
}

export async function revealPath(path: string): Promise<void> {
  if (isTauri) {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  }
}

/** Save text to a user-chosen file. Returns the path, or null if cancelled. */
export async function saveTextFile(defaultName: string, contents: string): Promise<string | null> {
  if (isTauri) {
    return invoke<string | null>("save_text_file", { defaultName, contents });
  }
  const blob = new Blob([contents], { type: "text/plain" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = defaultName;
  a.click();
  URL.revokeObjectURL(a.href);
  return defaultName;
}

export async function notifyOs(title: string, body: string): Promise<void> {
  if (!isTauri) return;
  try {
    const n = await import("@tauri-apps/plugin-notification");
    let granted = await n.isPermissionGranted();
    if (!granted) granted = (await n.requestPermission()) === "granted";
    if (granted) n.sendNotification({ title, body });
  } catch {
    /* notifications unavailable */
  }
}

export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

export interface UpdateInfo {
  configured: boolean;
  available: boolean;
  version?: string;
  message: string;
}
export async function checkForUpdates(): Promise<UpdateInfo> {
  if (!isTauri)
    return {
      configured: false,
      available: false,
      message: "Updates are available in the desktop application.",
    };
  return invoke<UpdateInfo>("check_for_updates");
}
export async function installUpdate(): Promise<void> {
  if (!isTauri) throw new Error("Updates require the desktop application.");
  await invoke("install_update");
}
