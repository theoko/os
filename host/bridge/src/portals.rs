//! Portal connectors — the OS reaching the user's own live services.
//!
//! Two are known: superintelmarkets.com (market intelligence) and
//! teddysearch.com (the knowledge corpus, handled in `tsearch`). Both are
//! plain HTTPS JSON, so this is a small fetch-and-shape layer rather than a
//! client library.
//!
//! Responses are cached with a short TTL. Probing these endpoints during
//! development got the caller rate-limited into `000` responses, and a search
//! field that hits a live API per keystroke would do the same to the user.

use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A named remote endpoint.
pub struct Endpoint {
    pub tool: &'static str,
    pub url: &'static str,
    /// Query parameters this endpoint understands, in call order.
    pub params: &'static [&'static str],
}

/// Confirmed-live endpoints. `/api/screen` and `/api/search` return 404 on the
/// public host, so they are deliberately absent rather than listed and broken.
pub const ENDPOINTS: &[Endpoint] = &[
    Endpoint {
        tool: "market.health",
        url: "https://superintelmarkets.com/health",
        params: &[],
    },
    Endpoint {
        tool: "market.fear_greed",
        url: "https://superintelmarkets.com/api/fear-greed",
        params: &["ticker", "purpose"],
    },
];

pub fn find(tool: &str) -> Option<&'static Endpoint> {
    ENDPOINTS.iter().find(|e| e.tool == tool)
}

/// How long a response stays fresh. Market sentiment does not move in seconds,
/// and this is what keeps the guest from rate-limiting the user's own site.
fn ttl() -> Duration {
    Duration::from_secs(
        env::var("OS_PORTAL_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120),
    )
}

static CACHE: Mutex<Option<HashMap<String, (Instant, String)>>> = Mutex::new(None);

fn cached(key: &str) -> Option<String> {
    let mut g = CACHE.lock().ok()?;
    let map = g.get_or_insert_with(HashMap::new);
    match map.get(key) {
        Some((at, body)) if at.elapsed() < ttl() => Some(body.clone()),
        _ => None,
    }
}

fn store(key: &str, body: &str) {
    if let Ok(mut g) = CACHE.lock() {
        g.get_or_insert_with(HashMap::new)
            .insert(key.to_string(), (Instant::now(), body.to_string()));
    }
}

/// Percent-encode a query value. Portal params are tickers and short words,
/// but a caller controls them, so they never go into a URL unescaped.
fn encode(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// GET `endpoint` with `args`, returning the raw JSON body.
pub fn fetch(ep: &Endpoint, args: &[(String, String)]) -> Result<String, String> {
    let mut url = ep.url.to_string();
    let mut first = true;
    for name in ep.params {
        if let Some((_, v)) = args.iter().find(|(k, _)| k == name) {
            url.push(if first { '?' } else { '&' });
            first = false;
            url.push_str(name);
            url.push('=');
            url.push_str(&encode(v));
        }
    }

    if let Some(hit) = cached(&url) {
        return Ok(hit);
    }

    let out = Command::new("curl")
        .args([
            "-sS",
            "--fail",
            "--max-time",
            "20",
            "-H",
            "Accept: application/json",
        ])
        .arg(&url)
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err
            .lines()
            .last()
            .unwrap_or("request failed")
            .chars()
            .take(100)
            .collect());
    }
    let body = String::from_utf8_lossy(&out.stdout).to_string();
    // Refuse HTML: a 200 from an SPA catch-all is not an API response.
    if body.trim_start().starts_with('<') {
        return Err("endpoint returned HTML, not JSON".into());
    }
    store(&url, &body);
    Ok(body)
}

/// Flatten a portal response into `ROW k=v` lines the guest can render.
///
/// The guest has no JSON parser and an 8x8-era line protocol, so the shaping
/// happens here.
pub fn rows_for(tool: &str, body: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return vec!["ROW error=unparseable response".into()];
    };
    match tool {
        "market.health" => {
            let mut rows = Vec::new();
            let status = v
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");
            rows.push(format!("ROW field=status|value={status}"));
            if let Some(d) = v.get("degraded").and_then(|d| d.as_array()) {
                let names: Vec<&str> = d.iter().filter_map(|x| x.as_str()).collect();
                if !names.is_empty() {
                    rows.push(format!("ROW field=degraded|value={}", names.join(" ")));
                }
            }
            if let Some(svcs) = v.get("services").and_then(|s| s.as_object()) {
                for (name, s) in svcs.iter().take(6) {
                    let ok = s.get("ok").and_then(|o| o.as_bool()).unwrap_or(false);
                    rows.push(format!(
                        "ROW field={name}|value={}",
                        if ok { "ok" } else { "down" }
                    ));
                }
            }
            rows
        }
        "market.fear_greed" => {
            let mut rows = Vec::new();
            for key in ["score", "label", "available_count"] {
                if let Some(x) = v.get(key) {
                    if !x.is_null() {
                        rows.push(format!("ROW field={key}|value={}", scalar(x)));
                    }
                }
            }
            if let Some(c) = v.get("components").and_then(|c| c.as_array()) {
                for comp in c.iter().take(5) {
                    let name = comp.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                    let val = comp
                        .get("value_fmt")
                        .and_then(|n| n.as_str())
                        .unwrap_or("N/A");
                    rows.push(format!("ROW field={name}|value={val}"));
                }
            }
            rows
        }
        _ => vec![format!("ROW body={}", scalar(&v))],
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
    .chars()
    // The guest font is ASCII-only and its line buffer is bounded.
    .filter(|c| c.is_ascii() && !c.is_control() && *c != '|')
    .take(80)
    .collect()
}

/// Where a portal snapshot is cached on disk, for offline display.
pub fn snapshot_path() -> PathBuf {
    env::var("OS_PORTAL_SNAPSHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library/Application Support/os/knowledge/portals.json")
        })
}

pub fn save_snapshot(tool: &str, body: &str) {
    let path = snapshot_path();
    if let Some(d) = path.parent() {
        let _ = fs::create_dir_all(d);
    }
    let mut all: HashMap<String, String> = fs::read_to_string(&path)
        .ok()
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    all.insert(tool.to_string(), body.to_string());
    if let Ok(raw) = serde_json::to_string(&all) {
        let _ = fs::write(&path, raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_confirmed_endpoints_are_listed() {
        // /api/screen and /api/search 404 publicly; listing them would give the
        // guest tools that always fail.
        for e in ENDPOINTS {
            assert!(
                e.url.starts_with("https://"),
                "portal must be TLS: {}",
                e.url
            );
            assert!(!e.url.contains("/api/screen"));
            assert!(!e.url.contains("/api/search"));
        }
        assert!(find("market.health").is_some());
        assert!(find("market.fear_greed").is_some());
        assert!(find("market.nope").is_none());
    }

    #[test]
    fn query_values_are_escaped() {
        assert_eq!(encode("NVDA"), "NVDA");
        assert_eq!(encode("a b"), "a%20b");
        assert_eq!(encode("x&y=z"), "x%26y%3Dz");
        assert_eq!(encode("../etc"), "..%2Fetc");
    }

    #[test]
    fn health_rows_report_status_and_degradation() {
        let body = r#"{"status":"degraded","degraded":["celery"],
            "services":{"celery":{"ok":false},"circuit_breaker":{"ok":true}}}"#;
        let rows = rows_for("market.health", body);
        assert!(rows
            .iter()
            .any(|r| r.contains("status") && r.contains("degraded")));
        assert!(rows
            .iter()
            .any(|r| r.contains("celery") && r.contains("down")));
        assert!(rows
            .iter()
            .any(|r| r.contains("circuit_breaker") && r.contains("ok")));
    }

    #[test]
    fn fear_greed_rows_carry_components() {
        let body = r#"{"available_count":2,"cnn_fear_greed":null,
            "components":[{"name":"VIX Level","value_fmt":"18.57"},
                          {"name":"Put/Call Ratio","value_fmt":"1.762"}]}"#;
        let rows = rows_for("market.fear_greed", body);
        assert!(rows
            .iter()
            .any(|r| r.contains("VIX Level") && r.contains("18.57")));
        assert!(rows.iter().any(|r| r.contains("Put/Call")));
        // Nulls must not render as the string "null".
        assert!(!rows.iter().any(|r| r.contains("cnn_fear_greed")));
    }

    #[test]
    fn unparseable_body_is_reported_not_panicked() {
        let rows = rows_for("market.health", "<html>nope</html>");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains("error"));
    }

    #[test]
    fn scalars_are_wire_safe() {
        // '|' is the field separator and the guest font is ASCII-only.
        let v: Value = serde_json::from_str(r#""a|béc""#).unwrap();
        let s = scalar(&v);
        assert!(!s.contains('|'));
        assert!(s.is_ascii());
    }

    #[test]
    fn snapshot_lives_outside_the_repo() {
        // Serialised: these tests mutate process env, which cargo's
        // parallel runner would otherwise leak between them.
        let _env = crate::graph::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_PORTAL_SNAPSHOT") };
        let p = snapshot_path().to_string_lossy().to_string();
        assert!(
            !p.contains("/os/search"),
            "snapshot must not land in the repo: {p}"
        );
    }
}
