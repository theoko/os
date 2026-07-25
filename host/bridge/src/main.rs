//! Host-side MCP-shaped connector bridge.
//!
//! Speaks the line protocol in docs/mcp-connectors-os-doc-v01.md over TCP.
//! Email backends: mock (default) or `gog`. Skills: defaults + saved on host.

mod graph;
mod workspace;
mod portals;
mod search;
mod transcribe;
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

    // Build the corpus index before serving. Lazily, whichever query arrives
    // first pays ~10s while every other one is instant — this was here before
    // the merge and the regression is invisible until you time a cold search.
    if tsearch::is_available() {
        let t0 = std::time::Instant::now();
        let n = tsearch::docs().len();
        let terms = tsearch::index().term_count();
        eprintln!(
            "tsearch: indexed {n} docs / {terms} terms in {:.1}s",
            t0.elapsed().as_secs_f64()
        );
    }

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
        // Both forms require skills=1; denied multiline still drains to END.
        if line.starts_with("CALL skills.save ") {
            let args = parse_args(line.trim_start_matches("CALL skills.save "));
            let name = arg_val(&args, "name").unwrap_or("").to_string();
            let has_skills = matches!(arg_val(&args, "skills"), Some("1"));
            let reply = if !has_skills {
                if arg_val(&args, "desc").is_none() {
                    // Drain LINE…END so the next command stays aligned.
                    loop {
                        if read_line_bounded(&mut reader, &mut raw)?.is_none() {
                            break;
                        }
                        let t = raw.trim_end_matches(['\r', '\n']);
                        if t == "END" {
                            break;
                        }
                    }
                }
                vec!["ERR skills.save needs_skills_cap".into()]
            } else if let Some(desc) = arg_val(&args, "desc") {
                let body = format!(
                    "---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n\n\
                     {desc}\n\n\
                     Suggested tools: `search.query`, `email.search`, `audio.transcribe`.\n\n\
                     (edit me)\n"
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
            vec!["OK tools=email.search,email.send,calendar.list,skills.list,skills.get,skills.save,search.query,workspace.index,tsearch.sync,teddy.health,teddy.fear_greed,teddy.gex,market.health,market.fear_greed,audio.transcribe,workspace.forget,audio.forget,portal.forget,email.forget,doc.read".into()]
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
        "email.search" if !matches!(arg_val(args, "email"), Some("1")) => {
            vec!["ERR email.search needs_email_cap".into()]
        }
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
        "skills.save" if !matches!(arg_val(args, "skills"), Some("1")) => {
            vec!["ERR skills.save needs_skills_cap".into()]
        }
        "skills.save" => {
            // Reached only via dispatch (tests); the socket path handles
            // skills.save in handle_client so it can read a LINE…END body.
            let name = arg_val(args, "name").unwrap_or("");
            let desc = arg_val(args, "desc").unwrap_or("User-saved skill.");
            let body = format!(
                "---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n\n\
                 {desc}\n\n\
                 Suggested tools: `search.query`, `email.search`, `audio.transcribe`.\n\n\
                 (edit me)\n"
            );
            match skills::save_skill(name, &body) {
                Ok(path) => vec![format!("OK skills.save path={}", path.display())],
                Err(e) => vec![format!("ERR skills.save {e}")],
            }
        }
        // Portal connectors leave the machine — same consent bit as tsearch.sync.
        tool if portals::is_portal_tool(tool) && !matches!(arg_val(args, "portal"), Some("1")) => {
            vec![format!("ERR {tool} needs_portal_cap")]
        }
        tool if portals::is_portal_tool(tool) => {
            let ep = portals::find(tool).expect("checked");
            match portals::fetch(ep, args) {
                Ok(body) => {
                    portals::save_snapshot(tool, &body);
                    let rows = portals::rows_for(tool, &body);
                    let mut out = vec![format!("OK {tool} n={}", rows.len())];
                    out.extend(rows);
                    out.push("END".into());
                    out
                }
                Err(e) => vec![format!("ERR {tool} {e}")],
            }
        }
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
                    let mut out = vec![format!(
                        "OK audio.transcribe words={words} seconds={secs:.0}"
                    )];
                    out.push(format!("ROW field=title|value={}", sanitize_field(&title)));
                    for line in summary {
                        out.push(format!("ROW field=summary|value={}", sanitize_field(&line)));
                    }
                    out.push("END".into());
                    out
                }
                Err(e) => vec![format!("ERR audio.transcribe {e}")],
            }
        }
        "tsearch.sync" if !matches!(arg_val(args, "portal"), Some("1")) => {
            vec!["ERR tsearch.sync needs_portal_cap".into()]
        }
        "tsearch.sync" => match tsearch::sync() {
            Ok((n, at)) => vec![
                format!("OK tsearch.sync n={n} crawled={at}"),
                "END".into(),
            ],
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
                    let mut out = vec![format!("OK doc.read n={}", lines.len())];
                    out.extend(lines);
                    out.push("END".into());
                    out
                }
                Err(e) => vec![format!("ERR doc.read {e}")],
            }
        }
        "workspace.forget" => match std::fs::remove_file(workspace::index_path()) {
            Ok(()) => vec!["OK workspace.forget removed".into(), "END".into()],
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                vec!["OK workspace.forget nothing_to_remove".into(), "END".into()]
            }
            Err(e) => vec![format!("ERR workspace.forget {e}")],
        },
        "audio.forget" => match std::fs::remove_file(transcribe::store_path()) {
            Ok(()) => vec!["OK audio.forget removed".into(), "END".into()],
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                vec!["OK audio.forget nothing_to_remove".into(), "END".into()]
            }
            Err(e) => vec![format!("ERR audio.forget {e}")],
        },
        // Revoking Email: purge the mail knowledge graph so "off" forgets.
        "email.forget" => match graph::forget() {
            Ok(status) => vec![format!("OK email.forget {status}"), "END".into()],
            Err(e) => vec![format!("ERR email.forget {e}")],
        },
        // Revoking Online services: purge teddy corpus API cache and live
        // portal snapshots so "off" means forgotten, not merely hidden.
        "portal.forget" => {
            let corpus = tsearch::forget();
            let snaps = portals::forget();
            match (corpus, snaps) {
                (Ok(c), Ok(s)) => {
                    let removed = c == "removed" || s == "removed";
                    vec![
                        format!(
                            "OK portal.forget {}",
                            if removed {
                                "removed"
                            } else {
                                "nothing_to_remove"
                            }
                        ),
                        "END".into(),
                    ]
                }
                (Err(e), _) | (_, Err(e)) => vec![format!("ERR portal.forget {e}")],
            }
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
            // Recordings have their own grant, so they get their own scope:
            // enabling workspace.index must not surface transcripts.
            let with_audio = matches!(arg_val(args, "audio"), Some("1"));
            let with_portal = matches!(arg_val(args, "portal"), Some("1"));
            if q.is_empty() {
                vec!["ERR search.query missing_q".into()]
            } else {
                search::query_scoped(q, k, cat, &backends.search, with_email, with_files, with_audio, with_portal)
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

/// Resolve a result URL back to readable text.
///
/// Scope is checked per source, using the same flags as `search.query`: a
/// caller that could not have found the document must not be able to read it
/// by guessing its URL.
fn read_doc(url: &str, max: usize, args: &[(String, String)]) -> Result<Vec<String>, String> {
    let with_files = matches!(arg_val(args, "files"), Some("1"));
    let with_audio = matches!(arg_val(args, "audio"), Some("1"));
    let with_email = matches!(arg_val(args, "email"), Some("1"));
    let with_portal = matches!(arg_val(args, "portal"), Some("1"));

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
    } else if let Some(id) = url.strip_prefix("email://") {
        // Mail graph stores sender/subject/snippet only — never the full body.
        if !with_email {
            return Err("needs_email_cap".into());
        }
        let g = graph::Graph::load_or_empty();
        let m = g
            .messages
            .iter()
            .find(|m| m.id == id)
            .ok_or("no such message")?;
        format!(
            "From: {}\nSubject: {}\n\n{}",
            m.from,
            m.subject,
            if m.snippet.is_empty() {
                "(no snippet)"
            } else {
                m.snippet.as_str()
            }
        )
    } else if let Some(rows) = search::body_for_builtin(url, max) {
        // Built-in os:// corpus — no portal bit.
        return Ok(rows);
    } else if search::portal_has_url(url) {
        // Teddy / remote corpus: same consent as search.query portal=1.
        if !with_portal {
            return Err("needs_portal_cap".into());
        }
        return search::body_for_portal(url, max).ok_or_else(|| "no readable body".into());
    } else {
        return Err("no readable body".into());
    };

    Ok(wrap_lines(&body, 78, max))
}

/// Hard-wrap text into `ROW line=...` entries the guest can render directly.
fn wrap_lines(text: &str, width: usize, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.lines() {
        if out.len() >= max {
            break;
        }
        let t = para.trim_end();
        if t.is_empty() {
            out.push("ROW line=".to_string());
            continue;
        }
        let mut cur = String::new();
        for word in t.split_whitespace() {
            if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > width {
                out.push(format!("ROW line={}", sanitize_field(&cur)));
                cur.clear();
                if out.len() >= max {
                    return out;
                }
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
        if !cur.is_empty() {
            out.push(format!("ROW line={}", sanitize_field(&cur)));
        }
    }
    out.truncate(max);
    out
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

    #[test]
    fn list_includes_teddy_api_and_teddy_portals() {
        // Corpus sync (teddy API) and live teddy.* tools must both be listed.
        let r = dispatch("LIST", &test_backends());
        assert!(r[0].contains("tsearch.sync"), "teddy API missing: {}", r[0]);
        assert!(r[0].contains("teddy.health"), "teddy portal missing: {}", r[0]);
        assert!(r[0].contains("teddy.fear_greed"), "{}", r[0]);
        assert!(r[0].contains("teddy.gex"), "{}", r[0]);
    }

    #[test]
    fn teddy_portals_need_the_portal_cap() {
        let denied = dispatch("CALL teddy.health", &test_backends());
        assert!(
            denied[0].contains("needs_portal_cap"),
            "ungated portal call: {denied:?}"
        );
        let denied_gex = dispatch("CALL teddy.gex", &test_backends());
        assert!(denied_gex[0].contains("needs_portal_cap"), "{denied_gex:?}");
        // Markets share the same consent bit.
        let denied_m = dispatch("CALL market.health", &test_backends());
        assert!(denied_m[0].contains("needs_portal_cap"), "{denied_m:?}");
    }

    #[test]
    fn tsearch_sync_still_needs_portal_cap() {
        let denied = dispatch("CALL tsearch.sync", &test_backends());
        assert!(denied[0].contains("needs_portal_cap"), "{denied:?}");
    }

    #[test]
    fn list_includes_portal_forget() {
        let r = dispatch("LIST", &test_backends());
        assert!(r[0].contains("portal.forget"), "{}", r[0]);
        assert!(r[0].contains("email.forget"), "{}", r[0]);
    }

    #[test]
    fn audio_transcribe_needs_the_audio_cap() {
        let denied = dispatch(
            "CALL audio.transcribe path=/tmp/x.wav",
            &test_backends(),
        );
        assert!(
            denied[0].contains("needs_audio_cap"),
            "ungated audio.transcribe: {denied:?}"
        );
    }

    #[test]
    fn email_search_needs_the_email_cap() {
        let denied = dispatch("CALL email.search q=in:inbox max=2", &test_backends());
        assert!(
            denied[0].contains("needs_email_cap"),
            "ungated email.search: {denied:?}"
        );
        // Allowed path ingests into the mail graph — keep it off the real home.
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-es-cap-{}", std::process::id()));
        let path = dir.join("emails.json");
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var("OS_GRAPH_PATH", &path);
        }
        let allowed = dispatch(
            "CALL email.search q=in:inbox max=2 email=1",
            &test_backends(),
        );
        assert!(
            allowed[0].starts_with("OK email.search"),
            "email=1 should search: {allowed:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("OS_GRAPH_PATH");
        }
    }

    #[test]
    fn email_forget_clears_the_graph() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-ef-{}", std::process::id()));
        let path = dir.join("emails.json");
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var("OS_GRAPH_PATH", &path);
        }
        let mut g = graph::Graph::default();
        g.ingest(&[(
            "a@x".into(),
            "secret".into(),
            "snippet".into(),
        )]);
        g.save().expect("save");
        assert!(path.exists());
        let r = dispatch("CALL email.forget", &test_backends());
        assert!(r[0].starts_with("OK email.forget"), "{r:?}");
        assert!(!path.exists(), "graph file should be gone");
        let again = dispatch("CALL email.forget", &test_backends());
        assert!(again[0].contains("nothing_to_remove"), "{again:?}");
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("OS_GRAPH_PATH");
        }
    }

    #[test]
    fn skills_save_needs_the_skills_cap() {
        let denied = dispatch(
            "CALL skills.save name=x desc=demo",
            &test_backends(),
        );
        assert!(
            denied[0].contains("needs_skills_cap"),
            "ungated skills.save: {denied:?}"
        );
        // With the bit, the request reaches validation rather than the cap
        // gate. Use an invalid name so this test never writes to a user's
        // saved-skill directory.
        let allowed = dispatch(
            "CALL skills.save name=not/valid desc=demo skills=1",
            &test_backends(),
        );
        assert!(
            allowed[0].contains("invalid_name"),
            "{allowed:?}"
        );
        assert!(!allowed[0].contains("needs_skills_cap"), "{allowed:?}");
    }

    #[test]
    fn skills_save_socket_path_denies_and_drains_body() {
        // The live wire path is handle_client, not dispatch — deny both forms
        // and keep reading after a rejected LINE…END body.
        let input = concat!(
            "CALL skills.save name=x desc=demo\n",
            "CALL skills.save name=y\n",
            "LINE must-not-persist\n",
            "END\n",
            "PING\n",
        );
        let mut out = Vec::new();
        handle_client(
            BufReader::new(input.as_bytes()),
            &mut out,
            &test_backends(),
        )
        .expect("handle_client");
        let lines: Vec<&str> = std::str::from_utf8(&out)
            .expect("utf8")
            .lines()
            .collect();
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(
            lines[0].contains("needs_skills_cap"),
            "one-line deny: {lines:?}"
        );
        assert!(
            lines[1].contains("needs_skills_cap"),
            "body deny: {lines:?}"
        );
        assert_eq!(lines[2], "OK pong", "desync after denied body: {lines:?}");
    }

    #[test]
    fn portal_forget_is_idempotent() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-pf-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let teddy = dir.join("teddy.json");
        let snaps = dir.join("portals.json");
        unsafe {
            std::env::set_var("OS_TSEARCH_CACHE", &teddy);
            std::env::set_var("OS_PORTAL_SNAPSHOT", &snaps);
        }
        tsearch::clear_memory();
        let r = dispatch("CALL portal.forget", &test_backends());
        assert!(r[0].starts_with("OK portal.forget"), "{r:?}");
        assert!(r[0].contains("nothing_to_remove"), "{r:?}");
        unsafe {
            std::env::remove_var("OS_TSEARCH_CACHE");
            std::env::remove_var("OS_PORTAL_SNAPSHOT");
        }
        tsearch::clear_memory();
        let _ = std::fs::remove_dir_all(&dir);
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
    fn reading_builtin_corpus_needs_no_wire_bit() {
        // Curated os:// docs are not personal data — open without portal/files.
        let rows = read_doc("os://AGENTS.md", 8, &args(&[])).expect("builtin");
        let text: String = rows
            .iter()
            .filter_map(|r| r.strip_prefix("ROW line="))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            text.contains("Capability-based agents") || text.contains("Agent-centric"),
            "{text}"
        );
    }

    #[test]
    fn reading_a_transcript_needs_the_audio_grant() {
        let e = read_doc("audio:///tmp/x.wav", 10, &args(&[("files", "1")])).unwrap_err();
        assert_eq!(e, "needs_audio_cap", "the files grant must not unlock recordings");
    }

    #[test]
    fn reading_a_transcript_with_audio_cap_shows_text() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-aread-{}", std::process::id()));
        let path = dir.join("t.json");
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var("OS_TRANSCRIPT_STORE", &path);
        }
        let mut store = transcribe::Store::default();
        store.upsert(transcribe::Transcript {
            source: "/tmp/os-unit-rec.wav".into(),
            title: "Unit Recording".into(),
            text: "unit recording transcript words for the reader".into(),
            seconds: 1.0,
            words: 7,
        });
        store.save().expect("save");
        let rows = read_doc(
            "audio:///tmp/os-unit-rec.wav",
            10,
            &args(&[("audio", "1")]),
        )
        .expect("read");
        let text: String = rows
            .iter()
            .filter_map(|r| r.strip_prefix("ROW line="))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("unit recording transcript"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("OS_TRANSCRIPT_STORE");
        }
    }

    #[test]
    fn reading_teddy_body_needs_the_portal_grant() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-pread-{}", std::process::id()));
        let path = dir.join("teddy.json");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            &path,
            r#"{"crawled_at":"now","docs":[{"t":"Portal Doc","u":"https://teddy.example/unit","c":"web","b":"portal body words","pr":0.5}]}"#,
        )
        .unwrap();
        unsafe {
            std::env::set_var("OS_TSEARCH_CACHE", &path);
        }
        tsearch::clear_memory();
        let denied = read_doc("https://teddy.example/unit", 10, &args(&[])).unwrap_err();
        assert_eq!(denied, "needs_portal_cap");
        let rows = read_doc(
            "https://teddy.example/unit",
            10,
            &args(&[("portal", "1")]),
        )
        .expect("portal=1");
        let text: String = rows
            .iter()
            .filter_map(|r| r.strip_prefix("ROW line="))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("portal body words"), "{text}");
        unsafe {
            std::env::remove_var("OS_TSEARCH_CACHE");
        }
        tsearch::clear_memory();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reading_mail_needs_the_email_grant() {
        let e = read_doc("email://deadbeef", 10, &args(&[("files", "1")])).unwrap_err();
        assert_eq!(e, "needs_email_cap", "files must not unlock mail peeks");
    }

    #[test]
    fn reading_mail_with_email_cap_shows_snippet() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-eread-{}", std::process::id()));
        let path = dir.join("emails.json");
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var("OS_GRAPH_PATH", &path);
        }
        let mut g = graph::Graph::default();
        g.ingest(&[(
            "ada@x.com".into(),
            "Open me".into(),
            "snippet for the reader".into(),
        )]);
        g.save().expect("save");
        let id = g.messages[0].id.clone();
        let rows = read_doc(
            &format!("email://{id}"),
            10,
            &args(&[("email", "1")]),
        )
        .expect("read");
        let text: String = rows
            .iter()
            .filter_map(|r| r.strip_prefix("ROW line="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("ada@x.com"), "{text}");
        assert!(text.contains("Open me"), "{text}");
        assert!(text.contains("snippet for the reader"), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("OS_GRAPH_PATH");
        }
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
        let rows = wrap_lines(text, 20, 3);
        assert!(rows.len() <= 3);
        for r in &rows {
            let line = r.strip_prefix("ROW line=").unwrap();
            assert!(line.chars().count() <= 20, "line too wide: {line:?}");
        }
    }

    #[test]
    fn blank_lines_survive_as_paragraph_breaks() {
        let rows = wrap_lines("one\n\ntwo", 40, 10);
        assert!(rows.iter().any(|r| r == "ROW line="), "paragraph break lost");
    }

    #[test]
    fn a_word_longer_than_the_width_does_not_loop_forever() {
        let rows = wrap_lines(&"x".repeat(300), 20, 5);
        assert!(!rows.is_empty());
        assert!(rows.len() <= 5);
    }
}
