//! Harness process lifecycle shared by the CLI and the desktop app:
//! discovery of a running harness, detached launch, health checks, graceful
//! stop, and the daemon entry point itself.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use magpie_api::RuntimeInfo;
use magpie_core::*;
use magpie_engine::{Harness, HarnessOptions};
use magpie_security::SystemSecretStore;
use magpie_store::paths::Paths;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

pub use magpie_store::paths;

#[derive(Debug, Clone)]
pub struct Connection {
    pub url: String,
    pub token: String,
    pub info: RuntimeInfo,
}

#[derive(Debug, Deserialize)]
pub struct Health {
    pub status: String,
    pub product: String,
    pub version: String,
    pub api_version: u32,
}

pub fn read_runtime(paths: &Paths) -> Option<RuntimeInfo> {
    serde_json::from_str(&std::fs::read_to_string(paths.runtime_file()).ok()?).ok()
}

pub fn read_admin_token(paths: &Paths) -> Option<String> {
    std::fs::read_to_string(paths.admin_token()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("http client")
}

/// Check whether a Magpie harness answers at `url`.
pub async fn probe(url: &str) -> Option<Health> {
    let h: Health = http().get(format!("{url}/health")).send().await.ok()?.json().await.ok()?;
    (h.product == PRODUCT_NAME && h.status == "ok").then_some(h)
}

/// Find a running, healthy harness for this data directory.
pub async fn discover(paths: &Paths) -> Option<Connection> {
    let mut info = read_runtime(paths)?;
    let url = reqwest::Url::parse(&info.url).ok()?;
    if url.scheme() != "http" || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
        return None;
    }
    let health = probe(&info.url).await?;
    // Return an authenticated incompatible service so callers can explain the
    // mismatch instead of launching a competing process and timing out.
    info.api_version = health.api_version;
    info.version = health.version;
    let token = read_admin_token(paths)?;
    let verified = http().get(format!("{}/v1/status", info.url)).bearer_auth(&token).send().await.ok()?;
    if !verified.status().is_success() {
        return None;
    }
    Some(Connection { url: info.url.clone(), token, info })
}

/// Launch `program args...` detached from this process, with output
/// appended to `log`.
pub fn spawn_detached(program: &Path, args: &[String], log: &Path) -> std::io::Result<u32> {
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(log).map(|m| m.len() > 5 * 1024 * 1024).unwrap_or(false) {
        let _ = std::fs::rename(log, log.with_extension("log.old"));
    }
    let out = std::fs::OpenOptions::new().create(true).append(true).open(log)?;
    let err = out.try_clone()?;
    let mut cmd = std::process::Command::new(program);
    cmd.args(args).stdin(std::process::Stdio::null()).stdout(out).stderr(err);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // New session: survives the parent terminal or app exiting.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    let child = cmd.spawn()?;
    Ok(child.id())
}

/// Return a connection to the harness, launching it if needed.
pub async fn ensure_running(paths: &Paths, program: &Path, args: &[String], timeout: Duration) -> HarnessResult<Connection> {
    if let Some(c) = discover(paths).await {
        return Ok(c);
    }
    let log = paths.logs_dir().join("harness.log");
    spawn_detached(program, args, &log)
        .map_err(|e| HarnessError::new(ErrorKind::LocalDependency, format!("Could not start the harness: {e}")))?;
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(150)).await;
        if let Some(c) = discover(paths).await {
            return Ok(c);
        }
    }
    Err(HarnessError::new(ErrorKind::Timeout, format!("The harness did not start in time. See {}", log.display())))
}

/// Ask the harness to stop and wait for it to exit.
pub async fn stop(conn: &Connection) -> HarnessResult<()> {
    let response = http()
        .post(format!("{}/v1/admin/shutdown", conn.url))
        .bearer_auth(&conn.token)
        .send()
        .await
        .map_err(|_| HarnessError::internal("Could not contact the harness for shutdown"))?;
    if !response.status().is_success() {
        return Err(HarnessError::internal("The harness rejected the shutdown request"));
    }
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(150)).await;
        if probe(&conn.url).await.is_none() {
            return Ok(());
        }
    }
    // Never signal a PID from a stale runtime file: it may now belong to another process.
    Err(HarnessError::internal("The harness did not stop within 15 seconds"))
}

pub struct DaemonOptions {
    /// Command that relaunches this harness (for start-on-login).
    pub launch_command: Option<(PathBuf, Vec<String>)>,
    pub port: Option<u16>,
}

fn init_logging(diagnostic: bool) {
    let default = if diagnostic { "debug" } else { "info" };
    let filter = tracing_subscriber::EnvFilter::try_from_env("MAGPIE_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(format!("{default},hyper=warn,reqwest=warn,h2=warn,rustls=warn")));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_target(false).with_ansi(false).try_init();
}

/// Run the harness in this process until stopped (signal or API).
pub async fn run_daemon(opts: DaemonOptions) -> HarnessResult<()> {
    let paths = Paths::resolve();
    paths.ensure().map_err(|e| HarnessError::internal(format!("cannot create data directory: {e}")))?;
    // Exclusive lock for the data directory, held for the process lifetime.
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.root.join("harness.lock"))
        .map_err(|e| HarnessError::internal(format!("cannot open lock file: {e}")))?;
    if lock_file.try_lock().is_err() {
        let detail = match discover(&paths).await {
            Some(c) => format!(" at {} (pid {})", c.url, c.info.pid),
            None => String::new(),
        };
        return Err(HarnessError::new(ErrorKind::InvalidRequest, format!("A harness is already running{detail}")));
    }
    let force_file = std::env::var("MAGPIE_SECRET_STORE").map(|v| v == "file").unwrap_or(false);
    let secrets = SystemSecretStore::new(paths.secrets_file(), force_file);
    let backend = secrets.primary_backend();

    // Peek at settings for logging verbosity before the harness opens.
    let diagnostic = magpie_store::Store::open(&paths.database())
        .ok()
        .and_then(|s| s.settings().ok())
        .map(|s| s.security.diagnostic_logging)
        .unwrap_or(false);
    init_logging(diagnostic);
    tracing::info!(version = VERSION, data = %paths.root.display(), secrets = backend.as_str(), "starting harness");

    let harness: Arc<Harness> = Harness::open(HarnessOptions {
        paths: paths.clone(),
        secrets: Arc::new(secrets),
        secret_backend: backend,
        adapter_factory: None,
        launch_command: opts.launch_command,
        background: true,
    })
    .await?;
    if let Some(port) = opts.port {
        let mut s = harness.settings();
        if s.server.port != port {
            s.server.port = port;
            harness.update_settings(s)?;
        }
    }

    let stop = CancellationToken::new();
    let stop_signal = stop.clone();
    tokio::spawn(async move {
        wait_for_signal().await;
        tracing::info!("stop signal received");
        stop_signal.cancel();
    });
    let result = magpie_api::serve(harness.clone(), stop.clone()).await;
    harness.shutdown().await;
    tracing::info!("harness stopped");
    drop(lock_file);
    result
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("signal handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
