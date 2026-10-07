#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use magpie_core::{ProviderKind, API_VERSION, VERSION};
use magpie_runtime::{paths::Paths, Connection};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

#[derive(Default)]
struct DesktopState {
    minimise: AtomicBool,
    keep_running: AtomicBool,
    exiting: AtomicBool,
    tray_available: AtomicBool,
    lifecycle: tokio::sync::Mutex<()>,
}

#[derive(Serialize)]
struct HarnessConnection {
    url: String,
    token: String,
    version: String,
    api_version: u32,
    launched: bool,
}
fn connection_value(c: Connection, launched: bool) -> Result<HarnessConnection, String> {
    if c.info.api_version != API_VERSION {
        return Err(format!(
            "Harness API {} is incompatible with desktop API {}. Stop the other harness and restart Magpie.",
            c.info.api_version, API_VERSION
        ));
    }
    Ok(HarnessConnection { url: c.url, token: c.token, version: c.info.version, api_version: c.info.api_version, launched })
}

fn launcher() -> Result<PathBuf, String> {
    // AppImage's extracted executable disappears on exit. Relaunch the durable image.
    #[cfg(target_os = "linux")]
    if let Some(path) = std::env::var_os("APPIMAGE") {
        return Ok(PathBuf::from(path));
    }
    std::env::current_exe().map_err(|e| e.to_string())
}

async fn connect(start: bool) -> Result<HarnessConnection, String> {
    let paths = Paths::resolve();
    if let Some(c) = magpie_runtime::discover(&paths).await {
        return connection_value(c, false);
    }
    if !start {
        return Err("Harness is not running".into());
    }
    let c = magpie_runtime::ensure_running(&paths, &launcher()?, &["--harness".into()], Duration::from_secs(25))
        .await
        .map_err(|e| e.to_string())?;
    connection_value(c, true)
}

#[tauri::command]
async fn harness_connection(start: bool, state: tauri::State<'_, DesktopState>) -> Result<HarnessConnection, String> {
    let _guard = state.lifecycle.lock().await;
    connect(start).await
}
async fn stop() -> Result<(), String> {
    if let Some(c) = magpie_runtime::discover(&Paths::resolve()).await {
        magpie_runtime::stop(&c).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
async fn harness_stop(state: tauri::State<'_, DesktopState>) -> Result<(), String> {
    let _guard = state.lifecycle.lock().await;
    stop().await
}
#[tauri::command]
async fn harness_restart(state: tauri::State<'_, DesktopState>) -> Result<HarnessConnection, String> {
    let _guard = state.lifecycle.lock().await;
    stop().await?;
    connect(true).await
}

#[tauri::command]
fn set_window_behaviour(minimise_to_tray: bool, keep_harness_running: bool, state: tauri::State<'_, DesktopState>) {
    state.minimise.store(minimise_to_tray, Ordering::Relaxed);
    state.keep_running.store(keep_harness_running, Ordering::Relaxed);
}
#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    platform: &'static str,
    data_dir: String,
    log_file: String,
}
#[tauri::command]
fn app_info() -> AppInfo {
    let paths = Paths::resolve();
    AppInfo {
        version: VERSION,
        platform: std::env::consts::OS,
        data_dir: paths.root.display().to_string(),
        log_file: paths.logs_dir().join("harness.log").display().to_string(),
    }
}

#[tauri::command]
async fn save_text_file(app: tauri::AppHandle, default_name: String, contents: String) -> Result<Option<String>, String> {
    if contents.len() > 128 * 1024 * 1024 {
        return Err("Export is too large; choose a shorter range".into());
    }
    // A webview never supplies the destination path. The native chooser authorises it.
    let name = std::path::Path::new(&default_name).file_name().and_then(|n| n.to_str()).unwrap_or("magpie-export.json").to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app.dialog().file().set_file_name(name).blocking_save_file() else { return Ok(None) };
        let path = file.into_path().map_err(|e| e.to_string())?;
        magpie_security::secrets::write_private(&path, contents.as_bytes()).map_err(|e| e.to_string())?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn login_command(kind: ProviderKind) -> Result<(&'static str, Vec<&'static str>), String> {
    match kind {
        ProviderKind::CodexCli => Ok(("codex", vec!["login"])),
        ProviderKind::ClaudeCode => Ok(("claude", vec!["auth", "login"])),
        ProviderKind::GeminiCli => Ok(("gemini", vec![])),
        _ => Err("Only official CLI login commands are supported".into()),
    }
}
#[tauri::command]
async fn open_cli_login(kind: ProviderKind) -> Result<(), String> {
    let (name, args) = login_command(kind)?;
    let bin = magpie_providers::cli::find_binary(name, None).ok_or_else(|| format!("Install the official {name} CLI first"))?;
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let choices = [("x-terminal-emulator", "-e"), ("gnome-terminal", "--"), ("konsole", "-e"), ("xterm", "-e")];
            for (terminal, flag) in choices {
                if let Some(program) = magpie_providers::cli::find_binary(terminal, None) {
                    let mut command = std::process::Command::new(program);
                    command.arg(flag).arg(&bin).args(&args);
                    // npm CLIs need their accompanying node binary on PATH.
                    let mut paths = vec![bin.parent().unwrap_or(std::path::Path::new("/usr/bin")).to_path_buf()];
                    if let Some(path) = std::env::var_os("PATH") {
                        paths.extend(std::env::split_paths(&path));
                    }
                    command.env("PATH", std::env::join_paths(paths).map_err(|e| e.to_string())?);
                    command.spawn().map_err(|e| e.to_string())?;
                    return Ok(());
                }
            }
            Err("No supported terminal was found. Run the displayed sign-in command in your terminal.".into())
        }
        #[cfg(target_os = "macos")]
        {
            let escaped = format!("'{}'", bin.display().to_string().replace('\'', "'\\''"));
            let command = format!("{} {}", escaped, args.join(" "));
            let script =
                format!("tell application \"Terminal\" to do script {}", serde_json::to_string(&command).map_err(|e| e.to_string())?);
            let status = std::process::Command::new("osascript")
                .args(["-e", &script, "-e", "tell application \"Terminal\" to activate"])
                .status()
                .map_err(|e| e.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Could not open Terminal".into())
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Fixed command names/arguments only; no user-controlled shell fragments.
            std::process::Command::new("cmd.exe")
                .args(["/c", "start", "Magpie sign-in", "cmd.exe", "/k", name])
                .args(args)
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct UpdateInfo {
    configured: bool,
    available: bool,
    version: Option<String>,
    message: String,
}
fn update_configured() -> bool {
    option_env!("MAGPIE_UPDATE_PUBLIC_KEY").is_some_and(|s| !s.is_empty())
        && option_env!("MAGPIE_UPDATE_URL").is_some_and(|s| s.starts_with("https://"))
}
#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    if !update_configured() {
        return Ok(UpdateInfo {
            configured: false,
            available: false,
            version: None,
            message: "This build has no signed update channel configured. Install newer versions manually.".into(),
        });
    }
    let update = app
        .updater_builder()
        .endpoints(vec![option_env!("MAGPIE_UPDATE_URL").unwrap_or_default().parse().map_err(|_| "Invalid update URL".to_string())?])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(UpdateInfo {
        configured: true,
        available: update.is_some(),
        version: update.as_ref().map(|u| u.version.clone()),
        message: update.as_ref().map(|u| format!("Version {} is available", u.version)).unwrap_or_else(|| "You are up to date".into()),
    })
}
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    if !update_configured() {
        return Err("No signed update channel configured".into());
    }
    let Some(update) = app
        .updater_builder()
        .endpoints(vec![option_env!("MAGPIE_UPDATE_URL").unwrap_or_default().parse().map_err(|_| "Invalid update URL".to_string())?])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Err("No update available".into());
    };
    stop().await?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart();
}

fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
fn request_exit(app: tauri::AppHandle) {
    let state = app.state::<DesktopState>();
    if state.exiting.swap(true, Ordering::SeqCst) {
        return;
    }
    let keep = state.keep_running.load(Ordering::Relaxed);
    tauri::async_runtime::spawn(async move {
        if !keep {
            if let Err(error) = stop().await {
                eprintln!("{}", magpie_security::redact(&error));
            }
        }
        app.exit(0);
    });
}

fn main() {
    // Daemon mode never initialises a webview, tray, display connection or UI runtime.
    if std::env::args().any(|a| a == "--harness") {
        let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime");
        let result = rt.block_on(magpie_runtime::run_daemon(magpie_runtime::DaemonOptions {
            launch_command: launcher().ok().map(|p| (p, vec!["--harness".into()])),
            port: None,
        }));
        if let Err(error) = result {
            eprintln!("{}", magpie_security::redact(&error.to_string()));
            std::process::exit(1);
        }
        return;
    }
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(DesktopState::default())
        .invoke_handler(tauri::generate_handler![
            harness_connection,
            harness_stop,
            harness_restart,
            set_window_behaviour,
            app_info,
            save_text_file,
            open_cli_login,
            check_for_updates,
            install_update
        ])
        .setup(|app| {
            if update_configured() {
                app.handle().plugin(
                    tauri_plugin_updater::Builder::new().pubkey(option_env!("MAGPIE_UPDATE_PUBLIC_KEY").unwrap_or_default()).build(),
                )?;
                // The URL is a release-time constant, never user supplied.
            }
            // Load preferences before any close event can arrive.
            if let Ok(store) = magpie_store::Store::open(&Paths::resolve().database()) {
                if let Ok(settings) = store.settings() {
                    let state = app.state::<DesktopState>();
                    state.minimise.store(settings.general.minimise_to_tray, Ordering::Relaxed);
                    state.keep_running.store(settings.general.keep_harness_running, Ordering::Relaxed);
                }
            }
            let open = MenuItem::with_id(app, "open", "Open Magpie", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Magpie", true, None::<&str>)?;
            let stop_and_quit = MenuItem::with_id(app, "stop-quit", "Stop harness and quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &stop_and_quit, &quit])?;
            let mut tray = TrayIconBuilder::new().tooltip("Magpie · local AI harness").menu(&menu).on_menu_event(|app, event| match event
                .id
                .as_ref()
            {
                "open" => show(app),
                "quit" => request_exit(app.clone()),
                "stop-quit" => {
                    app.state::<DesktopState>().keep_running.store(false, Ordering::Relaxed);
                    request_exit(app.clone());
                }
                _ => {}
            });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            let available = tray.build(app).is_ok();
            app.state::<DesktopState>().tray_available.store(available, Ordering::Relaxed);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<DesktopState>();
                api.prevent_close();
                if state.minimise.load(Ordering::Relaxed) && state.tray_available.load(Ordering::Relaxed) {
                    let _ = window.hide();
                } else {
                    request_exit(window.app_handle().clone());
                }
            }
        });
    let app = builder.build(tauri::generate_context!()).expect("Could not initialise Magpie");
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !app.state::<DesktopState>().exiting.load(Ordering::SeqCst) {
                api.prevent_exit();
                request_exit(app.clone());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_rejects_non_cli_provider() {
        assert!(login_command(ProviderKind::OpenAi).is_err());
    }
    #[test]
    fn login_uses_fixed_commands() {
        assert_eq!(login_command(ProviderKind::CodexCli).unwrap(), ("codex", vec!["login"]));
    }
}
