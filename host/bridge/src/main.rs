//! Host-side MCP-shaped connector bridge.
//!
//! Speaks the line protocol in docs/mcp-connectors-os-doc-v01.md over TCP.
//! Email backends: mock (default) or `gog`. Skills: defaults + saved on host.

mod search;
mod skills;

use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::Arc;

struct Backends {
    email: String,
    search: String,
}

fn main() {
    let addr = env::var("OS_MCP_BRIDGE_ADDR").unwrap_or_else(|_| "127.0.0.1:7420".into());
    let backends = Arc::new(Backends {
        email: env::var("OS_MCP_EMAIL_BACKEND").unwrap_or_else(|_| "mock".into()),
        search: env::var("OS_MCP_SEARCH_BACKEND").unwrap_or_else(|_| "tfidf".into()),
    });

    let (defaults, user) = skills::skills_dirs();
    eprintln!(
        "os-mcp-bridge listening on {addr} (email={}; search={}; skills defaults={} user={})",
        backends.email,
        backends.search,
        defaults.display(),
        user.display()
    );

    let listener = TcpListener::bind(&addr).expect("bind bridge");

    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                let backends = Arc::clone(&backends);
                std::thread::spawn(move || {
                    if let Err(e) = handle_client(stream, &backends) {
                        eprintln!("client error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}

fn handle_client(stream: TcpStream, backends: &Backends) -> std::io::Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(120)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        eprintln!("← {line}");

        // Multi-line save: CALL skills.save name=foo  then LINE… END
        if line.starts_with("CALL skills.save ") {
            let name = line
                .split_whitespace()
                .find_map(|a| a.strip_prefix("name="))
                .unwrap_or("")
                .to_string();
            let mut body = String::new();
            loop {
                let mut ln = String::new();
                let n = reader.read_line(&mut ln)?;
                if n == 0 {
                    break;
                }
                let t = ln.trim_end_matches(['\r', '\n']);
                if t == "END" {
                    break;
                }
                if let Some(rest) = t.strip_prefix("LINE ") {
                    body.push_str(rest);
                    body.push('\n');
                }
            }
            let reply = match skills::save_skill(&name, &body) {
                Ok(path) => vec![format!("OK skills.save path={}", path.display())],
                Err(e) => vec![format!("ERR skills.save {e}")],
            };
            for r in &reply {
                eprintln!("→ {r}");
                writeln!(writer, "{r}")?;
            }
            writer.flush()?;
            continue;
        }

        let reply = dispatch(line, backends);
        for r in &reply {
            eprintln!("→ {r}");
            writeln!(writer, "{r}")?;
        }
        writer.flush()?;
    }
    Ok(())
}

fn dispatch(line: &str, backends: &Backends) -> Vec<String> {
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    match cmd {
        "PING" => vec!["OK pong".into()],
        "LIST" => {
            vec!["OK tools=email.search,email.send,calendar.list,skills.list,skills.get,skills.save,search.query".into()]
        }
        "CALL" => {
            let tool = parts.next().unwrap_or("");
            let rest: Vec<&str> = parts.collect();
            call_tool(tool, &rest, backends)
        }
        _ => vec!["ERR unknown command".into()],
    }
}

fn call_tool(tool: &str, args: &[&str], backends: &Backends) -> Vec<String> {
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
            // Stub without body — creates a starter skill on disk.
            let name = arg_val(args, "name").unwrap_or("");
            let desc = arg_val(args, "desc").unwrap_or("User-saved skill.");
            let body = format!("---\nname: {name}\ndescription: {desc}\n---\n\n# {name}\n\n(edit me)\n");
            match skills::save_skill(name, &body) {
                Ok(path) => vec![format!("OK skills.save path={}", path.display())],
                Err(e) => vec![format!("ERR skills.save {e}")],
            }
        }
        "search.query" => {
            let q = arg_val(args, "q").unwrap_or("");
            let k: usize = arg_val(args, "k")
                .and_then(|s| s.parse().ok())
                .unwrap_or(5)
                .clamp(1, 20);
            let cat = arg_val(args, "cat");
            if q.is_empty() {
                vec!["ERR search.query missing_q".into()]
            } else {
                search::query(q, k, cat, &backends.search)
            }
        }
        _ => vec![format!("ERR {tool} not_found")],
    }
}

fn arg_val<'a>(args: &[&'a str], key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    args.iter().find_map(|a| a.strip_prefix(&prefix))
}

fn email_search(args: &[&str], backend: &str) -> Vec<String> {
    let query = arg_val(args, "q").unwrap_or("in:inbox");
    let max: usize = arg_val(args, "max")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5)
        .clamp(1, 20);

    match backend {
        "gog" => email_search_gog(query, max),
        _ => email_search_mock(query, max),
    }
}

fn email_search_mock(query: &str, max: usize) -> Vec<String> {
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
    let output = Command::new("gog")
        .args([
            "gmail",
            "search",
            query,
            "-j",
            "--results-only",
            "--no-input",
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
