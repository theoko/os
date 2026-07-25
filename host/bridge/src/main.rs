//! Host-side MCP-shaped connector bridge.
//!
//! Speaks the line protocol in docs/mcp-connectors-os-doc-v01.md over TCP.
//! Email backends: mock (default) or `gog`. Skills: defaults + saved on host.

mod graph;
mod workspace;
mod search;
mod tsearch;
mod skills;

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// Longest request line we accept from the wire; the peer is untrusted.
const MAX_LINE: u64 = 64 * 1024;
/// Cap on an accumulated skills.save body.
const MAX_BODY: usize = 1024 * 1024;

/// `read_line` with a hard length cap so a peer that never sends `\n` cannot
/// grow the buffer without bound. `Ok(None)` = EOF, `Err` on I/O or oversize.
fn read_line_bounded<R: BufRead>(reader: &mut R, line: &mut String) -> std::io::Result<Option<()>> {
    line.clear();
    let n = reader.take(MAX_LINE).read_line(line)?;
    if n == 0 {
        return Ok(None);
    }
    if n as u64 == MAX_LINE && !line.ends_with('\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "line exceeds MAX_LINE",
        ));
    }
    Ok(Some(()))
}

struct Backends {
    email: String,
    search: String,
}

fn main() {
    let connect = env::var("OS_MCP_BRIDGE_CONNECT").ok();
    let addr = env::var("OS_MCP_BRIDGE_ADDR").unwrap_or_else(|_| "127.0.0.1:7420".into());
    let backends = Arc::new(Backends {
        email: env::var("OS_MCP_EMAIL_BACKEND").unwrap_or_else(|_| "mock".into()),
        search: env::var("OS_MCP_SEARCH_BACKEND").unwrap_or_else(|_| "tfidf".into()),
    });

    let (defaults, user) = skills::skills_dirs();
    let where_ = connect.as_deref().unwrap_or(addr.as_str());
    eprintln!(
        "os-mcp-bridge {} on {where_} (email={}; search={}; skills defaults={} user={})",
        if connect.is_some() { "connecting" } else { "listening" },
        backends.email,
        backends.search,
        defaults.display(),
        user.display()
    );

    if let Some(target) = connect {
        connect_loop(&target, backends);
    } else if let Some(path) = addr.strip_prefix("unix:") {
        serve_unix(path, backends);
    } else {
        serve_tcp(&addr, backends);
    }
}

/// Dial a peer that is already listening (UTM QEMU serial unix server).
/// Retries until the guest appears, then serves one session and reconnects.
fn connect_loop(target: &str, backends: Arc<Backends>) {
    let path = target.strip_prefix("unix:").unwrap_or(target);
    loop {
        match UnixStream::connect(path) {
            Ok(stream) => {
                eprintln!("connected to {path}");
                let backends = Arc::clone(&backends);
                if let Err(e) = handle_unix(stream, &backends) {
                    eprintln!("client error: {e}");
                }
                eprintln!("guest disconnected — waiting to reconnect");
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        }
    }
}

fn serve_tcp(addr: &str, backends: Arc<Backends>) {
    let listener = TcpListener::bind(addr).expect("bind bridge tcp");
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                let backends = Arc::clone(&backends);
                std::thread::spawn(move || {
                    if let Err(e) = handle_tcp(stream, &backends) {
                        eprintln!("client error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

fn serve_unix(path: &str, backends: Arc<Backends>) {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    let listener = UnixListener::bind(path).expect("bind bridge unix");
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                let backends = Arc::clone(&backends);
                std::thread::spawn(move || {
                    if let Err(e) = handle_unix(stream, &backends) {
                        eprintln!("client error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

fn handle_tcp(stream: TcpStream, backends: &Backends) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
    let writer = stream.try_clone()?;
    let reader = BufReader::new(stream);
    handle_client(reader, writer, backends)
}

fn handle_unix(stream: UnixStream, backends: &Backends) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
    let writer = stream.try_clone()?;
    let reader = BufReader::new(stream);
    handle_client(reader, writer, backends)
}

fn handle_client<R: Read, W: Write>(
    mut reader: BufReader<R>,
    mut writer: W,
    backends: &Backends,
) -> std::io::Result<()> {
    let mut raw = String::new();
    loop {
        if read_line_bounded(&mut reader, &mut raw)?.is_none() {
            break;
        }
        // UEFI/Limine also write to COM2 under UTM; strip CSI/controls so a
        // guest `PING\n` that shared a line with firmware noise still parses.
        let line = scrub_protocol_line(&raw);
        if line.is_empty() {
            continue;
        }
        eprintln!("← {line}");

        // Multi-line save: CALL skills.save name=foo  then LINE… END.
        // A one-line form with desc= creates a starter skill with no body read,
        // so a client that never sends LINE/END cannot desync the protocol.
        if line.starts_with("CALL skills.save ") {
            let args = parse_args(line.trim_start_matches("CALL skills.save "));
            let name = arg_val(&args, "name").unwrap_or("").to_string();
            let reply = if let Some(desc) = arg_val(&args, "desc") {
                let body = format!(
                    "---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n\n(edit me)\n"
                );
                match skills::save_skill(&name, &body) {
                    Ok(path) => vec![format!("OK skills.save path={}", path.display())],
                    Err(e) => vec![format!("ERR skills.save {e}")],
                }
            } else {
                let mut body = String::new();
                let mut ended = false;
                loop {
                    if read_line_bounded(&mut reader, &mut raw)?.is_none() {
                        break;
                    }
                    let t = raw.trim_end_matches(['\r', '\n']);
                    if t == "END" {
                        ended = true;
                        break;
                    }
                    if let Some(rest) = t.strip_prefix("LINE ") {
                        if body.len() + rest.len() + 1 > MAX_BODY {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "skills.save body exceeds MAX_BODY",
                            ));
                        }
                        body.push_str(rest);
                        body.push('\n');
                    }
                }
                if !ended {
                    // Disconnect mid-body: do not write a truncated skill.
                    vec!["ERR skills.save truncated_body".into()]
                } else {
                    match skills::save_skill(&name, &body) {
                        Ok(path) => vec![format!("OK skills.save path={}", path.display())],
                        Err(e) => vec![format!("ERR skills.save {e}")],
                    }
                }
            };
            for r in &reply {
                eprintln!("→ {r}");
                writeln!(writer, "{r}")?;
            }
            writer.flush()?;
            continue;
        }

        let reply = dispatch(&line, backends);
        if reply.is_empty() {
            continue;
        }
        for r in &reply {
            eprintln!("→ {r}");
            writeln!(writer, "{r}")?;
        }
        writer.flush()?;
    }
    Ok(())
}

/// Drop ANSI CSI sequences and other controls; keep printable ASCII protocol.
fn scrub_protocol_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if c == '\r' || c == '\n' || c == '\t' {
            continue;
        }
        if c.is_control() {
            continue;
        }
        out.push(c);
    }
    // Firmware may leave prose before the guest command on the same "line".
    for prefix in ["CALL ", "PING", "LIST"] {
        if let Some(i) = out.find(prefix) {
            return out[i..].trim().to_string();
        }
    }
    out.trim().to_string()
}

fn dispatch(line: &str, backends: &Backends) -> Vec<String> {
    let (cmd, rest) = split_word(line);
    match cmd {
        "PING" => vec!["OK pong".into()],
        "LIST" => {
            vec!["OK tools=email.search,email.send,calendar.list,skills.list,skills.get,skills.save,search.query,workspace.index,tsearch.sync".into()]
        }
        "CALL" => {
            let (tool, rest) = split_word(rest);
            let args = parse_args(rest);
            call_tool(tool, &args, backends)
        }
        _ => {
            // Ignore UEFI/Limine console noise on the same COM2 pipe.
            Vec::new()
        }
    }
}

/// Split off the first whitespace-delimited word; the remainder keeps its
/// internal spacing (values may contain spaces per the wire protocol doc).
fn split_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    match s.find(char::is_whitespace) {
        Some(i) => (&s[..i], s[i..].trim_start()),
        None => (s, ""),
    }
}

/// Parse `key=value` pairs where values may contain spaces (per the wire
/// protocol doc). A new pair starts at a token whose prefix before `=` looks
/// like a key (`[a-z][a-z0-9_]*`); other tokens extend the current value.
fn parse_args(rest: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for tok in rest.split_whitespace() {
        let key = tok.split_once('=').map(|(k, _)| k);
        let is_key = key.is_some_and(|k| {
            let mut ch = k.chars();
            ch.next().is_some_and(|c| c.is_ascii_lowercase())
                && k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        });
        if is_key {
            let (k, v) = tok.split_once('=').unwrap();
            out.push((k.to_string(), v.to_string()));
        } else if let Some(last) = out.last_mut() {
            if !last.1.is_empty() {
                last.1.push(' ');
            }
            last.1.push_str(tok);
        }
    }
    out
}

fn call_tool(tool: &str, args: &[(String, String)], backends: &Backends) -> Vec<String> {
    match tool {
        "email.search" => email_search(args, &backends.email),
        "email.send" => vec!["ERR email.send disabled_until_cap_confirm".into()],
        "calendar.list" => vec![
            "OK calendar.list n=1".into(),
            "ROW title=Demo event|when=tomorrow".into(),
            "END".into(),
        ],
        "skills.list" => skills::list_response(),
        "skills.get" => {
            let name = arg_val(args, "name").unwrap_or("");
            skills::get_response(name)
        }
        "skills.save" => {
            // Reached only via dispatch (tests); the socket path handles
            // skills.save in handle_client so it can read a LINE…END body.
            let name = arg_val(args, "name").unwrap_or("");
            let desc = arg_val(args, "desc").unwrap_or("User-saved skill.");
            let body = format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n\n(edit me)\n");
            match skills::save_skill(name, &body) {
                Ok(path) => vec![format!("OK skills.save path={}", path.display())],
                Err(e) => vec![format!("ERR skills.save {e}")],
            }
        }
        "tsearch.sync" => match tsearch::sync() {
            Ok((n, at)) => vec![
                format!("OK tsearch.sync n={n} crawled={at}"),
                "END".into(),
            ],
            Err(e) => vec![format!("ERR tsearch.sync {e}")],
        },
        "workspace.index" => {
            // Building the index reads the user's files, so it needs the same
            // grant as searching them.
            if !matches!(arg_val(args, "files"), Some("1")) {
                return vec!["ERR workspace.index needs_workspace_cap".into()];
            }
            let roots = workspace::roots();
            let ix = workspace::build(&roots);
            let n = ix.entries.len();
            match ix.save() {
                Ok(p) => vec![
                    format!("OK workspace.index n={n} path={}", p.display()),
                    "END".into(),
                ],
                Err(e) => vec![format!("ERR workspace.index {e}")],
            }
        }
        "search.query" => {
            let q = arg_val(args, "q").unwrap_or("");
            let k: usize = arg_val(args, "k")
                .and_then(|s| s.parse().ok())
                .unwrap_or(5)
                .clamp(1, 20);
            let cat = arg_val(args, "cat");
            // Email content is opt-in per call. The guest only sets this when
            // the user granted email.search at setup, so holding search.query
            // alone cannot reach mail.
            let with_email = matches!(arg_val(args, "email"), Some("1"));
            let with_files = matches!(arg_val(args, "files"), Some("1"));
            if q.is_empty() {
                vec!["ERR search.query missing_q".into()]
            } else {
                search::query_scoped(q, k, cat, &backends.search, with_email, with_files)
            }
        }
        _ => vec![format!("ERR {tool} not_found")],
    }
}

/// Parse `ROW from=…|subj=…` lines back into graph messages.
///
/// Only sender and subject are kept — never the body.
fn ingest_rows(rows: &[String]) {
    let msgs: Vec<(String, String, String)> = rows
        .iter()
        .filter_map(|r| {
            let from = parse_row_field(r, "from")?;
            let subj = parse_row_field(r, "subj").unwrap_or("");
            Some((from.to_string(), subj.to_string(), String::new()))
        })
        .collect();
    if msgs.is_empty() {
        return;
    }
    let mut g = graph::Graph::load_or_empty();
    if g.ingest(&msgs) > 0 {
        if let Err(e) = g.save() {
            eprintln!("graph: save failed: {e}");
        }
    }
}

/// Read `key=value` out of a `ROW a=1|b=2` line.
fn parse_row_field<'a>(row: &'a str, key: &str) -> Option<&'a str> {
    let body = row.strip_prefix("ROW ")?;
    body.split('|')
        .find_map(|f| f.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn arg_val<'a>(args: &'a [(String, String)], key: &str) -> Option<&'a str> {
    args.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn email_search(args: &[(String, String)], backend: &str) -> Vec<String> {
    let query = arg_val(args, "q").unwrap_or("in:inbox");
    let max: usize = arg_val(args, "max")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
        .clamp(1, 20);

    let out = match backend {
        "gog" => email_search_gog(query, max),
        _ => email_search_mock(query, max),
    };
    // Fold what we just fetched into the knowledge graph. Hooked here rather
    // than inside a backend so every backend feeds it. Best effort: failing to
    // index must not fail the search the caller asked for.
    ingest_rows(&out);
    out
}

fn email_search_mock(query: &str, max: usize) -> Vec<String> {
    let query = sanitize_field(query);
    let samples = [
        ("Alice Chen", "Q2 planning notes"),
        ("GitHub", "Your Actions workflow run"),
        ("os bridge", &format!("Mock hit for {query}")),
    ];
    let n = samples.len().min(max);
    let mut out = vec![format!("OK email.search n={n}")];
    for (from, subj) in samples.iter().take(n) {
        out.push(format!("ROW from={from}|subj={subj}"));
    }
    out.push("END".into());
    out
}

fn email_search_gog(query: &str, max: usize) -> Vec<String> {
    // `--` stops flag parsing so an untrusted query cannot inject gog flags.
    let output = Command::new("gog")
        .args([
            "gmail",
            "search",
            "-j",
            "--results-only",
            "--no-input",
            "--",
            query,
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            let brief = err
                .lines()
                .next()
                .unwrap_or("gog_failed")
                .chars()
                .take(80)
                .collect::<String>();
            return vec![format!("ERR email.search {brief}")];
        }
        Err(_) => return vec!["ERR email.search gog_missing".into()],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut rows = Vec::new();

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let items = val
            .as_array()
            .cloned()
            .or_else(|| val.get("threads").and_then(|t| t.as_array().cloned()))
            .or_else(|| val.get("messages").and_then(|t| t.as_array().cloned()))
            .unwrap_or_default();

        for item in items.into_iter().take(max) {
            let from = item
                .pointer("/from")
                .or_else(|| item.pointer("/sender"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let subj = item
                .get("subject")
                .or_else(|| item.get("snippet"))
                .and_then(|v| v.as_str())
                .unwrap_or("(no subject)");
            rows.push(format!(
                "ROW from={}|subj={}",
                sanitize_field(from),
                sanitize_field(subj)
            ));
        }
    }

    if rows.is_empty() {
        for line in stdout.lines().take(max) {
            let line = sanitize_field(line);
            if !line.is_empty() {
                rows.push(format!("ROW from=gog|subj={line}"));
            }
        }
    }

    let n = rows.len();
    let mut out = vec![format!("OK email.search n={n}")];
    out.extend(rows);
    out.push("END".into());
    out
}

fn sanitize_field(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\n' | '\r' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .take(90)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_search_returns_rows() {
        let r = email_search_mock("in:inbox", 2);
        assert!(r[0].starts_with("OK email.search n=2"));
        assert!(r.iter().any(|l| l.starts_with("ROW ")));
        assert_eq!(r.last().map(String::as_str), Some("END"));
    }

    fn test_backends() -> Backends {
        Backends {
            email: "mock".into(),
            search: "tfidf".into(),
        }
    }

    #[test]
    fn dispatch_ping() {
        assert_eq!(dispatch("PING", &test_backends()), vec!["OK pong".to_string()]);
    }

    #[test]
    fn scrub_recovers_ping_after_uefi_csi() {
        let raw = "\u{1b}[2J\u{1b}[01;01HPING\n";
        assert_eq!(scrub_protocol_line(raw), "PING");
        let raw2 = "BdsDxe: loading...\n";
        assert!(!scrub_protocol_line(raw2).starts_with("PING"));
        let raw3 = "\u{1b}[0mCALL email.search q=x\n";
        assert_eq!(scrub_protocol_line(raw3), "CALL email.search q=x");
    }

    #[test]
    fn dispatch_skills_list() {
        let r = dispatch("CALL skills.list", &test_backends());
        assert!(r[0].starts_with("OK skills.list"));
        assert!(r.iter().any(|l| l.contains("email-triage")));
    }

    #[test]
    fn dispatch_search_query() {
        let r = dispatch("CALL search.query q=capability k=3", &test_backends());
        assert!(r[0].starts_with("OK search.query"), "{r:?}");
        assert!(r.iter().any(|l| l.starts_with("ROW ")));
        assert_eq!(r.last().map(String::as_str), Some("END"));
    }

    #[test]
    fn list_includes_search() {
        let r = dispatch("LIST", &test_backends());
        assert!(r[0].contains("search.query"));
    }
}
