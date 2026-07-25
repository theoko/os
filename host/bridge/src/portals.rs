//! Portal connectors — the OS reaching the user's own live services.
//!
//! teddysearch.com exposes two different things the bridge must keep distinct:
//!
//! * **Teddy API** (`tsearch` module) — the knowledge corpus at
//!   `/tsearch/corpus.json`. There is no server-side query endpoint; the file
//!   *is* the API, synced once and ranked locally.
//! * **Teddy portals** (this module) — live JSON tools on the same host
//!   (`/health`, `/api/fear-greed`, `/api/gex`). These leave the machine on
//!   every call and therefore require `portal=1`.
//!
//! `market.*` tools hit superintelmarkets.com (same response shapes). Both
//! families are plain HTTPS JSON with a short TTL cache — probing without a
//! TTL rate-limited the caller into `000` responses.

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

/// Confirmed-live endpoints. `/api/screen` and `/api/search` still 404 on the
/// public host, so they stay absent rather than listed and broken.
pub const ENDPOINTS: &[Endpoint] = &[
    // Teddy portals — live services on teddysearch.com (not the corpus dump).
    Endpoint {
        tool: "teddy.health",
        url: "https://teddysearch.com/health",
        params: &[],
    },
    Endpoint {
        tool: "teddy.fear_greed",
        url: "https://teddysearch.com/api/fear-greed",
        params: &["ticker", "purpose"],
    },
    Endpoint {
        tool: "teddy.gex",
        url: "https://teddysearch.com/api/gex",
        params: &[],
    },
    // Markets family — same shapes, different origin.
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

/// Tools that leave the machine and therefore need `portal=1`.
pub fn is_portal_tool(tool: &str) -> bool {
    find(tool).is_some()
}

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
        .args(["-sS", "--fail", "--max-time", "20", "-H", "Accept: application/json"])
        .arg(&url)
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().last().unwrap_or("request failed").chars().take(100).collect());
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
        "teddy.health" | "market.health" => rows_health(&v),
        "teddy.fear_greed" | "market.fear_greed" => rows_fear_greed(&v),
        "teddy.gex" => rows_gex(&v),
        _ => vec![format!("ROW body={}", scalar(&v))],
    }
}

fn rows_health(v: &Value) -> Vec<String> {
    let mut rows = Vec::new();
    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("unknown");
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

fn rows_fear_greed(v: &Value) -> Vec<String> {
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
                .map(|s| s.to_string())
                .or_else(|| comp.get("value").map(scalar))
                .unwrap_or_else(|| "N/A".into());
            rows.push(format!("ROW field={name}|value={val}"));
        }
    }
    rows
}

fn rows_gex(v: &Value) -> Vec<String> {
    let mut rows = Vec::new();
    for key in [
        "regime",
        "spot",
        "net_gex",
        "call_wall",
        "put_wall",
        "zero_gamma",
        "pcr",
        "n_strikes",
    ] {
        if let Some(x) = v.get(key) {
            if !x.is_null() {
                rows.push(format!("ROW field={key}|value={}", scalar(x)));
            }
        }
    }
    rows
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

/// Forget live portal state: in-memory TTL cache and on-disk snapshots.
///
/// Paired with `tsearch::forget` under `portal.forget` so revoking Online
/// services clears both the corpus API cache and the live portal residue.
pub fn forget() -> Result<&'static str, String> {
    if let Ok(mut g) = CACHE.lock() {
        *g = None;
    }
    let path = snapshot_path();
    match fs::remove_file(&path) {
        Ok(()) => Ok("removed"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok("nothing_to_remove"),
        Err(e) => Err(format!("{e}")),
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
            assert!(e.url.starts_with("https://"), "portal must be TLS: {}", e.url);
            assert!(!e.url.contains("/api/screen"));
            assert!(!e.url.contains("/api/search"));
        }
        assert!(find("teddy.health").is_some());
        assert!(find("teddy.fear_greed").is_some());
        assert!(find("teddy.gex").is_some());
        assert!(find("market.health").is_some());
        assert!(find("market.fear_greed").is_some());
        assert!(find("market.nope").is_none());
        assert!(is_portal_tool("teddy.health"));
        assert!(!is_portal_tool("search.query"));
    }

    #[test]
    fn teddy_and_market_portals_are_distinct_origins() {
        let teddy = find("teddy.health").unwrap().url;
        let market = find("market.health").unwrap().url;
        assert!(teddy.contains("teddysearch.com"), "{teddy}");
        assert!(market.contains("superintelmarkets.com"), "{market}");
        // Corpus dump is the teddy *API*, not a portal tool.
        assert!(find("tsearch.sync").is_none());
        assert!(!ENDPOINTS.iter().any(|e| e.url.contains("corpus.json")));
    }

    #[test]
    fn gex_rows_surface_regime_and_walls() {
        let body = r#"{"regime":"positive","spot":7413.1,"net_gex":1.2e9,
            "call_wall":7530.0,"put_wall":7300.0,"zero_gamma":7350.0,
            "pcr":0.9,"n_strikes":40,"intraday":[]}"#;
        let rows = rows_for("teddy.gex", body);
        assert!(rows.iter().any(|r| r.contains("regime") && r.contains("positive")));
        assert!(rows.iter().any(|r| r.contains("call_wall") && r.contains("7530")));
        // Nested arrays are summarised away — guest gets scalars only.
        assert!(!rows.iter().any(|r| r.contains("intraday")));
    }

    #[test]
    fn teddy_health_reuses_market_shaping() {
        let body = r#"{"status":"ok","degraded":[],"services":{"celery":{"ok":true}}}"#;
        let t = rows_for("teddy.health", body);
        let m = rows_for("market.health", body);
        assert_eq!(t, m);
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
        assert!(rows.iter().any(|r| r.contains("status") && r.contains("degraded")));
        assert!(rows.iter().any(|r| r.contains("celery") && r.contains("down")));
        assert!(rows.iter().any(|r| r.contains("circuit_breaker") && r.contains("ok")));
    }

    #[test]
    fn fear_greed_rows_carry_components() {
        let body = r#"{"available_count":2,"cnn_fear_greed":null,
            "components":[{"name":"VIX Level","value_fmt":"18.57"},
                          {"name":"Put/Call Ratio","value_fmt":"1.762"}]}"#;
        let rows = rows_for("market.fear_greed", body);
        assert!(rows.iter().any(|r| r.contains("VIX Level") && r.contains("18.57")));
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
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_PORTAL_SNAPSHOT") };
        let p = snapshot_path().to_string_lossy().to_string();
        assert!(!p.contains("/os/search"), "snapshot must not land in the repo: {p}");
    }

    #[test]
    fn forget_clears_snapshot_file() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-portal-forget-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("portals.json");
        unsafe { env::set_var("OS_PORTAL_SNAPSHOT", &path) };
        save_snapshot("teddy.health", r#"{"status":"ok"}"#);
        assert!(path.is_file());
        assert_eq!(forget().unwrap(), "removed");
        assert!(!path.is_file());
        assert_eq!(forget().unwrap(), "nothing_to_remove");
        unsafe { env::remove_var("OS_PORTAL_SNAPSHOT") };
        let _ = fs::remove_dir_all(&dir);
    }
}
