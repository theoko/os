//! Host paths shared by bridge modules.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// `$HOME`, or `.` when unset (tests / odd hosts).
pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `~/Library/Application Support/os` — skills, knowledge indexes, etc.
pub fn app_support() -> PathBuf {
    home().join("Library/Application Support/os")
}

/// `$VAR` if set, else `app_support()/knowledge/<file>`.
pub fn env_or_knowledge(var: &str, file: &str) -> PathBuf {
    env::var(var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| app_support().join("knowledge").join(file))
}

/// Create parent dirs and write `value` as compact JSON.
pub fn write_json<T: Serialize>(path: impl AsRef<Path>, value: &T) -> Result<(), String> {
    let path = path.as_ref();
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
    }
    let raw = serde_json::to_string(value).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

/// Read JSON from `path`, or `T::default()` when missing / invalid.
pub fn read_json_or_default<T: DeserializeOwned + Default>(path: impl AsRef<Path>) -> T {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => T::default(),
    }
}

/// Serialise tests that mutate process env / Application Support paths.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
