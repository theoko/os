//! Host-side MCP-shaped connector bridge.
//!
//! Speaks the line protocol in docs/mcp-connectors-os-doc-v01.md over TCP.
//! Email backends: automatic Gmail discovery, `gog`, or the explicit demo
//! `mock` backend. Skills: defaults + saved on host.

mod agent;
mod config;
mod graph;
mod intent;
mod portals;
mod search;
mod skills;
mod transcribe;
mod tsearch;
mod workspace;

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
        email: email_backend(),
        search: env::var("OS_MCP_SEARCH_BACKEND").unwrap_or_else(|_| "tfidf".into()),
    });

    let (defaults, user) = skills::skills_dirs();
    let where_ = connect.as_deref().unwrap_or(addr.as_str());
    eprintln!(
        "os-mcp-bridge {} on {where_} (email={}; search={}; skills defaults={} user={})",
        // "starting", not "listening": this prints *before* the bind, and
        // serve_tcp/serve_unix log "bound …" once the socket is really up.
        // Claiming to listen early is what made `ensure-bridge` report a
        // bridge that nothing could connect to yet.
        if connect.is_some() {
            "connecting"
        } else {
            "starting"
        },
        backends.email,
        backends.search,
        defaults.display(),
        user.display()
    );

    // Nothing is indexed here on purpose. Prewarming *before* the socket
    // exists kept the port closed for ~10s: `make utm-bridged` started QEMU
    // against a socket nobody was listening on yet and QEMU aborted with
    // "Connection refused" — a boot failure caused entirely by indexing order.
    // Listen modes bind first and warm off the accept path (`serve_tcp` /
    // `serve_unix`); connect mode has no port to protect and warms inline.
    if let Some(target) = connect {
        // Connect mode has no listen port; warm before dialing so the first
        // guest session does not pay the cold-index cost.
        warm_tsearch();
        connect_loop(&target, backends);
    } else if let Some(path) = addr.strip_prefix("unix:") {
        serve_unix(path, backends);
    } else {
        serve_tcp(&addr, backends);
    }
}

/// Build the corpus index. Must run *after* bind in listen modes so UTM/QEMU
/// can connect while a cold index is still building (SYN sits in the backlog),
/// and is spawned there so `accept` is not blocked either. Indexing before
/// bind made `ensure-bridge` report "started" for seconds while nothing
/// accepted on :7420 → Connection refused.
fn warm_tsearch() {
    if !tsearch::is_available() {
        return;
    }
    let t0 = std::time::Instant::now();
    let n = tsearch::docs().len();
    let terms = tsearch::index().term_count();
    eprintln!(
        "tsearch: indexed {n} docs / {terms} terms in {:.1}s",
        t0.elapsed().as_secs_f64()
    );
}

/// Pick a real inbox when the host has already authorized one.  The OS must
/// never pass fabricated demo mail off as a user's inbox: with no account, it
/// returns the explicit `unconfigured` state instead. `mock` remains useful
/// only for demos and tests when requested deliberately.
fn email_backend() -> String {
    match env::var("OS_MCP_EMAIL_BACKEND")
        .as_deref()
        .unwrap_or("auto")
    {
        "auto" => if gog_has_account() {
            "gog"
        } else {
            "unconfigured"
        }
        .into(),
        "gog" | "mock" => env::var("OS_MCP_EMAIL_BACKEND").unwrap(),
        _ => "unconfigured".into(),
    }
}

fn gog_has_account() -> bool {
    let Ok(out) = Command::new("gog")
        .args(["auth", "list", "--json", "--no-input"])
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    gog_accounts_from_json(&String::from_utf8_lossy(&out.stdout))
}

fn gog_accounts_from_json(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("accounts").and_then(|a| a.as_array()).cloned())
        .is_some_and(|accounts| !accounts.is_empty())
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
    eprintln!("bound tcp {addr}");
    // Bound *and* accepting before the corpus is warm: a guest that connects
    // during a cold index waits on its first query, not on the connection.
    std::thread::spawn(warm_tsearch);
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
    eprintln!("bound unix {}", path.display());
    std::thread::spawn(warm_tsearch);
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
    // Operator unlock is per connection and lives here: never a global, never
    // written down. A new connection starts locked however many others are
    // unlocked, and closing the socket is what re-locks it.
    let mut session = config::Session::new();
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
        eprintln!("← {}", redact_line(&line));

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

        let reply = dispatch_session(&line, backends, &mut session);
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

/// The log form of a request line: a secret must never reach `.bridge.log`.
///
/// Redaction is by *key*, not by tool name, so a mistyped tool
/// (`CALL config.unlok pass=…`) still gets scrubbed. Everything from `pass=` to
/// end of line goes, because `parse_args` lets a value contain spaces — a
/// password with a space in it would otherwise survive as trailing tokens.
fn redact_line(line: &str) -> String {
    match line.find("pass=") {
        Some(i) => format!("{}pass=***", &line[..i]),
        None => line.to_string(),
    }
}

/// Dispatch with no operator session — a fresh, locked one per call.
///
/// Live connections use `dispatch_session` so unlock state survives between
/// their requests; this is the entry point for callers that have no connection
/// (tests), and being locked is the correct default for them.
#[cfg(test)]
fn dispatch(line: &str, backends: &Backends) -> Vec<String> {
    dispatch_session(line, backends, &mut config::Session::new())
}

fn dispatch_session(
    line: &str,
    backends: &Backends,
    session: &mut config::Session,
) -> Vec<String> {
    let (cmd, rest) = split_word(line);
    match cmd {
        "PING" => vec!["OK pong".into()],
        "LIST" => {
            // Union of both lines: the planner pair (agent.act + intent.resolve)
            // and the portal pair (portal.status + portal.forget) are all
            // dispatchable, so all of them are advertised. `agent.plan` is not
            // listed — the implemented tool is `agent.act`.
            // Portal tools stay listed whichever family is active: the list is
            // what the bridge implements, and a tool that is off answers with
            // its own reason rather than vanishing.
            vec!["OK tools=email.search,email.send,calendar.list,skills.list,skills.get,skills.save,skills.forget,search.query,agent.act,intent.resolve,workspace.index,workspace.recent,tsearch.sync,teddy.health,teddy.fear_greed,teddy.gex,market.health,market.fear_greed,audio.transcribe,workspace.forget,audio.forget,portal.forget,portal.status,email.forget,doc.read,config.status,config.unlock,config.portal".into()]
        }
        "CALL" => {
            let (tool, rest) = split_word(rest);
            let args = parse_args(rest);
            call_tool(tool, &args, backends, session)
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
                && k.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
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

fn call_tool(
    tool: &str,
    args: &[(String, String)],
    backends: &Backends,
    session: &mut config::Session,
) -> Vec<String> {
    match tool {
        // The old agent.plan returned prose. agent.act returns prose *plus*
        // steps you can click, because a plan you cannot act on is a note.
        "agent.act" => {
            let goal = arg_val(args, "goal").unwrap_or("");
            if goal.trim().is_empty() {
                return vec!["ERR agent.act missing_goal".into()];
            }
            let grants = agent::Grants {
                files: matches!(arg_val(args, "files"), Some("1")),
                email: matches!(arg_val(args, "email"), Some("1")),
                audio: matches!(arg_val(args, "audio"), Some("1")),
                portal: matches!(arg_val(args, "portal"), Some("1")),
            };
            // How many rows the guest's screen holds. Returning more made the
            // agent's own sentence ("5 matches") contradict the three rows
            // actually rendered. `max` is accepted as an older spelling.
            let k: usize = arg_val(args, "k")
                .or_else(|| arg_val(args, "max"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(5)
                .clamp(1, 20);
            let a = agent::act(goal, grants, k);

            // agent.act must never be worse than the search box it replaces.
            // The planner only looks at your own sources, so a plain keyword
            // query ("q2 planning") or a goal it could not place still gets
            // the full scoped index behind it.
            let mut rows: Vec<(String, String, String)> = a
                .steps
                .iter()
                .map(|s| (s.label.clone(), s.url.clone(), s.why.clone()))
                .collect();
            let fell_back = rows.is_empty() || a.intent == agent::Intent::Unknown;
            if fell_back {
                let query = if a.subject.is_empty() {
                    goal
                } else {
                    &a.subject
                };
                for line in search::query_scoped(
                    query,
                    k,
                    None,
                    &backends.search,
                    grants.email,
                    grants.files,
                    grants.audio,
                    grants.portal,
                ) {
                    let (Some(t), Some(u)) = (
                        parse_row_field(&line, "title"),
                        parse_row_field(&line, "url"),
                    ) else {
                        continue;
                    };
                    if rows.iter().any(|(_, have, _)| have == u) {
                        continue;
                    }
                    rows.push((t.to_string(), u.to_string(), "matched your query".into()));
                }
            }
            rows.truncate(k);

            // Say the true thing about the rows actually being shown, not the
            // one the planner wrote before the fallback filled them in.
            let say = if a.steps.is_empty() && !rows.is_empty() {
                format!(
                    "{} matches for {}.",
                    rows.len(),
                    if a.subject.is_empty() {
                        goal
                    } else {
                        &a.subject
                    }
                )
            } else {
                a.say
            };

            let mut out = vec![format!(
                "OK agent.act n={} intent={}",
                rows.len(),
                a.intent.name()
            )];
            out.push(format!("SAY {say}"));
            for (title, url, why) in rows {
                out.push(format!("ROW title={title}|url={url}|why={why}"));
            }
            out.push("END".into());
            out
        }
        "email.search" if !matches!(arg_val(args, "email"), Some("1")) => {
            vec!["ERR email.search needs_email_cap".into()]
        }
        "email.search" => email_search(args, &backends.email),
        "email.send" if !matches!(arg_val(args, "email"), Some("1")) => {
            vec!["ERR email.send needs_email_cap".into()]
        }
        "email.send" if !matches!(arg_val(args, "confirm"), Some("1")) => {
            // Grant alone is not enough — guest must send confirm=1 after an
            // explicit UI confirm (AGENTS non-negotiable #3).
            vec!["ERR email.send disabled_until_cap_confirm".into()]
        }
        "email.send" => {
            let to = arg_val(args, "to").unwrap_or("");
            let subj = arg_val(args, "subj").unwrap_or("(no subject)");
            if to.is_empty() {
                vec!["ERR email.send missing_to".into()]
            } else {
                // Mock only — never talks to gog. CI proves the confirm path.
                vec![format!(
                    "OK email.send mock queued to={} subj={}",
                    sanitize_field(to),
                    sanitize_field(subj)
                )]
            }
        }
        // Same Google identity as email.search — no ambient calendar without
        // the email wire bit (reuse Cap::EmailSearch on the guest).
        "calendar.list" if !matches!(arg_val(args, "email"), Some("1")) => {
            vec!["ERR calendar.list needs_email_cap".into()]
        }
        "calendar.list" => calendar_list_mock(),
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
            // Same starter body the socket path writes in `handle_client`, so
            // a skill saved through either route looks the same on disk.
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
        // Operator configuration. `config.status` answers while locked — it
        // names no secret and the guest needs it to draw the panel.
        "config.status" => config::status(session),
        "config.unlock" => config::unlock(session, arg_val(args, "pass").unwrap_or("")),
        "config.portal" => config::set_portal(session, arg_val(args, "family")),
        // Portal connectors leave the machine — same consent bit as
        // tsearch.sync. The guest's consent bit is checked before the
        // operator's family choice so that a tool the guest never asked for
        // reports the missing grant it always did; sending `portal=1` is what
        // then surfaces the family policy.
        tool if portals::find_any(tool).is_some()
            && !matches!(arg_val(args, "portal"), Some("1")) =>
        {
            vec![format!("ERR {tool} needs_portal_cap")]
        }
        // A real portal tool from the family this machine is not configured
        // for. Distinct from `not_found`: the tool exists, the operator has it
        // switched off, and no request leaves the host.
        tool if portals::find_any(tool).is_some() && !portals::is_portal_tool(tool) => {
            vec![format!("ERR {tool} portal_family_disabled")]
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
        "tsearch.sync" => {
            // Returns at once; the guest polls portal.status rather than
            // holding COM2 open for a minute.
            let state = tsearch::sync_background();
            vec![format!("OK tsearch.sync {state}"), "END".into()]
        }
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
        // Local only: reports what is cached, never fetches. The status dot
        // must not become a reason to hit the network.
        "portal.status" => {
            let n = tsearch::docs().len();
            let cached = tsearch::is_available();
            vec![
                format!(
                    "OK portal.status n={n} cached={} syncing={}",
                    if cached { 1 } else { 0 },
                    if tsearch::is_syncing() { 1 } else { 0 }
                ),
                "END".into(),
            ]
        }
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
        "skills.forget" => match skills::forget() {
            Ok(status) => vec![format!("OK skills.forget {status}"), "END".into()],
            Err(e) => vec![format!("ERR skills.forget {e}")],
        },
        // Revoking Online services must remove the fetched corpus and the live
        // portal snapshots, not merely stop consulting them — the other grants
        // already work that way, and 64MB of someone else's crawl sitting on
        // disk after they said no is exactly the gap "off means gone" closes.
        // `tsearch::forget` also drops the parsed copy: deleting the file alone
        // leaves the corpus resident in memory and still searchable.
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
        "workspace.recent" if !matches!(arg_val(args, "files"), Some("1")) => {
            vec!["ERR workspace.recent needs_workspace_cap".into()]
        }
        "workspace.recent" => {
            let k: usize = arg_val(args, "k")
                .and_then(|s| s.parse().ok())
                .unwrap_or(3)
                .clamp(1, 10);
            workspace_recent(k)
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
                search::query_scoped(
                    q,
                    k,
                    cat,
                    &backends.search,
                    with_email,
                    with_files,
                    with_audio,
                    with_portal,
                )
            }
        }
        // Natural-language planner. File hits require files=1; mail act notes
        // when email=1 is missing. Planning itself is not a personal-data leak.
        "intent.resolve" => {
            let q = arg_val(args, "q").unwrap_or("").trim();
            if q.is_empty() {
                vec!["ERR intent.resolve missing_q".into()]
            } else {
                let with_files = matches!(arg_val(args, "files"), Some("1"));
                let with_email = matches!(arg_val(args, "email"), Some("1"));
                let plan = intent::resolve(q, with_files, with_email);
                intent::wire_response(&plan)
            }
        }
        _ => vec![format!("ERR {tool} not_found")],
    }
}

/// Top-ranked workspace files for the guest Home "Recent files" strip.
fn workspace_recent(k: usize) -> Vec<String> {
    let mut entries = workspace::Index::load().entries;
    entries.sort_by(|a, b| {
        b.pr
            .partial_cmp(&a.pr)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let n = entries.len().min(k);
    let mut out = vec![format!("OK workspace.recent n={n}")];
    for e in entries.into_iter().take(n) {
        out.push(format!(
            "ROW title={}|url=file://{}|snip={}",
            sanitize_field(&e.title),
            sanitize_field(&e.path),
            sanitize_field(&e.snippet)
        ));
    }
    out.push("END".into());
    out
}

/// Mock calendar rows — deterministic ids via [`graph::id_for`].
fn calendar_mock_events() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        (
            "Demo event",
            "tomorrow",
            "Planning sync. Bring the Q2 notes.",
        ),
        (
            "Focus block",
            "Friday 14:00",
            "No meetings. Deep work on the guest skill runner.",
        ),
    ]
}

fn calendar_list_mock() -> Vec<String> {
    let events = calendar_mock_events();
    let mut out = vec![format!("OK calendar.list n={}", events.len())];
    for (title, when, _) in events {
        let id = graph::id_for(title, when);
        out.push(format!(
            "ROW id={id}|title={}|when={}",
            sanitize_field(title),
            sanitize_field(when)
        ));
    }
    out.push("END".into());
    out
}

fn calendar_body(id: &str) -> Option<String> {
    for (title, when, notes) in calendar_mock_events() {
        if graph::id_for(title, when) == id {
            return Some(format!("Event: {title}\nWhen: {when}\n\n{notes}"));
        }
    }
    None
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
        // Mail graph stores sender/subject/snippet only — never the full body,
        // and it is readable only with the grant that surfaced it.
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
    } else if let Some(id) = url.strip_prefix("cal://") {
        // Same Google identity / email=1 consent as calendar.list.
        if !with_email {
            return Err("needs_email_cap".into());
        }
        calendar_body(id).ok_or("no such event")?
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
    args.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn email_search(args: &[(String, String)], backend: &str) -> Vec<String> {
    let query = arg_val(args, "q").unwrap_or("in:inbox");
    let max: usize = arg_val(args, "max")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
        .clamp(1, 20);

    let out = match backend {
        "gog" => email_search_gog(query, max),
        "unconfigured" => vec!["ERR email.search email_not_connected".into()],
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
        // Carry the graph id so the guest can open the message, not just list
        // it. Same hash the graph uses, so the two agree. Both spellings ship:
        // `id=` for callers that build their own URL, `url=` for callers that
        // hand it straight to doc.read.
        let id = graph::id_for(from, subj);
        out.push(format!("ROW id={id}|from={from}|subj={subj}|url=email://{id}"));
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
            // Hash the sanitized pair, so the id matches what the graph stores.
            let (from, subj) = (sanitize_field(from), sanitize_field(subj));
            let id = graph::id_for(&from, &subj);
            rows.push(format!("ROW id={id}|from={from}|subj={subj}|url=email://{id}"));
        }
    }

    if rows.is_empty() {
        for line in stdout.lines().take(max) {
            let line = sanitize_field(line);
            if !line.is_empty() {
                let id = graph::id_for("gog", &line);
                rows.push(format!("ROW id={id}|from=gog|subj={line}|url=email://{id}"));
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

    #[test]
    fn auto_email_detects_only_a_real_saved_account() {
        assert!(!gog_accounts_from_json(r#"{"accounts":[]}"#));
        assert!(gog_accounts_from_json(
            r#"{"accounts":[{"email":"me@example.com"}]}"#
        ));
        assert!(!gog_accounts_from_json("not json"));
    }

    fn test_backends() -> Backends {
        Backends {
            email: "mock".into(),
            search: "tfidf".into(),
        }
    }

    #[test]
    fn dispatch_ping() {
        assert_eq!(
            dispatch("PING", &test_backends()),
            vec!["OK pong".to_string()]
        );
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
        assert!(r[0].contains("intent.resolve"), "{r:?}");
    }

    #[test]
    fn intent_resolve_plans_a_paper_ask() {
        let r = dispatch(
            "CALL intent.resolve q=i wanna work on my paper",
            &test_backends(),
        );
        assert!(
            r[0].starts_with("OK intent.resolve act=open"),
            "{r:?}"
        );
        assert!(r[0].contains("query="), "{r:?}");
        assert!(r.iter().any(|l| l.starts_with("ROW plan=")), "{r:?}");
        assert_eq!(r.last().map(String::as_str), Some("END"));
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
        assert!(r[0].contains("skills.forget"), "{}", r[0]);
    }

    #[test]
    fn skills_forget_clears_saved_skills() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-sforget-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("OS_SKILLS_USER", &dir);
        }
        let saved = dispatch(
            "CALL skills.save name=dispatch-saved desc=demo skills=1",
            &test_backends(),
        );
        assert!(saved[0].starts_with("OK skills.save"), "{saved:?}");
        let listed = dispatch("CALL skills.list", &test_backends());
        assert!(listed.iter().any(|l| l.contains("dispatch-saved")), "{listed:?}");
        let r = dispatch("CALL skills.forget", &test_backends());
        assert!(r[0].starts_with("OK skills.forget removed"), "{r:?}");
        let after = dispatch("CALL skills.list", &test_backends());
        assert!(
            !after.iter().any(|l| l.contains("dispatch-saved")),
            "saved skill survived forget: {after:?}"
        );
        let again = dispatch("CALL skills.forget", &test_backends());
        assert!(again[0].starts_with("OK skills.forget"), "{again:?}");
        unsafe {
            std::env::remove_var("OS_SKILLS_USER");
        }
        let _ = std::fs::remove_dir_all(&dir);
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
    fn email_send_needs_email_and_confirm() {
        let no_email = dispatch(
            "CALL email.send to=ada@x.com subj=Hi body=Hello",
            &test_backends(),
        );
        assert!(
            no_email[0].contains("needs_email_cap"),
            "ungated email.send: {no_email:?}"
        );
        let no_confirm = dispatch(
            "CALL email.send to=ada@x.com subj=Hi body=Hello email=1",
            &test_backends(),
        );
        assert!(
            no_confirm[0].contains("disabled_until_cap_confirm"),
            "confirm-less email.send: {no_confirm:?}"
        );
        let ok = dispatch(
            "CALL email.send to=ada@x.com subj=Hi body=Hello email=1 confirm=1",
            &test_backends(),
        );
        assert!(
            ok[0].starts_with("OK email.send mock"),
            "confirmed mock send: {ok:?}"
        );
        assert!(ok[0].contains("ada@x.com"), "{ok:?}");
    }

    #[test]
    fn workspace_recent_needs_the_files_cap() {
        let denied = dispatch("CALL workspace.recent", &test_backends());
        assert!(
            denied[0].contains("needs_workspace_cap"),
            "ungated workspace.recent: {denied:?}"
        );
    }

    #[test]
    fn calendar_list_needs_the_email_cap() {
        let denied = dispatch("CALL calendar.list", &test_backends());
        assert!(
            denied[0].contains("needs_email_cap"),
            "ungated calendar.list: {denied:?}"
        );
        let allowed = dispatch("CALL calendar.list email=1", &test_backends());
        assert!(
            allowed[0].starts_with("OK calendar.list"),
            "email=1 should list: {allowed:?}"
        );
        let row = allowed
            .iter()
            .find(|l| l.starts_with("ROW "))
            .expect("ROW");
        let id = parse_row_field(row, "id").expect("id=");
        let title = parse_row_field(row, "title").expect("title=");
        let when = parse_row_field(row, "when").expect("when=");
        assert_eq!(id, graph::id_for(title, when));
    }

    #[test]
    fn email_search_rows_carry_graph_ids() {
        let rows = email_search_mock("in:inbox", 2);
        let row = rows.iter().find(|l| l.starts_with("ROW ")).expect("ROW");
        let id = parse_row_field(row, "id").expect("id=");
        let from = parse_row_field(row, "from").expect("from=");
        let subj = parse_row_field(row, "subj").expect("subj=");
        assert_eq!(id, graph::id_for(from, subj), "ROW id must match graph");
        assert_eq!(id.len(), 16);
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

    /// Everything `LIST` advertises must reach a real arm of `call_tool`.
    /// A tool listed but not dispatchable is a guest button that always fails.
    #[test]
    fn every_listed_tool_is_dispatchable() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The list includes the *.forget tools, which delete real host state.
        // Point every one of them at a scratch dir before calling anything.
        let dir = std::env::temp_dir().join(format!("os-listable-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        unsafe {
            std::env::set_var("OS_TSEARCH_CACHE", dir.join("teddy.json"));
            std::env::set_var("OS_PORTAL_SNAPSHOT", dir.join("portals.json"));
            std::env::set_var("OS_GRAPH_PATH", dir.join("emails.json"));
            std::env::set_var("OS_WORKSPACE_INDEX", dir.join("workspace.json"));
            std::env::set_var("OS_TRANSCRIPT_STORE", dir.join("transcripts.json"));
            std::env::set_var("OS_SKILLS_USER", dir.join("skills"));
            std::env::set_var("OS_CONFIG_PATH", dir.join("config.json"));
            std::env::set_var("OS_CONFIG_PASSWORD", "");
        }
        tsearch::clear_memory();

        let listing = dispatch("LIST", &test_backends());
        let tools: Vec<String> = listing[0]
            .trim_start_matches("OK tools=")
            .split(',')
            .map(String::from)
            .collect();
        assert!(tools.len() > 20, "{listing:?}");
        for tool in &tools {
            // Args only where a missing one would produce the tool's *own*
            // not_found and hide the real answer; no live portal call is made
            // because none of these carry portal=1.
            let call = match tool.as_str() {
                "skills.get" => "CALL skills.get name=email-triage".to_string(),
                t => format!("CALL {t}"),
            };
            let reply = dispatch(&call, &test_backends());
            assert!(!reply.is_empty(), "{tool} answered nothing");
            assert_ne!(
                reply[0],
                format!("ERR {tool} not_found"),
                "{tool} is listed but not dispatchable"
            );
        }
        assert!(tools.iter().any(|t| t == "config.status"), "{listing:?}");
        assert!(tools.iter().any(|t| t == "config.unlock"), "{listing:?}");
        assert!(tools.iter().any(|t| t == "config.portal"), "{listing:?}");

        unsafe {
            for k in [
                "OS_TSEARCH_CACHE",
                "OS_PORTAL_SNAPSHOT",
                "OS_GRAPH_PATH",
                "OS_WORKSPACE_INDEX",
                "OS_TRANSCRIPT_STORE",
                "OS_SKILLS_USER",
                "OS_CONFIG_PATH",
                "OS_CONFIG_PASSWORD",
            ] {
                std::env::remove_var(k);
            }
        }
        tsearch::clear_memory();
        let _ = std::fs::remove_dir_all(&dir);
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

/// Operator config over the wire: unlock is per connection, the family choice
/// is host state, and neither the log nor a disabled family leaks.
#[cfg(test)]
mod config_wire_tests {
    use super::*;

    const PASS: &str = "correct horse battery staple";

    fn backends() -> Backends {
        Backends { email: "mock".into(), search: "tfidf".into() }
    }

    /// Point config at a scratch dir and set the admin password from the
    /// environment, so no test reads the real Keychain or the real
    /// Application Support config. Caller holds `graph::ENV_LOCK`.
    fn scratch(tag: &str, pass: Option<&str>) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("os-cfgwire-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        unsafe {
            std::env::set_var("OS_CONFIG_PATH", dir.join("config.json"));
            std::env::set_var("OS_CONFIG_PASSWORD", pass.unwrap_or(""));
        }
        dir
    }

    fn cleanup(dir: std::path::PathBuf) {
        unsafe {
            std::env::remove_var("OS_CONFIG_PATH");
            std::env::remove_var("OS_CONFIG_PASSWORD");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Run one whole connection over `input`, as the socket path would.
    fn connection(input: &str) -> Vec<String> {
        let mut out = Vec::new();
        handle_client(BufReader::new(input.as_bytes()), &mut out, &backends())
            .expect("handle_client");
        String::from_utf8(out)
            .expect("utf8")
            .lines()
            .map(String::from)
            .collect()
    }

    #[test]
    fn status_is_answered_while_locked_and_names_no_secret() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("status", Some(PASS));
        let r = dispatch("CALL config.status", &backends());
        assert_eq!(r.len(), 1, "single line, no END: {r:?}");
        assert_eq!(r[0], "OK config.status portal=none locked=1 configured=1");
        assert!(!r[0].contains(PASS));
        // No password on the host: configured=0. `locked` still describes this
        // connection only — an unconfigured host is one nobody can unlock, so
        // it reads as locked and `config.portal` stays refused.
        unsafe { std::env::set_var("OS_CONFIG_PASSWORD", "") };
        let r = dispatch("CALL config.status", &backends());
        assert_eq!(r[0], "OK config.status portal=none locked=1 configured=0", "{r:?}");
        cleanup(dir);
    }

    #[test]
    fn unlock_accepts_the_secret_and_rejects_everything_else() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("unlock", Some(PASS));

        let mut s = config::Session::new();
        assert_eq!(
            call_tool("config.unlock", &args(&[("pass", "wrong")]), &backends(), &mut s),
            vec!["ERR config.unlock bad_pass".to_string()]
        );
        assert!(s.locked(), "a wrong guess must not unlock");
        // A prefix of the real secret is not the real secret.
        assert_eq!(
            call_tool("config.unlock", &args(&[("pass", "correct horse")]), &backends(), &mut s),
            vec!["ERR config.unlock bad_pass".to_string()]
        );
        assert_eq!(
            call_tool("config.unlock", &args(&[("pass", PASS)]), &backends(), &mut s),
            vec!["OK config.unlock".to_string()]
        );
        assert!(!s.locked());

        // Status now reports this connection as unlocked.
        let st = call_tool("config.status", &[], &backends(), &mut s);
        assert!(st[0].contains("locked=0"), "{st:?}");
        cleanup(dir);
    }

    #[test]
    fn unlock_on_a_host_with_no_password_says_so() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // No password at all — and OS_CONFIG_PASSWORD="" keeps this off the
        // real Keychain rather than merely off a real password.
        let dir = scratch("nopass", None);
        let r = dispatch("CALL config.unlock pass=anything", &backends());
        assert_eq!(r, vec!["ERR config.unlock not_configured".to_string()]);
        // And an unconfigured host still cannot be configured by a guest.
        let r = dispatch("CALL config.portal family=teddy", &backends());
        assert_eq!(r, vec!["ERR config.portal locked".to_string()]);
        cleanup(dir);
    }

    #[test]
    fn repeated_failures_cut_the_connection_off_entirely() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("toomany", Some(PASS));
        let mut s = config::Session::new();
        for i in 1..=config::MAX_UNLOCK_ATTEMPTS {
            let r = call_tool("config.unlock", &args(&[("pass", "nope")]), &backends(), &mut s);
            assert_eq!(r, vec!["ERR config.unlock bad_pass".to_string()], "attempt {i}");
        }
        let capped = call_tool("config.unlock", &args(&[("pass", "nope")]), &backends(), &mut s);
        assert_eq!(capped, vec!["ERR config.unlock too_many".to_string()]);
        // Past the cap even the right password is refused: grinding for it and
        // then using it on the same connection must not work.
        let right = call_tool("config.unlock", &args(&[("pass", PASS)]), &backends(), &mut s);
        assert_eq!(right, vec!["ERR config.unlock too_many".to_string()]);
        assert!(s.locked());
        cleanup(dir);
    }

    #[test]
    fn a_second_connection_does_not_inherit_the_first_unlock() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("perconn", Some(PASS));

        let first = connection(&format!(
            "CALL config.unlock pass={PASS}\nCALL config.portal family=market\n"
        ));
        assert_eq!(first[0], "OK config.unlock", "{first:?}");
        assert_eq!(first[1], "OK config.portal family=market", "{first:?}");

        // New connection: locked again, though the family it set persists.
        let second = connection("CALL config.status\nCALL config.portal family=teddy\n");
        assert_eq!(second[0], "OK config.status portal=market locked=1 configured=1", "{second:?}");
        assert_eq!(second[1], "ERR config.portal locked", "{second:?}");
        assert_eq!(config::family(), config::Family::Market, "guest must not have changed it");
        cleanup(dir);
    }

    #[test]
    fn the_family_choice_survives_a_reload() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("persist", Some(PASS));

        let locked = dispatch("CALL config.portal family=teddy", &backends());
        assert_eq!(locked, vec!["ERR config.portal locked".to_string()]);
        assert_eq!(config::family(), config::Family::None, "refused write must not land");

        let mut s = config::Session::new();
        call_tool("config.unlock", &args(&[("pass", PASS)]), &backends(), &mut s);
        let ok = call_tool("config.portal", &args(&[("family", "teddy")]), &backends(), &mut s);
        assert_eq!(ok, vec!["OK config.portal family=teddy".to_string()]);

        // "Reload" = read it back through a path that holds no state.
        assert_eq!(config::family(), config::Family::Teddy);
        let raw = std::fs::read_to_string(config::config_path()).expect("config written");
        assert!(raw.contains("teddy"), "{raw}");
        assert!(!raw.contains(PASS), "the password must never be persisted: {raw}");
        let fresh = connection("CALL config.status\n");
        assert_eq!(fresh[0], "OK config.status portal=teddy locked=1 configured=1");

        // Unknown spellings are rejected rather than silently becoming none.
        for bad in ["", "family=teddysearch", "family=TEDDY", "family=all"] {
            let r = call_tool(
                "config.portal",
                &parse_args(bad),
                &backends(),
                &mut s,
            );
            assert_eq!(r, vec!["ERR config.portal unknown_family".to_string()], "{bad:?}");
        }
        assert_eq!(config::family(), config::Family::Teddy, "a bad value must not clear it");
        cleanup(dir);
    }

    #[test]
    fn a_disabled_family_cannot_be_called_by_the_guest() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("enforce", Some(PASS));

        config::set_family(config::Family::Teddy).expect("write");
        // portal=1 is present: this is the operator's policy refusing, not the
        // guest's missing consent bit, and no request leaves the host.
        for tool in ["market.health", "market.fear_greed"] {
            let r = dispatch(&format!("CALL {tool} portal=1"), &backends());
            assert_eq!(r, vec![format!("ERR {tool} portal_family_disabled")], "{tool}");
            // Distinct from an unknown tool — the guest must be able to tell
            // "switched off here" from "no such tool".
            assert!(!r[0].contains("not_found"), "{r:?}");
        }
        assert!(portals::is_portal_tool("teddy.gex"), "the chosen family stays live");

        config::set_family(config::Family::None).expect("write");
        for tool in ["market.health", "teddy.health", "teddy.gex"] {
            let r = dispatch(&format!("CALL {tool} portal=1"), &backends());
            assert_eq!(r, vec![format!("ERR {tool} portal_family_disabled")], "{tool}");
        }
        // An actually unknown tool still reports not_found.
        let unknown = dispatch("CALL market.nope portal=1", &backends());
        assert_eq!(unknown, vec!["ERR market.nope not_found".to_string()]);
        cleanup(dir);
    }

    #[test]
    fn the_password_is_redacted_wherever_a_request_is_logged() {
        assert_eq!(
            redact_line(&format!("CALL config.unlock pass={PASS}")),
            "CALL config.unlock pass=***"
        );
        // Redaction is by key, so a mistyped tool name still scrubs.
        assert_eq!(redact_line("CALL config.unlok pass=s3cret"), "CALL config.unlok pass=***");
        // Trailing args go too: a value may contain spaces, so anything after
        // `pass=` could still be part of the secret.
        assert_eq!(
            redact_line("CALL config.unlock pass=s3cret k=1"),
            "CALL config.unlock pass=***"
        );
        assert_eq!(redact_line("PING"), "PING");
        assert_eq!(redact_line("CALL search.query q=pass"), "CALL search.query q=pass");

        // Every request-log call site must go through it. This is the one place
        // a secret would otherwise land in .bridge.log verbatim. The needles
        // are assembled at runtime so this test cannot match its own source.
        let src = include_str!("main.rs");
        let arrow = '\u{2190}';
        let redacted = format!("eprintln!(\"{arrow} {{}}\", redact_line(&line));");
        let raw = format!("eprintln!(\"{arrow} {{line}}\")");
        assert!(src.contains(&redacted), "the request log must be redacted");
        assert!(!src.contains(&raw), "a raw request log came back");
    }

    #[test]
    fn unlock_over_a_socket_never_echoes_the_secret_back() {
        let _env = graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = scratch("echo", Some(PASS));
        let out = connection(&format!("CALL config.unlock pass={PASS}\nCALL config.status\n"));
        for line in &out {
            assert!(!line.contains(PASS), "secret echoed on the wire: {line}");
        }
        assert_eq!(out[0], "OK config.unlock");
        cleanup(dir);
    }

    fn args(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }
}

#[cfg(test)]
mod read_tests {
    use super::*;

    fn args(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn reading_calendar_needs_email_and_matches_list_id() {
        // Same id calendar.list emits for the first mock event.
        let id = graph::id_for("Demo event", "tomorrow");
        let denied = read_doc(&format!("cal://{id}"), 10, &args(&[])).unwrap_err();
        assert_eq!(denied, "needs_email_cap");
        let rows = read_doc(
            &format!("cal://{id}"),
            10,
            &args(&[("email", "1")]),
        )
        .expect("read");
        let text: String = rows
            .iter()
            .filter_map(|r| r.strip_prefix("ROW line="))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Demo event"), "{text}");
        assert!(text.contains("tomorrow"), "{text}");
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
        assert_eq!(
            e, "needs_audio_cap",
            "the files grant must not unlock recordings"
        );
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
        let e = read_doc(
            "file://../../../../etc/passwd",
            10,
            &args(&[("files", "1")]),
        )
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
        assert!(
            rows.iter().any(|r| r == "ROW line="),
            "paragraph break lost"
        );
    }

    #[test]
    fn a_word_longer_than_the_width_does_not_loop_forever() {
        let rows = wrap_lines(&"x".repeat(300), 20, 5);
        assert!(!rows.is_empty());
        assert!(rows.len() <= 5);
    }
}

#[cfg(test)]
mod agent_dispatch_tests {
    use super::*;

    fn backends() -> Backends {
        Backends {
            email: "mock".into(),
            search: "tfidf".into(),
        }
    }

    fn rows(reply: &[String]) -> usize {
        reply.iter().filter(|l| l.starts_with("ROW ")).count()
    }

    fn say(reply: &[String]) -> String {
        reply
            .iter()
            .find_map(|l| l.strip_prefix("SAY "))
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn the_row_limit_is_honoured_through_the_fallback_path() {
        // `agent::act` respected the limit while the keyword fallback appended
        // past it, so a plain search returned five rows however few were asked
        // for - and the sentence counted all five.
        for k in 1..=5 {
            let reply = dispatch(
                &format!("CALL agent.act goal=planning k={k} portal=1"),
                &backends(),
            );
            assert!(rows(&reply) <= k, "asked for {k}, got {}", rows(&reply));
        }
    }

    #[test]
    fn the_sentence_counts_the_rows_that_were_actually_sent() {
        for k in [1usize, 3, 5] {
            let reply = dispatch(
                &format!("CALL agent.act goal=planning k={k} portal=1"),
                &backends(),
            );
            let said = say(&reply);
            if let Ok(n) = said
                .split_whitespace()
                .next()
                .unwrap_or("x")
                .parse::<usize>()
            {
                assert_eq!(
                    n,
                    rows(&reply),
                    "said {n:?} but sent {}: {said}",
                    rows(&reply)
                );
            }
        }
    }

    #[test]
    fn the_header_count_matches_the_rows_too() {
        let reply = dispatch("CALL agent.act goal=planning k=2 portal=1", &backends());
        let n: usize = reply[0]
            .split_whitespace()
            .find_map(|f| f.strip_prefix("n="))
            .and_then(|v| v.parse().ok())
            .expect("OK line carries n=");
        assert_eq!(n, rows(&reply));
    }
}
