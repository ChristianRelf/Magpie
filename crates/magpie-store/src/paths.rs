//! Platform data locations. Override with `MAGPIE_HOME` (used by tests and
//! portable installs).

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        if let Ok(home) = std::env::var("MAGPIE_HOME") {
            if !home.is_empty() {
                return Self { root: PathBuf::from(home) };
            }
        }
        let base = dirs_data_dir().unwrap_or_else(|| std::env::temp_dir());
        Self { root: base.join("magpie") }
    }

    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("magpie.db")
    }

    /// Admin token used by the desktop app and CLI (owner-only file).
    pub fn admin_token(&self) -> PathBuf {
        self.root.join("admin.token")
    }

    /// Runtime info of the running harness (port, pid, version).
    pub fn runtime_file(&self) -> PathBuf {
        self.root.join("harness.json")
    }

    pub fn secrets_file(&self) -> PathBuf {
        self.root.join("secrets.json")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700))?;
        }
        std::fs::create_dir_all(self.logs_dir())
    }
}

fn dirs_data_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}
