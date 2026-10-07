//! Secret storage. Credentials are kept in the OS credential manager
//! (macOS Keychain, Windows Credential Manager, Secret Service on Linux).
//! Production writes fail closed when the credential manager is unavailable.
//! An explicit plaintext file store is available only in debug builds for
//! isolated local testing; it is never a silent fallback.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const SERVICE: &str = "dev.magpie.harness";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("credential store unavailable: {0}")]
    Unavailable(String),
    #[error("credential store error: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretBackend {
    Keyring,
    File,
    Memory,
}

impl SecretBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            SecretBackend::Keyring => "keyring",
            SecretBackend::File => "file",
            SecretBackend::Memory => "memory",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "keyring" => Some(Self::Keyring),
            "file" => Some(Self::File),
            "memory" => Some(Self::Memory),
            _ => None,
        }
    }
}

pub trait SecretStore: Send + Sync {
    /// Store a secret; returns the backend that holds it.
    fn set(&self, id: &str, secret: &str) -> Result<SecretBackend, SecretError>;
    fn get(&self, id: &str, backend: Option<SecretBackend>) -> Result<Option<String>, SecretError>;
    fn delete(&self, id: &str, backend: Option<SecretBackend>) -> Result<(), SecretError>;
}

/// OS credential manager via the `keyring` crate.
#[derive(Default)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(id: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, id).map_err(|e| SecretError::Unavailable(e.to_string()))
    }

    /// Probe whether the platform credential manager is usable.
    pub fn probe() -> bool {
        let Ok(entry) = Self::entry("__magpie_probe__") else { return false };
        match entry.get_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => true,
            Err(_) => false,
        }
    }
}

impl SecretStore for KeyringSecretStore {
    fn set(&self, id: &str, secret: &str) -> Result<SecretBackend, SecretError> {
        Self::entry(id)?
            .set_password(secret)
            .map_err(|e| SecretError::Backend(e.to_string()))?;
        Ok(SecretBackend::Keyring)
    }

    fn get(&self, id: &str, _: Option<SecretBackend>) -> Result<Option<String>, SecretError> {
        match Self::entry(id)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError::Backend(e.to_string())),
        }
    }

    fn delete(&self, id: &str, _: Option<SecretBackend>) -> Result<(), SecretError> {
        match Self::entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError::Backend(e.to_string())),
        }
    }
}

/// Fallback store: a JSON file readable only by the current user.
pub struct FileSecretStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileSecretStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path, lock: Mutex::new(()) }
    }

    fn read(&self) -> Result<BTreeMap<String, String>, SecretError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| SecretError::Backend(e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(SecretError::Backend(e.to_string())),
        }
    }

    fn write(&self, map: &BTreeMap<String, String>) -> Result<(), SecretError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| SecretError::Backend(e.to_string()))?;
        }
        let tmp = self.path.with_extension("tmp");
        let data = serde_json::to_vec(map).map_err(|e| SecretError::Backend(e.to_string()))?;
        write_private(&tmp, &data).map_err(|e| SecretError::Backend(e.to_string()))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| SecretError::Backend(e.to_string()))
    }
}

/// Write a file with owner-only permissions.
pub fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(data)?;
    f.sync_all()
}

impl SecretStore for FileSecretStore {
    fn set(&self, id: &str, secret: &str) -> Result<SecretBackend, SecretError> {
        let _g = self.lock.lock().unwrap();
        let mut map = self.read()?;
        map.insert(id.to_string(), secret.to_string());
        self.write(&map)?;
        Ok(SecretBackend::File)
    }

    fn get(&self, id: &str, _: Option<SecretBackend>) -> Result<Option<String>, SecretError> {
        let _g = self.lock.lock().unwrap();
        Ok(self.read()?.get(id).cloned())
    }

    fn delete(&self, id: &str, _: Option<SecretBackend>) -> Result<(), SecretError> {
        let _g = self.lock.lock().unwrap();
        let mut map = self.read()?;
        if map.remove(id).is_some() {
            self.write(&map)?;
        }
        Ok(())
    }
}

/// In-memory store used by tests.
#[derive(Default)]
pub struct MemorySecretStore {
    map: Mutex<BTreeMap<String, String>>,
}

impl SecretStore for MemorySecretStore {
    fn set(&self, id: &str, secret: &str) -> Result<SecretBackend, SecretError> {
        self.map.lock().unwrap().insert(id.into(), secret.into());
        Ok(SecretBackend::Memory)
    }
    fn get(&self, id: &str, _: Option<SecretBackend>) -> Result<Option<String>, SecretError> {
        Ok(self.map.lock().unwrap().get(id).cloned())
    }
    fn delete(&self, id: &str, _: Option<SecretBackend>) -> Result<(), SecretError> {
        self.map.lock().unwrap().remove(id);
        Ok(())
    }
}

/// Production store: OS keyring only. Debug tests may explicitly select files.
pub struct SystemSecretStore {
    keyring: Option<KeyringSecretStore>,
    file: FileSecretStore,
}

impl SystemSecretStore {
    /// `force_file` disables the keyring (set via `MAGPIE_SECRET_STORE=file`).
    pub fn new(file_path: PathBuf, force_file: bool) -> Self {
        let keyring = if cfg!(debug_assertions) && force_file { None } else { Some(KeyringSecretStore) };
        Self { keyring, file: FileSecretStore::new(file_path) }
    }

    pub fn primary_backend(&self) -> SecretBackend {
        if self.keyring.is_some() {
            SecretBackend::Keyring
        } else {
            SecretBackend::File
        }
    }
}

impl SecretStore for SystemSecretStore {
    fn set(&self, id: &str, secret: &str) -> Result<SecretBackend, SecretError> {
        if let Some(k) = &self.keyring {
            return k.set(id, secret).map_err(|_| SecretError::Unavailable(
                "Unlock the operating system credential manager and try again. Magpie will not save this key to a plaintext file.".into()
            ));
        }
        self.file.set(id, secret)
    }

    fn get(&self, id: &str, backend: Option<SecretBackend>) -> Result<Option<String>, SecretError> {
        if let Some(k) = &self.keyring {
            if backend == Some(SecretBackend::File) {
                return Err(SecretError::Unavailable("This account used a development plaintext store. Reconnect it to save the credential in your OS keyring.".into()));
            }
            return k.get(id, None);
        }
        self.file.get(id, None)
    }

    fn delete(&self, id: &str, backend: Option<SecretBackend>) -> Result<(), SecretError> {
        if backend == Some(SecretBackend::File) { return self.file.delete(id, None); }
        if let Some(k) = &self.keyring { return k.delete(id, None); }
        self.file.delete(id, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_store_roundtrip_and_permissions() {
        let dir = std::env::temp_dir().join(format!("magpie-sec-{}", std::process::id()));
        let path = dir.join("secrets.json");
        let store = FileSecretStore::new(path.clone());
        assert_eq!(store.set("acc_1", "sk-123").unwrap(), SecretBackend::File);
        assert_eq!(store.get("acc_1", None).unwrap().as_deref(), Some("sk-123"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        store.delete("acc_1", None).unwrap();
        assert_eq!(store.get("acc_1", None).unwrap(), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
