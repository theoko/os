//! Host-side MCP-shaped connector bridge.
//!
//! Speaks the line protocol in docs/mcp-connectors-os-doc-v01.md over TCP.
//! Email backends: mock (default) or `gog`. Skills: defaults + saved on host.

mod paths;
mod workspace;
mod search;
mod text;
mod transcribe;
mod tsearch;
mod skills;

#[cfg(test)]
mod budgets_lockstep;

use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;

/// Longest request line we accept from the wire; the peer is untrusted.
const MAX_LINE: u64 = 64 * 1024;
/// Cap on an accumulated skills.save body.
const MAX_BODY: usize = 1024 * 1024;

/// `read_line` with a hard length cap so a peer that never sends `\n` cannot
/// grow the buffer without bound. `Ok(true)` = got a line, `Ok(false)` = EOF.
fn read_line_bounded<R: BufRead>(reader: &mut R, line: &mut String) -> std::io::Result<bool> {
    line.clear();
    let n = reader.take(MAX_LINE).read_line(line)?;
    if n == 0 {
        return Ok(false);
    }
    if n as u64 == MAX_LINE && !line.ends_with('\n') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "line exceeds MAX_LINE",
        ));
    }
    Ok(true)
}

fn main() {
    let connect = env::var("OS_MCP_BRIDGE_CONNECT").ok();
    let addr = env::var("OS_MCP_BRIDGE_ADDR").unwrap_or_else(|_| "127.0.0.1:7420".into());
    let email = env::var("OS_MCP_EMAIL_BACKEND").unwrap_or_else(|_| "mock".into());

    let (defaults, user) = skills::skills_dirs();
    let where_ = connect.as_deref().unwrap_or(addr.as_str());
    eprintln!(
        "os-mcp-bridge {} on {where_} (email={email}; skills defaults={} user={})",
        if connect.is_some() { "connecting" } else { "listening" },
        defaults.display(),
        user.display()
    );

    if let Some(target) = connect {
        // Guest (QEMU/UTM) listens; we dial and retry. One topology everywhere.
        let addr = target.strip_prefix("tcp:").unwrap_or(target.as_str());
        dial_loop(addr);
    } else {
        // Foreground debug only (`make bridge-run` + nc). Bridged runs dial.
        serve_tcp(&addr);
    }
}

/// Dial COM2 (guest TcpServer). Retries until the guest appears, serves one
/// session, reconnects. QEMU's TcpClient mode does not retry — that is why the
/// bridge is always the client for interactive runs.
fn dial_loop(addr: &str) {
    loop {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                eprintln!("connected to tcp:{addr}");
                if let Err(e) = handle_tcp(stream) {
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

fn serve_tcp(addr: &str) {
    let listener = TcpListener::bind(addr).expect("bind bridge tcp");
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(e) = handle_tcp(stream) {
                        eprintln!("client error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

fn handle_tcp(stream: TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
    let writer = stream.try_clone()?;
    let reader = BufReader::new(stream);
    handle_client(reader, writer)
}

fn handle_client<R: Read, W: Write>(
    mut reader: BufReader<R>,
    mut writer: W,
) -> std::io::Result<()> {
    let mut raw = String::new();
    loop {
        if !read_line_bounded(&mut reader, &mut raw)? {
            break;
        }
        // UEFI/Limine also write to COM2 under UTM; strip CSI/controls so a
        // guest `PING\n` that shared a line with firmware noise still parses.
        let line = scrub_protocol_line(&raw);
        if line.is_empty() {
            continue;
        }
        eprintln!("← {line}");

        // CALL skills.save name=foo  then LINE… END (sole save shape).
        if line.starts_with("CALL skills.save ") {
            let args = parse_args(line.trim_start_matches("CALL skills.save "));
            let name = arg_val(&args, "name").unwrap_or("");
            let mut body = String::new();
            let mut ended = false;
            loop {
                if !read_line_bounded(&mut reader, &mut raw)? {
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
            let reply = if !ended {
                // Disconnect mid-body: do not write a truncated skill.
                vec!["ERR skills.save truncated_body".into()]
            } else {
                match skills::save_skill(name, &body) {
                    Ok(_) => text::framed_ok("OK skills.save".into(), []),
                    Err(e) => vec![format!("ERR skills.save {e}")],
                }
            };
            write_reply(&mut writer, &reply)?;
            continue;
        }

        let reply = dispatch(&line);
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
    if let Some(i) = ["CALL ", "PING"].iter().find_map(|p| out.find(p)) {
        out.drain(..i);
    }
    let end = out.trim_end().len();
    out.truncate(end);
    let lead = out.len() - out.trim_start().len();
    if lead > 0 {
        out.drain(..lead);
    }
    out
}

fn dispatch(line: &str) -> Vec<String> {
    let (cmd, rest) = split_word(line);
    match cmd {
        "PING" => vec!["OK pong".into()],
        "CALL" => {
            let (tool, rest) = split_word(rest);
            let args = parse_args(rest);
            call_tool(tool, &args)
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
        if let Some((k, v)) = tok.split_once('=') {
            let is_key = {
                let mut ch = k.chars();
                ch.next().is_some_and(|c| c.is_ascii_lowercase())
                    && k.chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            };
            if is_key {
                out.push((k.to_string(), v.to_string()));
                continue;
            }
        }
        if let Some(last) = out.last_mut() {
            if !last.1.is_empty() {
                last.1.push(' ');
            }
            last.1.push_str(tok);
        }
    }
    out
}

/// Delete a capability-produced store. Missing file is success.
fn forget_file(tool: &str, path: &std::path::Path) -> Vec<String> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return vec![format!("ERR {tool} {e}")],
    }
    text::framed_ok(format!("OK {tool}"), [])
}

/// Mock CI peek count (`email.search`). Gog does not use this as a clamp.
/// Guest omits args on the wire; `GUEST_MAX_HITS` / `GUEST_DOC_LINES` are
/// defaults when nc omits `k=` / `lines=`.
const GUEST_MAIL_MAX: usize = 3;
const GUEST_MAX_HITS: usize = 3;
const GUEST_DOC_LINES: usize = 18;

fn call_tool(tool: &str, args: &[(String, String)]) -> Vec<String> {
    match tool {
        "email.search" => {
            // Peek is `ROW n=<count>` only — no message ROWs. Mock uses a fixed
            // CI count; gog reports the true match count (no leftover max= clamp).
            let backend = env::var("OS_MCP_EMAIL_BACKEND").unwrap_or_else(|_| "mock".into());
            match backend.as_str() {
                "gog" => email_search_gog(arg_val(args, "q").unwrap_or("in:inbox")),
                _ => email_count_ok(GUEST_MAIL_MAX),
            }
        }
        "email.send" => vec![format!("ERR {tool} disabled_until_cap_confirm")],
        "skills.list" => skills::list_response(),
        // skills.save is handled in handle_client (LINE…END body on the socket).
        "audio.transcribe" => {
            // Reads media and puts the words in a searchable index, so it
            // needs the grant just like workspace.index does. Empty framed OK
            // — guest never read title/summary ROWs (search indexes the store).
            if !arg_flag(args, "audio") {
                return vec![format!("ERR {tool} needs_audio_cap")];
            }
            let Some(path) = arg_val(args, "path") else {
                return vec![format!("ERR {tool} missing_path")];
            };
            match transcribe::transcribe(std::path::Path::new(path)) {
                Ok(t) => {
                    let mut store = transcribe::Store::load();
                    store.upsert(t);
                    match store.save() {
                        Ok(()) => text::framed_ok(format!("OK {tool}"), []),
                        Err(e) => vec![format!("ERR {tool} {e}")],
                    }
                }
                Err(e) => vec![format!("ERR {tool} {e}")],
            }
        }
        // Revoking a grant should remove what it produced, not merely hide it.
        // Read one indexed document back, so a result can be opened rather
        // than merely located.
        "doc.read" => {
            let Some(url) = arg_val(args, "url") else {
                return vec![format!("ERR {tool} missing_url")];
            };
            // Guest omits lines=; default matches guest `DocPage::MAX`.
            let max = arg_usize(args, "lines", GUEST_DOC_LINES, 200);
            match read_doc(url, max, arg_flag(args, "files"), arg_flag(args, "audio")) {
                Ok(lines) => text::framed_ok(format!("OK {tool}"), lines),
                Err(e) => vec![format!("ERR {tool} {e}")],
            }
        }
        "workspace.forget" => forget_file(tool, &workspace::index_path()),
        "audio.forget" => forget_file(tool, &transcribe::store_path()),
        "workspace.index" => {
            // Force-rebuild for nc / host. Guest search with files=1 lazy-builds
            // when the on-disk index is empty (see search::workspace_docs).
            if !arg_flag(args, "files") {
                return vec![format!("ERR {tool} needs_workspace_cap")];
            }
            let roots = workspace::roots();
            let ix = workspace::build(&roots);
            match ix.save() {
                Ok(()) => text::framed_ok(format!("OK {tool}"), []),
                Err(e) => vec![format!("ERR {tool} {e}")],
            }
        }
        "search.query" => {
            let q = arg_val(args, "q").unwrap_or("");
            // Guest omits k=; default matches guest `search::MAX_HITS`.
            let k = arg_usize(args, "k", GUEST_MAX_HITS, 20);
            let with_files = arg_flag(args, "files");
            // Recordings have their own grant, so they get their own scope:
            // enabling Files must not surface transcripts.
            let with_audio = arg_flag(args, "audio");
            if q.is_empty() {
                vec![format!("ERR {tool} missing_q")]
            } else {
                search::query_all(q, k, with_files, with_audio)
            }
        }
        // Distinct from tool-specific `… not_found` replies.
        _ => vec![format!("ERR unknown_tool {tool}")],
    }
}

/// Resolve a result URL back to readable text.
///
/// Scope is checked per source, using the same flags as `search.query`: a
/// caller that could not have found the document must not be able to read it
/// by guessing its URL.
fn read_doc(
    url: &str,
    max: usize,
    with_files: bool,
    with_audio: bool,
) -> Result<Vec<String>, String> {
    use std::borrow::Cow;

    let body: Cow<'_, str> = if let Some(rel) = url.strip_prefix("file://") {
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
        Cow::Owned(std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?)
    } else if let Some(src) = url.strip_prefix("audio://") {
        if !with_audio {
            return Err("needs_audio_cap".into());
        }
        Cow::Owned(
            transcribe::Store::load()
                .items
                .into_iter()
                .find(|t| t.source == src)
                .map(|t| t.text)
                .ok_or("no such transcript")?,
        )
    } else {
        // Corpus and teddysearch documents carry their body in the index.
        let body = search::load_docs()
            .ok()
            .and_then(|docs| docs.iter().find(|d| d.u == url).map(|d| d.b.as_str()))
            .or_else(|| {
                tsearch::docs()
                    .iter()
                    .find(|d| d.u == url)
                    .map(|d| d.b.as_str())
            })
            .ok_or("no readable body")?;
        Cow::Borrowed(body)
    };

    Ok(search::wrap_lines(&body, max))
}

fn arg_val<'a>(args: &'a [(String, String)], key: &str) -> Option<&'a str> {
    args.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Scope / grant bits on the wire are `key=1`.
fn arg_flag(args: &[(String, String)], key: &str) -> bool {
    matches!(arg_val(args, key), Some("1"))
}

fn arg_usize(args: &[(String, String)], key: &str, default: usize, max: usize) -> usize {
    arg_val(args, key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
        .clamp(1, max)
}

/// Guest mail peek reads one `ROW n=<count>` (no per-message payloads).
fn email_count_ok(n: usize) -> Vec<String> {
    text::framed_ok("OK email.search".into(), [format!("ROW n={n}")])
}

fn email_search_gog(query: &str) -> Vec<String> {
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
            let brief = text::stderr_brief(&o.stderr, "gog_failed", 80);
            return vec![format!("ERR email.search {brief}")];
        }
        Err(_) => return vec!["ERR email.search gog_missing".into()],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut n = 0usize;

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
        let items: &[serde_json::Value] = val
            .as_array()
            .map(|a| a.as_slice())
            .or_else(|| val.get("threads").and_then(|t| t.as_array()).map(|a| a.as_slice()))
            .or_else(|| val.get("messages").and_then(|t| t.as_array()).map(|a| a.as_slice()))
            .unwrap_or(&[]);
        // Count-only peek: report how many hits gog returned (do not re-clamp).
        n = items.len();
    }

    if n == 0 {
        n = stdout.lines().filter(|l| !l.trim().is_empty()).count();
    }

    email_count_ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_search_returns_rows() {
        let r = email_count_ok(2);
        assert_eq!(r[0], "OK email.search");
        assert_eq!(r.iter().filter(|l| l.starts_with("ROW ")).count(), 1);
        assert!(r.iter().any(|l| l == "ROW n=2"), "{r:?}");
        assert_eq!(r.last().map(String::as_str), Some("END"));
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
        let r = dispatch("CALL skills.list");
        assert_eq!(r[0], "OK skills.list");
        assert!(r.iter().any(|l| l.contains("email-triage")));
    }

    #[test]
    fn dispatch_search_query() {
        // Guest omits k=; default is MAX_HITS (3).
        let r = dispatch("CALL search.query q=capability");
        assert_eq!(r[0], "OK search.query");
        assert!(r.iter().any(|l| l.starts_with("ROW ")));
        assert_eq!(r.last().map(String::as_str), Some("END"));
        assert!(
            r.iter().filter(|l| l.starts_with("ROW ")).count() <= 3,
            "default k must cap at guest MAX_HITS"
        );
    }

    #[test]
    fn ping_answers() {
        assert_eq!(dispatch("PING"), vec!["OK pong".to_string()]);
    }

    #[test]
    fn every_listed_tool_is_dispatched() {
        // skills.save is LINE…END on the socket — not in call_tool.
        for tool in [
            "email.search",
            "email.send",
            "skills.list",
            "search.query",
            "workspace.index",
            "audio.transcribe",
            "workspace.forget",
            "audio.forget",
            "doc.read",
        ] {
            let r = dispatch(&format!("CALL {tool}"));
            assert!(
                !r.first().is_some_and(|s| s.starts_with("ERR unknown_tool ")),
                "{tool} missing from call_tool: {r:?}"
            );
        }
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;

    #[test]
    fn reading_needs_the_matching_grant() {
        // Guessing a URL must not bypass the grant that would have found it.
        for (url, files, audio, err) in [
            ("file://a/b.md", false, false, "needs_workspace_cap"),
            ("audio:///tmp/x.wav", true, false, "needs_audio_cap"),
        ] {
            assert_eq!(read_doc(url, 10, files, audio).unwrap_err(), err, "{url}");
        }
    }

    #[test]
    fn traversal_outside_the_indexed_roots_is_refused() {
        let e = read_doc("file://../../../../etc/passwd", 10, true, false).unwrap_err();
        assert!(e.contains("outside the indexed roots"), "{e}");
    }

    #[test]
    fn wrapping_respects_the_width_and_line_cap() {
        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi";
        let rows = search::wrap_lines(text, 3);
        assert!(rows.len() <= 3);
        for r in &rows {
            let line = r.strip_prefix("ROW line=").unwrap();
            assert!(
                line.chars().count() <= search::LINE_CHARS,
                "line too wide: {line:?}"
            );
        }
    }

    #[test]
    fn blank_lines_survive_as_paragraph_breaks() {
        let rows = search::wrap_lines("one\n\ntwo", 10);
        assert!(rows.iter().any(|r| r == "ROW line="), "paragraph break lost");
    }

    #[test]
    fn a_word_longer_than_the_width_does_not_loop_forever() {
        let rows = search::wrap_lines(&"x".repeat(300), 5);
        assert!(!rows.is_empty());
        assert!(rows.len() <= 5);
        for r in &rows {
            let line = r.strip_prefix("ROW line=").unwrap();
            assert!(
                line.chars().count() <= search::LINE_CHARS,
                "long word overshot: {line:?}"
            );
        }
    }
}
