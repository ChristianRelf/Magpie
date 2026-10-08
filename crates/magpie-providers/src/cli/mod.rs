//! Adapters that delegate to official provider CLIs. The CLI owns
//! authentication; Magpie never reads or stores its credentials and only
//! invokes documented, non-interactive interfaces.

pub mod claude_code;
pub mod claude_usage;
pub mod codex;
pub mod gemini_cli;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use magpie_core::*;
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio_util::sync::CancellationToken;

/// Locate a CLI binary: explicit override, PATH, then common install
/// locations (the harness may run from a login item with a minimal PATH).
pub fn find_binary(name: &str, override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = override_path.filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    if let Ok(p) = which::which(name) {
        return Some(p);
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = &home {
        for rel in [".local/bin", ".npm-global/bin", ".bun/bin", ".volta/bin", ".claude/local", ".yarn/bin", "bin", ".cargo/bin"] {
            candidates.push(h.join(rel));
        }
        // nvm installs: ~/.nvm/versions/node/<v>/bin
        if let Ok(rd) = std::fs::read_dir(h.join(".nvm/versions/node")) {
            let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("bin")).collect();
            dirs.sort();
            dirs.reverse();
            candidates.extend(dirs);
        }
    }
    for p in ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/snap/bin"] {
        candidates.push(PathBuf::from(p));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("npm"));
    }
    for dir in candidates {
        for file in exe_names(name) {
            let p = dir.join(&file);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn exe_names(name: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![format!("{name}.exe"), format!("{name}.cmd"), name.to_string()]
    } else {
        vec![name.to_string()]
    }
}

/// PATH with the binary's directory prepended, so npm-installed CLIs find
/// the `node` runtime installed alongside them.
fn augmented_path(bin: &Path) -> std::ffi::OsString {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(dir) = bin.parent() {
        paths.push(dir.to_path_buf());
    }
    if let Some(existing) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    for extra in ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin"] {
        let p = PathBuf::from(extra);
        if !paths.contains(&p) {
            paths.push(p);
        }
    }
    std::env::join_paths(paths).unwrap_or_default()
}

/// Build a command for a CLI with a sanitised environment.
pub fn command(bin: &Path, remove_env: &[&str]) -> Command {
    let mut cmd = Command::new(bin);
    cmd.env("PATH", augmented_path(bin)).env("NO_COLOR", "1").env("CI", "1").kill_on_drop(true);
    for k in remove_env {
        cmd.env_remove(k);
    }
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Status of an installed CLI, for the connect flow.
#[derive(Debug, Clone, Serialize)]
pub struct CliStatus {
    pub kind: ProviderKind,
    pub binary: String,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub install_command: Option<String>,
    pub login_command: Option<String>,
}

/// Detect an installed provider CLI and its version.
pub async fn detect(kind: ProviderKind) -> CliStatus {
    let d = kind.descriptor();
    let bin_name = d.cli_binary.unwrap_or_default();
    let path = find_binary(bin_name, None);
    let mut version = None;
    if let Some(p) = &path {
        if let Ok(out) = run_capture(p, &["--version"], &[], Duration::from_secs(15), None).await {
            version = out.stdout.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty());
        }
    }
    CliStatus {
        kind,
        binary: bin_name.to_string(),
        installed: path.is_some(),
        path: path.map(|p| p.display().to_string()),
        version,
        install_command: d.cli_install.map(str::to_string),
        login_command: d.cli_login.map(str::to_string),
    }
}

pub struct Captured {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Run a command to completion with a timeout.
pub async fn run_capture(bin: &Path, args: &[&str], remove_env: &[&str], timeout: Duration, cwd: Option<&Path>) -> HarnessResult<Captured> {
    let mut cmd = command(bin, remove_env);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    let child = cmd.spawn().map_err(|e| spawn_error(bin, e))?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(Captured {
            status: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: magpie_security::redact(&String::from_utf8_lossy(&out.stderr)),
        }),
        Ok(Err(e)) => Err(HarnessError::new(ErrorKind::LocalDependency, e.to_string())),
        Err(_) => Err(HarnessError::new(ErrorKind::Timeout, format!("{} did not respond in time", bin.display()))),
    }
}

pub fn spawn_error(bin: &Path, e: std::io::Error) -> HarnessError {
    HarnessError::new(ErrorKind::LocalDependency, format!("Could not start {}: {e}", bin.display()))
}

/// A running CLI process emitting JSON lines on stdout.
pub struct JsonlProcess {
    child: Child,
    lines: tokio::io::Lines<BufReader<ChildStdout>>,
    stderr: Arc<tokio::sync::Mutex<String>>,
}

impl JsonlProcess {
    pub async fn spawn(mut cmd: Command, stdin_data: Option<String>) -> HarnessResult<Self> {
        cmd.stdin(if stdin_data.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
        let program = cmd.as_std().get_program().to_owned();
        let mut child = cmd.spawn().map_err(|e| spawn_error(Path::new(&program), e))?;
        if let Some(data) = stdin_data {
            if let Some(mut stdin) = child.stdin.take() {
                tokio::spawn(async move {
                    let _ = stdin.write_all(data.as_bytes()).await;
                    let _ = stdin.shutdown().await;
                });
            }
        }
        let stdout = child.stdout.take().ok_or_else(|| HarnessError::internal("no stdout"))?;
        let stderr_buf = Arc::new(tokio::sync::Mutex::new(String::new()));
        if let Some(mut stderr) = child.stderr.take() {
            let buf = stderr_buf.clone();
            tokio::spawn(async move {
                let mut chunk = [0u8; 4096];
                while let Ok(n) = stderr.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }
                    let mut b = buf.lock().await;
                    if b.len() < 64 * 1024 {
                        b.push_str(&String::from_utf8_lossy(&chunk[..n]));
                    }
                }
            });
        }
        Ok(Self { child, lines: BufReader::new(stdout).lines(), stderr: stderr_buf })
    }

    /// Next parsed JSON line. Non-JSON lines are skipped. Returns `None` at
    /// end of output.
    pub async fn next(&mut self, cancel: &CancellationToken, idle: Duration) -> HarnessResult<Option<serde_json::Value>> {
        loop {
            let line = tokio::select! {
                l = tokio::time::timeout(idle, self.lines.next_line()) => match l {
                    Ok(Ok(l)) => l,
                    Ok(Err(e)) => return Err(HarnessError::new(ErrorKind::LocalDependency, e.to_string())),
                    Err(_) => {
                        let _ = self.child.start_kill();
                        return Err(HarnessError::new(ErrorKind::Timeout, "CLI produced no output in time"));
                    }
                },
                _ = cancel.cancelled() => {
                    let _ = self.child.start_kill();
                    return Err(HarnessError::cancelled());
                }
            };
            let Some(line) = line else { return Ok(None) };
            let t = line.trim();
            if t.is_empty() || !t.starts_with('{') {
                continue;
            }
            if let Ok(v) = serde_json::from_str(t) {
                return Ok(Some(v));
            }
        }
    }

    /// Wait for exit; returns exit code and captured (redacted) stderr.
    pub async fn finish(mut self) -> (Option<i32>, String) {
        let status = tokio::time::timeout(Duration::from_secs(10), self.child.wait()).await;
        let code = match status {
            Ok(Ok(s)) => s.code(),
            _ => {
                let _ = self.child.start_kill();
                None
            }
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        let err = self.stderr.lock().await.clone();
        (code, magpie_security::redact(err.trim()))
    }
}

/// Render a multi-turn request as a single prompt for CLIs that accept one
/// prompt per invocation.
pub fn flatten_prompt(req: &ExecRequest, include_system: bool) -> String {
    let convo: Vec<&Message> = req.messages.iter().filter(|m| m.role != Role::System).collect();
    let mut system: Vec<String> = Vec::new();
    if include_system {
        system.extend(req.system.iter().cloned());
        system.extend(req.messages.iter().filter(|m| m.role == Role::System).map(|m| m.text_content()));
    }
    let mut out = String::new();
    if !system.is_empty() {
        out.push_str("<instructions>\n");
        out.push_str(&system.join("\n\n"));
        out.push_str("\n</instructions>\n\n");
    }
    if convo.len() == 1 && convo[0].role == Role::User {
        out.push_str(&convo[0].text_content());
        return out;
    }
    out.push_str("<conversation>\n");
    for m in &convo {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool_result",
            Role::System => "system",
        };
        out.push_str(&format!("<{role}>\n{}\n", m.text_content()));
        for c in &m.tool_calls {
            out.push_str(&format!("[called {} with {}]\n", c.name, c.arguments));
        }
        out.push_str(&format!("</{role}>\n"));
    }
    out.push_str("</conversation>\n\nRespond to the final user message.");
    out
}

/// Classify a CLI failure from its output.
pub fn classify_cli_failure(text: &str, code: Option<i32>) -> HarnessError {
    let lower = text.to_ascii_lowercase();
    let msg = if text.trim().is_empty() {
        format!("CLI exited with status {}", code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".into()))
    } else {
        text.trim().chars().take(600).collect()
    };
    let kind = if lower.contains("not logged in")
        || lower.contains("please run /login")
        || lower.contains("login required")
        || lower.contains("authentication")
        || lower.contains("invalid api key")
        || lower.contains("unauthorized")
        || lower.contains("oauth token has expired")
        || lower.contains("sign in")
    {
        ErrorKind::Authentication
    } else if lower.contains("usage limit")
        || lower.contains("limit reached")
        || lower.contains("quota")
        || lower.contains("out of credits")
    {
        ErrorKind::QuotaExhausted
    } else if lower.contains("rate limit") || lower.contains("429") || lower.contains("too many requests") {
        ErrorKind::RateLimited
    } else if lower.contains("overloaded") || lower.contains("529") || lower.contains("503") || lower.contains("internal server error") {
        ErrorKind::ProviderUnavailable
    } else if lower.contains("model")
        && (lower.contains("not found")
            || lower.contains("not available")
            || lower.contains("does not exist")
            || lower.contains("not supported"))
    {
        ErrorKind::ModelNotFound
    } else if lower.contains("context") && (lower.contains("too long") || lower.contains("exceed")) {
        ErrorKind::ContextLength
    } else {
        ErrorKind::ProviderUnavailable
    };
    let mut e = HarnessError::new(kind, magpie_security::redact(&msg));
    if let Some(c) = code {
        e.status = Some(c);
    }
    e
}

/// Directory used as the working directory for non-agent CLI requests, so
/// CLIs do not pick up project instructions from wherever the harness runs.
pub fn ensure_scratch(dir: &Path) -> PathBuf {
    let _ = std::fs::create_dir_all(dir);
    dir.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_single_turn_verbatim() {
        let r = ExecRequest::simple("hello there");
        assert_eq!(flatten_prompt(&r, true), "hello there");
    }

    #[test]
    fn flattens_multi_turn_with_instructions() {
        let mut r = ExecRequest::simple("first");
        r.system = Some("be terse".into());
        r.messages.push(Message::text(Role::Assistant, "ok"));
        r.messages.push(Message::text(Role::User, "second"));
        let p = flatten_prompt(&r, true);
        assert!(p.starts_with("<instructions>\nbe terse"));
        assert!(p.contains("<assistant>\nok\n</assistant>"));
        assert!(p.ends_with("Respond to the final user message."));
        assert!(!flatten_prompt(&r, false).contains("be terse"));
    }

    #[test]
    fn classifies_cli_errors() {
        assert_eq!(classify_cli_failure("Invalid API key · Please run /login", Some(1)).kind, ErrorKind::Authentication);
        assert_eq!(classify_cli_failure("Claude AI usage limit reached|1760000000", Some(1)).kind, ErrorKind::QuotaExhausted);
        assert_eq!(classify_cli_failure("API Error: 529 overloaded", Some(1)).kind, ErrorKind::ProviderUnavailable);
        assert_eq!(classify_cli_failure("", Some(2)).message, "CLI exited with status 2");
    }
}
