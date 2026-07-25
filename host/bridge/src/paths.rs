//! Host paths shared by bridge modules.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// `$HOME`, or `.` when unset (tests / odd hosts).
pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Create parent dirs and write `value` as compact JSON.
pub fn write_json<T: Serialize>(path: impl AsRef<Path>, value: &T) -> Result<PathBuf, String> {
    let path = path.as_ref();
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
    }
    let raw = serde_json::to_string(value).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path.to_path_buf())
}
