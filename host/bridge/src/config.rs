//! Operator configuration — which portal family this machine pulls from.
//!
//! These laptops ship pre-configured to people who are not the owner, so
//! choosing a portal family is an **operator** action, not a guest one. It sits
//! behind a password that lives in the macOS Keychain: not in the repo, not in
//! the ISO, and not in any file this program writes.
//!
//! Set the password on a host with:
//!
//! ```sh
//! security add-generic-password -s os-admin -a config -w '<secret>'
//! # -U replaces an existing one:
//! security add-generic-password -U -s os-admin -a config -w '<secret>'
//! ```
//!
//! `OS_CONFIG_PASSWORD` overrides the Keychain for tests and dev. Set it to the
//! *empty string* to mean "this host has no admin password" — that is how a
//! test exercises `not_configured` without touching the real Keychain.
//!
//! ## Wire protocol
//!
//! | Request | Reply |
//! |---------|-------|
//! | `CALL config.status` | `OK config.status portal=<family> locked=<0\|1> configured=<0\|1>` |
//! | `CALL config.unlock pass=<secret>` | `OK config.unlock` / `ERR config.unlock bad_pass\|not_configured\|too_many` |
//! | `CALL config.portal family=<teddy\|market\|none>` | `OK config.portal family=<family>` / `ERR config.portal locked\|unknown_family` |
//!
//! `config.status` is answered while locked: it names no secret, and the guest
//! needs it to draw the panel before anyone has typed anything.
//!
//! **Unlock is per connection.** It lives in the `Session` the connection
//! handler owns, is never global, and is never written to disk — a new
//! connection starts locked even if another one is unlocked right now. The
//! family choice is the opposite: it is host state and persists.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Keychain service/account the admin password is stored under.
pub const KEYCHAIN_SERVICE: &str = "os-admin";
pub const KEYCHAIN_ACCOUNT: &str = "config";

/// Failed unlocks a single connection gets before it is cut off entirely.
pub const MAX_UNLOCK_ATTEMPTS: u32 = 5;

/// Fixed pause after a wrong password. Small enough to be invisible to an
/// operator typing one password, large enough that a guest cannot grind the
/// space over a serial line before the cap stops it anyway.
const UNLOCK_FAIL_DELAY: Duration = Duration::from_millis(200);

/// Which portal family the OS pulls live data from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Family {
    Teddy,
    Market,
    /// No live portal family. This is the shipped default: a fresh machine
    /// pulls from nobody until an operator says otherwise.
    #[default]
    None,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Teddy => "teddy",
            Family::Market => "market",
            Family::None => "none",
        }
    }

    /// Parse a wire value. Unknown spellings are rejected rather than coerced
    /// to `none`, so a typo is an error the operator sees.
    pub fn parse(s: &str) -> Option<Family> {
        match s {
            "teddy" => Some(Family::Teddy),
            "market" => Some(Family::Market),
            "none" => Some(Family::None),
            _ => None,
        }
    }
}

/// Per-connection unlock state. One per `handle_client`, never shared.
#[derive(Debug, Default)]
pub struct Session {
    unlocked: bool,
    failures: u32,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn locked(&self) -> bool {
        !self.unlocked
    }
}

fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Where the operator's choice is persisted. `OS_CONFIG_PATH` overrides it so
/// tests never write to a real machine's configuration.
pub fn config_path() -> PathBuf {
    env::var_os("OS_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join("Library/Application Support/os/config/config.json"))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    portal_family: String,
}

/// The active portal family.
///
/// Read from disk on each call: the file is a few bytes, and a cached copy
/// would let a stale in-memory value keep a family alive after it was turned
/// off — the one failure mode that actually matters here. A missing, empty,
/// corrupt or unknown-valued file all mean `none`; nothing here panics.
pub fn family() -> Family {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|raw| serde_json::from_str::<ConfigFile>(&raw).ok())
        .and_then(|c| Family::parse(&c.portal_family))
        .unwrap_or(Family::None)
}

/// Persist the operator's choice, creating the config directory if needed.
pub fn set_family(f: Family) -> Result<(), String> {
    let path = config_path();
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
    }
    let raw = serde_json::to_string(&ConfigFile {
        portal_family: f.name().to_string(),
    })
    .map_err(|e| format!("encode: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("write {}: {e}", path.display()))
}

/// The admin password, if this host has one.
///
/// Environment first (tests and dev), Keychain second. Nothing is read from a
/// file: AGENTS.md is explicit that secrets stay out of the tree, and this one
/// must also stay out of the ISO the laptops boot from.
pub fn admin_password() -> Option<String> {
    if let Ok(p) = env::var("OS_CONFIG_PASSWORD") {
        // Set-but-empty is an explicit "no password configured", and it short
        // circuits before the Keychain so tests never reach the real one.
        return (!p.is_empty()).then_some(p);
    }
    keychain_password()
}

fn keychain_password() -> Option<String> {
    let out = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // `security -w` appends a newline. Only that is trimmed: a password may
    // legitimately start or end with a space.
    let pass = String::from_utf8_lossy(&out.stdout)
        .trim_end_matches(['\n', '\r'])
        .to_string();
    (!pass.is_empty()).then_some(pass)
}

/// Compare two secrets without leaking where they first differ.
///
/// The loop always walks the full overlap — no early `return` on a mismatched
/// byte — and the length check is a separate, non-short-circuiting `&` so that
/// a wrong-length guess costs the same as a wrong-content one. There is no hash
/// crate in this workspace (deps are serde + serde_json) and one is not being
/// added for this: the secret lives in the Keychain, not in the repo.
fn secrets_match(a: &[u8], b: &[u8]) -> bool {
    let mut diff: u8 = 0;
    let n = a.len().min(b.len());
    for i in 0..n {
        diff |= a[i] ^ b[i];
    }
    (diff == 0) & (a.len() == b.len())
}

/// `CALL config.status` — allowed while locked; exposes no secret.
pub fn status(session: &Session) -> Vec<String> {
    vec![format!(
        "OK config.status portal={} locked={} configured={}",
        family().name(),
        u8::from(session.locked()),
        u8::from(admin_password().is_some()),
    )]
}

/// `CALL config.unlock pass=<secret>` — per-connection, rate limited.
pub fn unlock(session: &mut Session, pass: &str) -> Vec<String> {
    // The cap is checked first and is absolute: past it the connection gets
    // `too_many` even for the right password, so a guest cannot grind for it
    // and then use it. Reconnecting is the operator's way out.
    if session.failures >= MAX_UNLOCK_ATTEMPTS {
        return vec!["ERR config.unlock too_many".into()];
    }
    let Some(expected) = admin_password() else {
        // Nothing to guess: this is not a failed attempt, it is an unconfigured
        // host, and saying so is what tells the operator to go set a password.
        return vec!["ERR config.unlock not_configured".into()];
    };
    if secrets_match(pass.as_bytes(), expected.as_bytes()) {
        session.unlocked = true;
        return vec!["OK config.unlock".into()];
    }
    session.failures += 1;
    std::thread::sleep(UNLOCK_FAIL_DELAY);
    vec!["ERR config.unlock bad_pass".into()]
}

/// `CALL config.portal family=<teddy|market|none>` — operator only.
pub fn set_portal(session: &Session, family_arg: Option<&str>) -> Vec<String> {
    if session.locked() {
        return vec!["ERR config.portal locked".into()];
    }
    // A missing `family=` is an unknown family: there is no default worth
    // guessing when the answer changes where the machine sends its traffic.
    let Some(f) = family_arg.and_then(Family::parse) else {
        return vec!["ERR config.portal unknown_family".into()];
    };
    match set_family(f) {
        Ok(()) => vec![format!("OK config.portal family={}", f.name())],
        Err(e) => {
            // The detail goes to the host log; the wire gets one stable token.
            eprintln!("config: {e}");
            vec!["ERR config.portal write_failed".into()]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Point config at a scratch file and password. Returns the temp dir so the
    /// caller can clean up. Callers hold `graph::ENV_LOCK` first.
    fn scratch(tag: &str, pass: Option<&str>) -> PathBuf {
        let dir = env::temp_dir().join(format!("os-config-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch dir");
        unsafe {
            env::set_var("OS_CONFIG_PATH", dir.join("config.json"));
            // Empty string = "no password on this host", still never Keychain.
            env::set_var("OS_CONFIG_PASSWORD", pass.unwrap_or(""));
        }
        dir
    }

    fn cleanup(dir: PathBuf) {
        unsafe {
            env::remove_var("OS_CONFIG_PATH");
            env::remove_var("OS_CONFIG_PASSWORD");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn comparison_does_not_exit_early_and_still_decides_correctly() {
        assert!(secrets_match(b"hunter2", b"hunter2"));
        assert!(!secrets_match(b"hunter2", b"hunter3"));
        // Differing in the first byte and in the last are both mismatches.
        assert!(!secrets_match(b"xunter2", b"hunter2"));
        // Length is compared separately, so a prefix is not a match.
        assert!(!secrets_match(b"hunter", b"hunter2"));
        assert!(!secrets_match(b"hunter2", b"hunter"));
        assert!(secrets_match(b"", b""));
        assert!(!secrets_match(b"", b"x"));
    }

    #[test]
    fn family_round_trips_through_the_file() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("roundtrip", Some("s3cret"));
        assert_eq!(family(), Family::None, "fresh host starts with no family");
        set_family(Family::Market).expect("write");
        assert_eq!(family(), Family::Market);
        set_family(Family::None).expect("write");
        assert_eq!(family(), Family::None);
        cleanup(dir);
    }

    #[test]
    fn a_corrupt_config_reads_as_none() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("corrupt", Some("s3cret"));
        fs::write(config_path(), "{not json at all").expect("write");
        assert_eq!(family(), Family::None);
        fs::write(config_path(), r#"{"portal_family":"nope"}"#).expect("write");
        assert_eq!(family(), Family::None);
        cleanup(dir);
    }

    #[test]
    fn set_family_creates_the_parent_directory() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("mkdir", Some("s3cret"));
        let nested = dir.join("a/b/config.json");
        unsafe { env::set_var("OS_CONFIG_PATH", &nested) };
        set_family(Family::Teddy).expect("write into a directory that does not exist yet");
        assert!(nested.is_file());
        assert_eq!(family(), Family::Teddy);
        cleanup(dir);
    }

    #[test]
    fn the_env_password_never_reaches_the_keychain() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("pass", Some("s3cret"));
        assert_eq!(admin_password().as_deref(), Some("s3cret"));
        unsafe { env::set_var("OS_CONFIG_PASSWORD", "") };
        assert_eq!(admin_password(), None, "empty means unconfigured");
        cleanup(dir);
    }
}
