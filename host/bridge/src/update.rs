//! Is there a newer build than the one this machine is running?
//!
//! The images are already published and checksummed by scripts/publish-os.sh
//! and watched by scripts/check-published.sh. What was missing is the other
//! half: a machine in somebody's hands has no way to learn that a newer build
//! exists. This is that half — it reports, and never applies.
//!
//! THE TRAP THIS IS BUILT AROUND
//! teddysearch.com serves a catch-all under /tsearch/: a missing path returns
//! **HTTP 200 with the homepage HTML**. Probed against the live host, VERSION,
//! manifest.json and latest.json all answered 200 — with a web page. An
//! updater that trusted the status code would parse that page as a release
//! descriptor and either crash or invent a version.
//!
//! So a fetch is believed only when the body is actually a manifest. Status
//! codes prove nothing here.

use std::process::Command;

/// Where releases are published — the root scripts/publish-os.sh writes to.
pub const DEFAULT_MANIFEST: &str = "https://teddysearch.com/tsearch/os/manifest.json";

pub fn manifest_url() -> String {
    std::env::var("OS_UPDATE_MANIFEST").unwrap_or_else(|_| DEFAULT_MANIFEST.to_string())
}

/// What a release manifest must carry to be usable.
#[derive(Debug, PartialEq, Eq)]
pub struct Manifest {
    pub version: String,
    /// sha256 of os.iso, matching the published SHA256SUMS.
    pub x86_sha: String,
    /// sha256 of os-arm64.iso.
    pub arm_sha: String,
}

/// Why a fetch is not a manifest. Each is a distinct thing to tell a person:
/// "update check failed" sends nobody anywhere useful.
#[derive(Debug, PartialEq, Eq)]
pub enum Bad {
    /// The catch-all served a web page. The common case today.
    NotJson,
    /// Valid JSON, missing a field the updater needs.
    Incomplete(&'static str),
    /// Could not reach the host at all.
    Unreachable,
}

impl Bad {
    pub fn token(&self) -> &'static str {
        match self {
            // Named for what it is rather than "parse_error": a server
            // returning its homepage for a missing file is a deployment fact,
            // and worth reading straight off the wire.
            Bad::NotJson => "manifest_is_not_json",
            Bad::Incomplete(_) => "manifest_incomplete",
            Bad::Unreachable => "unreachable",
        }
    }
}

/// Parse a manifest body, refusing anything that is not one.
///
/// Hand-rolled rather than serde on purpose: the input is untrusted, the shape
/// is three strings, and the failure that matters is "this is HTML" — easier
/// to state plainly than to derive.
pub fn parse(body: &str) -> Result<Manifest, Bad> {
    let trimmed = body.trim_start();
    // JSON starts with `{`. The homepage starts with `<!DOCTYPE`.
    if !trimmed.starts_with('{') {
        return Err(Bad::NotJson);
    }
    let field = |name: &str| -> Option<String> {
        let pat = format!("\"{name}\"");
        let rest = trimmed.split_once(&pat)?.1;
        let rest = rest.split_once(':')?.1.trim_start();
        let rest = rest.strip_prefix('"')?;
        let (val, _) = rest.split_once('"')?;
        (!val.is_empty()).then(|| val.to_string())
    };
    Ok(Manifest {
        version: field("version").ok_or(Bad::Incomplete("version"))?,
        x86_sha: field("x86_sha256").ok_or(Bad::Incomplete("x86_sha256"))?,
        arm_sha: field("arm_sha256").ok_or(Bad::Incomplete("arm_sha256"))?,
    })
}

fn fetch(url: &str) -> Result<String, Bad> {
    let out = Command::new("curl")
        .args(["-fsS", "--max-time", "20", url])
        .output()
        .map_err(|_| Bad::Unreachable)?;
    if !out.status.success() {
        return Err(Bad::Unreachable);
    }
    String::from_utf8(out.stdout).map_err(|_| Bad::NotJson)
}

/// `CALL update.check` — report, never apply.
///
/// Applying is deliberately absent. This kernel boots from read-only media and
/// has no filesystem driver, so it cannot rewrite its own boot image. And once
/// the Linux substrate makes that possible, an OS that silently replaces
/// itself from the network is the opposite of a system whose accesses are
/// explicit. Reporting is the honest capability today.
pub fn check(running: Option<&str>) -> Vec<String> {
    let url = manifest_url();
    let body = match fetch(&url) {
        Ok(b) => b,
        Err(e) => return vec![format!("ERR update.check {}", e.token())],
    };
    let m = match parse(&body) {
        Ok(m) => m,
        Err(e) => {
            // Name the URL: the answer is usually "nothing published there
            // yet" rather than "the updater is broken".
            eprintln!("update: {url} did not serve a manifest ({e:?})");
            return vec![format!("ERR update.check {}", e.token())];
        }
    };

    let mut out = vec![format!("OK update.check latest={}", m.version)];
    match running {
        Some(v) if v == m.version => out.push("ROW state=current".into()),
        Some(v) => out.push(format!("ROW state=behind|running={v}|latest={}", m.version)),
        // A guest that cannot say what it runs is a real state, not an error:
        // nothing bakes a build id into the kernel yet.
        None => out.push(format!("ROW state=unknown_running|latest={}", m.version)),
    }
    out.push(format!("ROW x86_sha256={}", m.x86_sha));
    out.push(format!("ROW arm_sha256={}", m.arm_sha));
    out.push("END".into());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{
      "version": "0.11.0",
      "built": "2026-07-26",
      "x86_sha256": "2be090a92543fc9c41e8a640516f53e54417db13c1a87f6ae0ce41a977cc1dcd",
      "arm_sha256": "ac0ac462d90f090ff12e5e8e6abded6323d5e7d1ffdec275a81cc1edc2e64836"
    }"#;

    #[test]
    fn a_manifest_parses() {
        let m = parse(GOOD).expect("valid manifest");
        assert_eq!(m.version, "0.11.0");
        assert!(m.x86_sha.starts_with("2be090a9"));
        assert!(m.arm_sha.starts_with("ac0ac462"));
    }

    #[test]
    fn the_homepage_is_not_a_manifest() {
        // The live failure this module exists for. Probed against the real
        // host: VERSION, manifest.json and latest.json each answered HTTP 200
        // with this page, because /tsearch/ has a catch-all.
        let html = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<title>teddy — revived</title>";
        assert_eq!(parse(html), Err(Bad::NotJson));
    }

    #[test]
    fn a_200_that_is_html_never_reads_as_a_version() {
        // The danger is not that it fails - it is that it might succeed and
        // invent a version out of page text.
        let html = "<html><body>version: 9.9.9</body></html>";
        assert!(matches!(parse(html), Err(Bad::NotJson)));
    }

    #[test]
    fn a_manifest_missing_a_checksum_is_refused() {
        // Half a manifest is worse than none: it would let a machine report
        // "up to date" against a release it cannot verify.
        let partial = r#"{"version":"0.11.0","x86_sha256":"abc"}"#;
        assert_eq!(parse(partial), Err(Bad::Incomplete("arm_sha256")));
    }

    #[test]
    fn an_empty_version_is_not_a_version() {
        let blank = r#"{"version":"","x86_sha256":"a","arm_sha256":"b"}"#;
        assert_eq!(parse(blank), Err(Bad::Incomplete("version")));
    }

    #[test]
    fn each_failure_has_its_own_token() {
        let tokens = [
            Bad::NotJson.token(),
            Bad::Incomplete("version").token(),
            Bad::Unreachable.token(),
        ];
        let unique: std::collections::HashSet<_> = tokens.iter().collect();
        assert_eq!(unique.len(), tokens.len(), "tokens must be distinguishable");
    }

    #[test]
    fn a_newer_published_version_is_not_read_as_current() {
        let m = parse(GOOD).unwrap();
        assert_ne!("0.10.0", m.version, "an older build must not read as current");
    }
}
