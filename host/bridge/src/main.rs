//! Host-side MCP-shaped connector bridge.
//!
//! Speaks the line protocol in docs/mcp-connectors-os-doc-v01.md over TCP.
//! Email backends: mock (default) or `gog`. Skills: defaults + saved on host.

mod graph;
mod paths;
mod workspace;
mod search;
mod text;
mod transcribe;
mod tsearch;
mod skills;

use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::Arc;

/// Longest request line we accept from the wire; the peer is untrusted.
const MAX_LINE: u64 = 64 * 1024;
/// Cap on an accumulated skills.save body.
const MAX_BODY: usize = 1024 * 1024;

/// Tools advertised by `LIST` — keep in sync with `call_tool` (+ `skills.save`
/// which is handled on the socket for the LINE…END body).
const TOOLS: &[&str] = &[
    "email.search",
    "email.send",
    "skills.list",
    "skills.get",
    "skills.save",
    "search.query",
    "workspace.index",
    "tsearch.sync",
    "audio.transcribe",
    "workspace.forget",
    "audio.forget",
    "doc.read",
];

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
}

fn main() {
    let connect = env::var("OS_MCP_BRIDGE_CONNECT").ok();
    let addr = env::var("OS_MCP_BRIDGE_ADDR").unwrap_or_else(|_| "127.0.0.1:7420".into());
    let backends = Arc::new(Backends {
        email: env::var("OS_MCP_EMAIL_BACKEND").unwrap_or_else(|_| "mock".into()),
    });

    let (defaults, user) = skills::skills_dirs();
    let where_ = connect.as_deref().unwrap_or(addr.as_str());
    eprintln!(
        "os-mcp-bridge {} on {where_} (email={}; skills defaults={} user={})",
        if connect.is_some() { "connecting" } else { "listening" },
        backends.email,
        defaults.display(),
        user.display()
    );

    if let Some(target) = connect {
        // Guest (QEMU/UTM) listens; we dial and retry. One topology everywhere.
        let addr = target.strip_prefix("tcp:").unwrap_or(target.as_str());
        dial_loop(addr, backends);
    } else {
        // Foreground debug only (`make bridge-run` + nc). Bridged runs dial.
        serve_tcp(&addr, backends);
    }
}

/// Dial COM2 (guest TcpServer). Retries until the guest appears, serves one
/// session, reconnects. QEMU's TcpClient mode does not retry — that is why the
/// bridge is always the client for interactive runs.
fn dial_loop(addr: &str, backends: Arc<Backends>) {
    loop {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                eprintln!("connected to tcp:{addr}");
                if let Err(e) = handle_tcp(stream, &backends) {
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

fn handle_tcp(stream: TcpStream, backends: &Backends) -> std::io::Result<()> {
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
                save_skill_reply(&name, &body)
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
                    save_skill_reply(&name, &body)
                }
            };
            write_reply(&mut writer, &reply)?;
            continue;
        }

        let reply = dispatch(&line, backends);
        if !reply.is_empty() {
            write_reply(&mut writer, &reply)?;
        }
    }
    Ok(())
}

fn write_reply<W: Write>(writer: &mut W, reply: &[String]) -> std::io::Result<()> {
    for r in reply {
        eprintln!("→ {r}");
        writeln!(writer, "{r}")?;
    }
    writer.flush()
}

fn save_skill_reply(name: &str, body: &str) -> Vec<String> {
    match skills::save_skill(name, body) {
        Ok(path) => vec![format!("OK skills.save path={}", path.display())],
        Err(e) => vec![format!("ERR skills.save {e}")],
    }
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
        "LIST" => vec![format!("OK tools={}", TOOLS.join(","))],
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

/// Delete a capability-produced store. Missing file is success (`nothing_to_remove`).
fn forget_file(tool: &str, path: &std::path::Path) -> Vec<String> {
    match std::fs::remove_file(path) {
        Ok(()) => text::framed_ok(format!("OK {tool} removed"), []),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            text::framed_ok(format!("OK {tool} nothing_to_remove"), [])
        }
        Err(e) => vec![format!("ERR {tool} {e}")],
    }
}

fn call_tool(tool: &str, args: &[(String, String)], backends: &Backends) -> Vec<String> {
    match tool {
        "email.search" => email_search(args, &backends.email),
        "email.send" => vec!["ERR email.send disabled_until_cap_confirm".into()],
        "skills.list" => skills::list_response(),
        "skills.get" => {
            let name = arg_val(args, "name").unwrap_or("");
            skills::get_response(name)
        }
        // skills.save is handled in handle_client (LINE…END body on the socket).
        "audio.transcribe" => {
            // Reads media and puts the words in a searchable index, so it
            // needs the grant just like workspace.index does.
            if !matches!(arg_val(args, "audio"), Some("1")) {
                return vec!["ERR audio.transcribe needs_audio_cap".into()];
            }
            let Some(path) = arg_val(args, "path") else {
                return vec!["ERR audio.transcribe missing_path".into()];
            };
            match transcribe::transcribe(std::path::Path::new(path)) {
                Ok(t) => {
                    let mut store = transcribe::Store::load();
                    let summary = transcribe::summarize(&t.text, 3);
                    let (title, words, secs) = (t.title.clone(), t.words, t.seconds);
                    store.upsert(t);
                    let _ = store.save();
                    let mut rows = vec![format!(
                        "ROW field=title|value={}",
                        sanitize_field(&title)
                    )];
                    rows.extend(summary.into_iter().map(|line| {
                        format!("ROW field=summary|value={}", sanitize_field(&line))
                    }));
                    text::framed_ok(
                        format!("OK audio.transcribe words={words} seconds={secs:.0}"),
                        rows,
                    )
                }
                Err(e) => vec![format!("ERR audio.transcribe {e}")],
            }
        }
        "tsearch.sync" => match tsearch::sync() {
            Ok((n, at)) => text::framed_ok(format!("OK tsearch.sync n={n} crawled={at}"), []),
            Err(e) => vec![format!("ERR tsearch.sync {e}")],
        },
        // Revoking a grant should remove what it produced, not merely hide it.
        // Read one indexed document back, so a result can be opened rather
        // than merely located.
        "doc.read" => {
            let Some(url) = arg_val(args, "url") else {
                return vec!["ERR doc.read missing_url".into()];
            };
            let max: usize = arg_val(args, "lines")
                .and_then(|s| s.parse().ok())
                .unwrap_or(24)
                .clamp(1, 200);
            match read_doc(url, max, args) {
                Ok(lines) => {
                    text::framed_ok(format!("OK doc.read n={}", lines.len()), lines)
                }
                Err(e) => vec![format!("ERR doc.read {e}")],
            }
        }
        "workspace.forget" => forget_file("workspace.forget", &workspace::index_path()),
        "audio.forget" => forget_file("audio.forget", &transcribe::store_path()),
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
                Ok(p) => text::framed_ok(
                    format!("OK workspace.index n={n} path={}", p.display()),
                    [],
                ),
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
            // Recordings have their own grant, so they get their own scope:
            // enabling workspace.index must not surface transcripts.
            let with_audio = matches!(arg_val(args, "audio"), Some("1"));
            if q.is_empty() {
                vec!["ERR search.query missing_q".into()]
            } else {
                search::query_all(q, k, cat, with_email, with_files, with_audio)
            }
        }
        // Distinct from tool-specific `… not_found` (e.g. skills.get).
        _ => vec![format!("ERR unknown_tool {tool}")],
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

/// Resolve a result URL back to readable text.
///
/// Scope is checked per source, using the same flags as `search.query`: a
/// caller that could not have found the document must not be able to read it
/// by guessing its URL.
fn read_doc(url: &str, max: usize, args: &[(String, String)]) -> Result<Vec<String>, String> {
    let with_files = matches!(arg_val(args, "files"), Some("1"));
    let with_audio = matches!(arg_val(args, "audio"), Some("1"));

    let body = if let Some(rel) = url.strip_prefix("file://") {
        if !with_files {
            return Err("needs_workspace_cap".into());
        }
        // Resolve against the configured roots rather than trusting the path,
        // so "../.." cannot escape into the rest of the filesystem.
        let mut found = None;
        for root in workspace::roots() {
            let candidate = root.join(rel);
            if let Ok(real) = candidate.canonicalize() {
                if let Ok(root_real) = root.canonicalize() {
                    if real.starts_with(&root_real) && real.is_file() {
                        found = Some(real);
                        break;
                    }
                }
            }
        }
        let path = found.ok_or("outside the indexed roots")?;
        std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?
    } else if let Some(src) = url.strip_prefix("audio://") {
        if !with_audio {
            return Err("needs_audio_cap".into());
        }
        transcribe::Store::load()
            .items
            .into_iter()
            .find(|t| t.source == src)
            .map(|t| t.text)
            .ok_or("no such transcript")?
    } else {
        // Corpus and teddysearch documents carry their body in the index.
        return search::body_for(url, max).ok_or_else(|| "no readable body".into());
    };

    Ok(search::wrap_lines(&body, 78, max))
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
    let rows = samples
        .iter()
        .take(n)
        .map(|(from, subj)| format!("ROW from={from}|subj={subj}"));
    text::framed_ok(format!("OK email.search n={n}"), rows)
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
    text::framed_ok(format!("OK email.search n={n}"), rows)
}

fn sanitize_field(s: &str) -> String {
    text::sanitize(s, 90, false, true)
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

    #[test]
    fn list_matches_tools_table() {
        let r = dispatch("LIST", &test_backends());
        assert_eq!(r[0], format!("OK tools={}", TOOLS.join(",")));
    }

    #[test]
    fn every_listed_tool_is_dispatched() {
        for tool in TOOLS {
            if *tool == "skills.save" {
                // Multi-line body protocol lives in handle_client, not call_tool.
                continue;
            }
            let r = dispatch(&format!("CALL {tool}"), &test_backends());
            assert!(
                !r.first().is_some_and(|s| s.starts_with("ERR unknown_tool ")),
                "{tool} listed but missing from call_tool: {r:?}"
            );
        }
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;

    fn args(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn reading_a_file_needs_the_workspace_grant() {
        // Guessing a URL must not bypass the grant that would have found it.
        let e = read_doc("file://a/b.md", 10, &args(&[])).unwrap_err();
        assert_eq!(e, "needs_workspace_cap");
    }

    #[test]
    fn reading_a_transcript_needs_the_audio_grant() {
        let e = read_doc("audio:///tmp/x.wav", 10, &args(&[("files", "1")])).unwrap_err();
        assert_eq!(e, "needs_audio_cap", "the files grant must not unlock recordings");
    }

    #[test]
    fn traversal_outside_the_indexed_roots_is_refused() {
        let e = read_doc("file://../../../../etc/passwd", 10, &args(&[("files", "1")]))
            .unwrap_err();
        assert!(e.contains("outside the indexed roots"), "{e}");
    }

    #[test]
    fn wrapping_respects_the_width_and_line_cap() {
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi";
        let rows = search::wrap_lines(text, 20, 3);
        assert!(rows.len() <= 3);
        for r in &rows {
            let line = r.strip_prefix("ROW line=").unwrap();
            assert!(line.chars().count() <= 20, "line too wide: {line:?}");
        }
    }

    #[test]
    fn blank_lines_survive_as_paragraph_breaks() {
        let rows = search::wrap_lines("one\n\ntwo", 40, 10);
        assert!(rows.iter().any(|r| r == "ROW line="), "paragraph break lost");
    }

    #[test]
    fn a_word_longer_than_the_width_does_not_loop_forever() {
        let rows = search::wrap_lines(&"x".repeat(300), 20, 5);
        assert!(!rows.is_empty());
        assert!(rows.len() <= 5);
    }
}
