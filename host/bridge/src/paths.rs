//! Host paths shared by bridge modules.

use std::env;
use std::path::PathBuf;

/// `$HOME`, or `.` when unset (tests / odd hosts).
pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
