//! Start-on-login registration for the harness process, using each
//! platform's per-user mechanism (no elevated privileges).

use std::path::{Path, PathBuf};

use magpie_core::*;

const ID: &str = "dev.magpie.harness";

/// Registry checks run when the UI reads settings, including during navigation.
/// A detached harness must never create a console for these background commands.
#[cfg(target_os = "windows")]
fn registry_command() -> std::process::Command {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    let mut command = std::process::Command::new("reg.exe");
    command.creation_flags(0x0800_0000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()); // CREATE_NO_WINDOW
    command
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

#[cfg(target_os = "linux")]
fn entry_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home().map(|h| h.join(".config")))?;
    Some(base.join("autostart").join("magpie-harness.desktop"))
}

#[cfg(target_os = "macos")]
fn entry_path() -> Option<PathBuf> {
    home().map(|h| h.join("Library/LaunchAgents").join(format!("{ID}.plist")))
}

#[cfg(target_os = "windows")]
fn entry_path() -> Option<PathBuf> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn entry_path() -> Option<PathBuf> {
    None
}

fn quote(s: &str) -> String {
    if s.contains(' ') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

pub fn enable(program: &Path, args: &[String]) -> HarnessResult<()> {
    let prog = program.display().to_string();
    #[cfg(target_os = "windows")]
    {
        let cmdline = std::iter::once(quote(&prog)).chain(args.iter().map(|a| quote(a))).collect::<Vec<_>>().join(" ");
        let status = registry_command()
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "MagpieHarness",
                "/t",
                "REG_SZ",
                "/d",
                &cmdline,
                "/f",
            ])
            .status()
            .map_err(|e| HarnessError::internal(e.to_string()))?;
        return if status.success() { Ok(()) } else { Err(HarnessError::internal("Could not register login item")) };
    }
    #[allow(unreachable_code)]
    {
        let path = entry_path().ok_or_else(|| HarnessError::invalid("Start on login is not supported on this platform"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| HarnessError::internal(e.to_string()))?;
        }
        let content = if cfg!(target_os = "macos") {
            let mut items = format!("    <string>{}</string>\n", xml_escape(&prog));
            for a in args {
                items.push_str(&format!("    <string>{}</string>\n", xml_escape(a)));
            }
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{ID}</string>\n  <key>ProgramArguments</key>\n  <array>\n{items}  </array>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>ProcessType</key>\n  <string>Background</string>\n</dict>\n</plist>\n"
            )
        } else {
            let exec = std::iter::once(quote(&prog)).chain(args.iter().map(|a| quote(a))).collect::<Vec<_>>().join(" ");
            format!(
                "[Desktop Entry]\nType=Application\nName=Magpie Harness\nComment=Local AI harness\nExec={exec}\nTerminal=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n"
            )
        };
        std::fs::write(&path, content).map_err(|e| HarnessError::internal(e.to_string()))
    }
}

pub fn disable() -> HarnessResult<()> {
    #[cfg(target_os = "windows")]
    {
        let _ = registry_command()
            .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "MagpieHarness", "/f"])
            .status();
        return Ok(());
    }
    #[allow(unreachable_code)]
    {
        if let Some(p) = entry_path() {
            match std::fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(HarnessError::internal(e.to_string())),
            }
        }
        Ok(())
    }
}

pub fn is_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        return registry_command()
            .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "MagpieHarness"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
    }
    #[allow(unreachable_code)]
    entry_path().map(|p| p.exists()).unwrap_or(false)
}

#[allow(dead_code)]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
