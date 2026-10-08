//! Opt-in bridge for Claude Code's documented statusLine integration.
//! It stores only reported quota fields in Magpie's private data directory.

use magpie_providers::cli::claude_usage::{UsageSnapshot, SNAPSHOT_FILE};
use magpie_security::secrets::write_private;
use magpie_store::paths::Paths;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::io::AsyncWriteExt;

const BINDING: &str = "claude-statusline.json";
pub const FLAG: &str = "--claude-statusline";

#[derive(Serialize, Deserialize)]
struct Binding {
    account_id: String,
    settings_path: PathBuf,
    command: String,
    previous: Option<Value>,
}

#[derive(Serialize)]
pub struct ReportingStatus {
    pub enabled: bool,
    pub account_id: Option<String>,
    pub settings_path: String,
}

pub fn settings_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        let path = PathBuf::from(path);
        let path = if path.is_absolute() { path } else { std::env::current_dir().map_err(|e| e.to_string())?.join(path) };
        return Ok(path.join("settings.json"));
    }
    let home = if cfg!(windows) { std::env::var_os("USERPROFILE") } else { std::env::var_os("HOME") };
    Ok(PathBuf::from(home.ok_or("Home directory is unavailable")?).join(".claude/settings.json"))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Settings file is too large".into());
    }
    serde_json::from_reader(file.take(1024 * 1024 + 1)).map_err(|_| "Settings contain invalid JSON; no changes made".into())
}

fn binding(paths: &Paths) -> Result<Option<Binding>, String> {
    let path = paths.root.join(BINDING);
    if !path.exists() {
        return Ok(None);
    }
    serde_json::from_value(read_json(&path)?).map(Some).map_err(|_| "Usage reporting configuration is invalid".into())
}

fn read_settings(path: &Path) -> Result<Value, String> {
    let value = if path.exists() { read_json(path)? } else { json!({}) };
    if !value.is_object() {
        return Err("Claude settings must be a JSON object; no changes made".into());
    }
    Ok(value)
}

/// Write a complete private file before replacing the destination.
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    // Keep dotfile-manager symlinks intact when updating Claude settings.
    let resolved = if path.is_symlink() { Some(path.canonicalize().map_err(|e| e.to_string())?) } else { None };
    let path = resolved.as_deref().unwrap_or(path);
    let parent = path.parent().ok_or("Missing parent directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    // Windows clock resolution can give concurrent writes the same timestamp.
    let temp = parent.join(format!(".{}.tmp", magpie_core::new_id("magpie-statusline")));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let result = write_private(&temp, &bytes).and_then(|_| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn quote(path: &Path) -> Result<String, String> {
    let s = path.to_str().ok_or("Installation path is not valid Unicode")?;
    Ok(format!("'{}'", s.replace('\'', "'\\''")))
}

#[cfg(windows)]
fn powershell_script(script: &str) -> String {
    use base64::Engine;
    let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn bridge_command(executable: &Path, root: &Path) -> Result<String, String> {
    #[cfg(windows)]
    {
        // Explicit PowerShell works under both of Claude's Windows shells.
        // EncodedCommand avoids a second layer of shell interpolation.
        let literal =
            |p: &Path| p.to_str().map(|s| format!("'{}'", s.replace('\\', "/").replace('\'', "''"))).ok_or("Path is not valid Unicode");
        let script = format!("$OutputEncoding=[System.Text.UTF8Encoding]::new($false); [Console]::InputEncoding=$OutputEncoding; [Console]::OutputEncoding=$OutputEncoding; $input | & {} '{FLAG}' {}; exit $LASTEXITCODE", literal(executable)?, literal(root)?);
        Ok(format!("powershell.exe -NoProfile -NonInteractive -EncodedCommand {} # {FLAG}", powershell_script(&script)))
    }
    #[cfg(not(windows))]
    {
        let appimage = executable.extension().is_some_and(|e| e == "AppImage");
        Ok(format!("{} {}{FLAG} {}", quote(executable)?, if appimage { "--appimage-extract-and-run " } else { "" }, quote(root)?))
    }
}

fn status_shell(command: &str) -> tokio::process::Command {
    #[cfg(windows)]
    {
        let mut candidates: Vec<PathBuf> = std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH").map(PathBuf::from).into_iter().collect();
        if let Some(git) = magpie_providers::cli::find_binary("git", None) {
            candidates.extend(git.ancestors().skip(1).take(4).map(|p| p.join("bin/bash.exe")));
        }
        if let Some(bash) = candidates.into_iter().find(|p| p.is_file()) {
            let mut c = tokio::process::Command::new(bash);
            c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
            c.args(["-c", command]);
            return c;
        }
        let mut c = tokio::process::Command::new("powershell.exe");
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let script = format!("$OutputEncoding=[System.Text.UTF8Encoding]::new($false); [Console]::InputEncoding=$OutputEncoding; [Console]::OutputEncoding=$OutputEncoding; {command}");
        c.args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &powershell_script(&script)]);
        c
    }
    #[cfg(not(windows))]
    {
        let shell = magpie_providers::cli::find_binary("bash", None).unwrap_or_else(|| PathBuf::from("sh"));
        let mut c = tokio::process::Command::new(shell);
        c.args(["-c", command]);
        c
    }
}

pub fn status(paths: &Paths, settings: &Path) -> Result<ReportingStatus, String> {
    let saved = binding(paths)?;
    let current = read_settings(settings)?;
    let enabled =
        saved.as_ref().is_some_and(|b| b.settings_path == settings && current["statusLine"]["command"].as_str() == Some(&b.command));
    Ok(ReportingStatus { enabled, account_id: saved.map(|b| b.account_id), settings_path: settings.display().to_string() })
}

pub fn enable(paths: &Paths, settings: &Path, executable: &Path, account_id: &str) -> Result<ReportingStatus, String> {
    paths.ensure().map_err(|e| e.to_string())?;
    if let Some(saved) = binding(paths)? {
        if saved.account_id != account_id {
            return Err("Disable usage reporting for the other Claude connection first".into());
        }
        let current = status(paths, settings)?;
        if current.enabled {
            return Ok(current);
        }
        return Err(
            "Claude's status line changed outside Magpie. Disable reporting before reconnecting it; your current status line will be kept."
                .into(),
        );
    }
    let mut value = read_settings(settings)?;
    if value["disableAllHooks"] == true || value["allowManagedHooksOnly"] == true {
        return Err("Claude settings currently prohibit a user status line. Review disableAllHooks/allowManagedHooksOnly in Claude before enabling reporting.".into());
    }
    let previous = value.get("statusLine").cloned();
    if let Some(p) = &previous {
        if !p.is_null() && (p["type"] != "command" || !p["command"].is_string()) {
            return Err("Unsupported existing Claude status line; no changes made".into());
        }
        if p["command"].as_str().is_some_and(|c| c.contains(FLAG)) {
            return Err("A Magpie usage bridge is already configured in Claude".into());
        }
    }
    let root =
        if paths.root.is_absolute() { paths.root.clone() } else { std::env::current_dir().map_err(|e| e.to_string())?.join(&paths.root) };
    let command = bridge_command(executable, &root)?;
    let mut line = previous.clone().filter(Value::is_object).unwrap_or_else(|| json!({}));
    line["type"] = json!("command");
    line["command"] = json!(command);
    value["statusLine"] = line;
    let saved = Binding { account_id: account_id.into(), settings_path: settings.to_path_buf(), command, previous };
    atomic_json(&paths.root.join(BINDING), &saved)?;
    if let Err(e) = atomic_json(settings, &value) {
        let _ = std::fs::remove_file(paths.root.join(BINDING));
        return Err(e);
    }
    status(paths, settings)
}

pub fn disable(paths: &Paths, account_id: &str) -> Result<ReportingStatus, String> {
    let Some(saved) = binding(paths)? else {
        return status(paths, &settings_path()?);
    };
    if saved.account_id != account_id {
        return Err("Usage reporting belongs to another Claude connection".into());
    }
    let mut value = read_settings(&saved.settings_path)?;
    // Preserve edits made by the user since installation.
    if value["statusLine"]["command"].as_str() == Some(&saved.command) {
        match saved.previous {
            Some(previous) => {
                value["statusLine"] = previous;
            }
            None => {
                value.as_object_mut().unwrap().remove("statusLine");
            }
        }
        atomic_json(&saved.settings_path, &value)?;
    }
    std::fs::remove_file(paths.root.join(BINDING)).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(paths.root.join(SNAPSHOT_FILE));
    status(paths, &saved.settings_path)
}

/// Invoked by Claude, before any Tauri/GUI runtime starts. Never calls a model.
pub async fn run(paths: &Paths) -> Result<(), String> {
    if std::env::var_os("MAGPIE_STATUSLINE_ACTIVE").is_some() {
        return Err("Recursive Magpie status line detected; reconnect usage reporting".into());
    }
    let saved = binding(paths)?.ok_or("Usage reporting is disabled")?;
    let mut input = Vec::new();
    std::io::stdin().take(1024 * 1024 + 1).read_to_end(&mut input).map_err(|e| e.to_string())?;
    if input.len() > 1024 * 1024 {
        return Err("Status-line input is too large".into());
    }
    let value: Value = serde_json::from_slice(&input).map_err(|_| "Invalid status-line input")?;
    let snapshot = UsageSnapshot::from_statusline(&saved.account_id, &value);
    if let Some(s) = &snapshot {
        // Cache failure must not break an existing status line.
        let _ = atomic_json(&paths.root.join(SNAPSHOT_FILE), s);
    }
    if let Some(command) = saved.previous.as_ref().and_then(|p| p["command"].as_str()).filter(|c| !c.is_empty()) {
        let mut process = status_shell(command);
        process.env("MAGPIE_STATUSLINE_ACTIVE", "1");
        let mut child = process
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| e.to_string())?;
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            if let Some(mut stdin) = child.stdin.take() {
                // Some existing status lines intentionally ignore stdin.
                let _ = stdin.write_all(&input).await;
            }
            child.wait().await
        })
        .await
        .map_err(|_| "Previous status line timed out")?
        .map_err(|e| e.to_string())?;
    } else if let Some(s) = snapshot {
        let parts: Vec<_> = s.limits().iter().filter_map(|w| w.used_percent.map(|p| format!("{}: {p:.0}% used", w.label))).collect();
        println!("Claude | {}", parts.join(" | "));
    } else {
        println!("Claude | waiting for reported allowance");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Paths, PathBuf) {
        let root = std::env::temp_dir().join(magpie_core::new_id("magpie-statusline-test"));
        (Paths::at(root.join("magpie")), root.join("claude/settings.json"))
    }

    #[test]
    fn preserves_existing_statusline_and_unrelated_settings() {
        let (paths, settings) = fixture();
        let original = json!({"statusLine":{"type":"command","command":"echo original","padding":2},"theme":"dark"});
        atomic_json(&settings, &original).unwrap();
        assert!(enable(&paths, &settings, Path::new("/test path/magpie"), "account").unwrap().enabled);
        assert!(enable(&paths, &settings, Path::new("/test path/magpie"), "other").is_err());
        let mut changed = read_settings(&settings).unwrap();
        assert_eq!(changed["statusLine"]["padding"], 2);
        changed["theme"] = json!("light");
        atomic_json(&settings, &changed).unwrap();
        assert!(!disable(&paths, "account").unwrap().enabled);
        let restored = read_settings(&settings).unwrap();
        assert_eq!(restored["statusLine"], original["statusLine"]);
        assert_eq!(restored["theme"], "light");
        std::fs::remove_dir_all(paths.root.parent().unwrap()).unwrap();
    }

    #[test]
    fn user_edits_and_invalid_settings_are_never_overwritten() {
        let (paths, settings) = fixture();
        enable(&paths, &settings, Path::new("/test/magpie"), "account").unwrap();
        let edited = json!({"statusLine":{"type":"command","command":"echo changed"}});
        atomic_json(&settings, &edited).unwrap();
        disable(&paths, "account").unwrap();
        assert_eq!(read_settings(&settings).unwrap(), edited);
        std::fs::write(&settings, b"invalid json").unwrap();
        assert!(enable(&paths, &settings, Path::new("/test/magpie"), "account").is_err());
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), "invalid json");
        std::fs::remove_dir_all(paths.root.parent().unwrap()).unwrap();
    }

    #[test]
    fn respects_hook_restrictions() {
        let (paths, settings) = fixture();
        let original = json!({"disableAllHooks":true});
        atomic_json(&settings, &original).unwrap();
        assert!(enable(&paths, &settings, Path::new("/test/magpie"), "account").is_err());
        assert_eq!(read_settings(&settings).unwrap(), original);
        assert!(binding(&paths).unwrap().is_none());
        std::fs::remove_dir_all(paths.root.parent().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn settings_symlink_is_preserved() {
        let (paths, settings) = fixture();
        let target = settings.with_file_name("dotfile.json");
        atomic_json(&target, &json!({"theme":"dark"})).unwrap();
        std::os::unix::fs::symlink(&target, &settings).unwrap();
        enable(&paths, &settings, Path::new("/test/magpie"), "account").unwrap();
        assert!(settings.is_symlink());
        disable(&paths, "account").unwrap();
        assert!(settings.is_symlink());
        assert_eq!(read_json(&target).unwrap(), json!({"theme":"dark"}));
        std::fs::remove_dir_all(paths.root.parent().unwrap()).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_bridge_command_encodes_paths_and_unicode() {
        use base64::Engine;
        let command = bridge_command(Path::new("C:/O'Brien & tools/Magpie.exe"), Path::new("C:/Users/テスト/Magpie")).unwrap();
        assert!(command.ends_with(FLAG));
        let encoded = command.split("-EncodedCommand ").nth(1).unwrap().split_whitespace().next().unwrap();
        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).unwrap();
        let units: Vec<_> = bytes.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        let decoded = String::from_utf16(&units).unwrap();
        assert!(decoded.contains("'C:/O''Brien & tools/Magpie.exe'"));
        assert!(decoded.contains("'C:/Users/テスト/Magpie'"));
        assert!(decoded.contains("$input | &"));
    }
}
