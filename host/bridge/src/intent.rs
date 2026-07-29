//! Smart intent planner for natural-language Home goals.
//!
//! Inference stays on the host. The guest sends the raw ask; this module
//! classifies the act, expands synonyms, ranks the workspace index, and
//! returns a structured plan the guest can execute under caps.
//!
//! No cloud LLM in v1 — the ranking is deterministic and CI-safe. A future
//! `OS_MCP_INTENT_BACKEND=llm` can swap the body without changing the wire.

use crate::workspace;

/// What the guest should try to do first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Open,
    Search,
    Mail,
}

impl Act {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Search => "search",
            Self::Mail => "mail",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub why: String,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub act: Act,
    pub query: String,
    pub plans: Vec<String>,
    pub hits: Vec<Hit>,
}

/// Resolve a free-form ask into act + query + optional pre-ranked hits.
pub fn resolve(goal: &str, with_files: bool, with_email: bool) -> Plan {
    let goal = goal.trim();
    let act = classify_act(goal);
    let query = expand_query(goal);
    let mut plans = Vec::new();
    match act {
        Act::Open => {
            plans.push("Find the document that matches your ask".into());
            plans.push("Open the best match in the Reader".into());
            if with_files {
                plans.push("Prefer Your files over the built-in corpus".into());
            }
        }
        Act::Search => {
            plans.push("Search under your grants".into());
            plans.push("Cite titles; open a hit if one fits".into());
        }
        Act::Mail => {
            plans.push("Read the inbox under Email".into());
            plans.push("Surface urgent or matching threads".into());
        }
    }

    let mut hits = Vec::new();
    if with_files && matches!(act, Act::Open | Act::Search) {
        hits.extend(rank_workspace(&query, 3));
        if hits.is_empty() {
            // Broader pass: score against the raw goal tokens too.
            hits.extend(rank_workspace(goal, 3));
        }
    }
    if hits.is_empty() && matches!(act, Act::Open | Act::Search) {
        plans.push("No local file match yet - fall through to agent.act".into());
    }
    if act == Act::Mail && !with_email {
        plans.push("Email is off - grant it on Capabilities to act".into());
    }

    Plan {
        act,
        query,
        plans,
        hits,
    }
}

fn classify_act(goal: &str) -> Act {
    let g = goal.to_ascii_lowercase();
    if contains_any(&g, &["inbox", "email", "mail", "gmail"]) {
        return Act::Mail;
    }
    if contains_any(
        &g,
        &[
            "work on",
            "working on",
            "continue",
            "finish",
            "write",
            "edit",
            "open",
            "read",
            "paper",
            "thesis",
            "draft",
            "essay",
            "doc",
            "document",
            "notes",
        ],
    ) {
        return Act::Open;
    }
    if contains_any(&g, &["find", "search", "look up", "where is", "show me"]) {
        return Act::Search;
    }
    // Default: treat as open/work — Home asks are usually "help me do X".
    Act::Open
}

fn expand_query(goal: &str) -> String {
    let mut tokens = content_tokens(goal);
    // Synonym expansion for common work asks.
    let joined = tokens.join(" ");
    if contains_any(&joined, &["paper", "thesis", "essay", "manuscript"]) {
        for extra in ["paper", "thesis", "draft", "manuscript", "writeup"] {
            push_unique(&mut tokens, extra);
        }
    }
    if contains_any(&joined, &["notes", "note"]) {
        for extra in ["notes", "memo", "meeting"] {
            push_unique(&mut tokens, extra);
        }
    }
    if contains_any(&joined, &["cap", "capability", "caps"]) {
        for extra in ["capability", "caps", "grant"] {
            push_unique(&mut tokens, extra);
        }
    }
    if tokens.is_empty() {
        // Last resort: keep non-stop tokens even if short.
        tokens = content_tokens(goal);
    }
    tokens.into_iter().take(8).collect::<Vec<_>>().join(" ")
}

fn content_tokens(goal: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "i", "im", "i'm", "wanna", "want", "to", "a", "an", "the", "my", "me", "on", "in",
        "for", "of", "and", "or", "please", "can", "you", "we", "work", "working", "get",
        "got", "do", "doing", "help", "with", "about", "find", "show", "open", "read",
        "look", "looking", "gotta", "gonna", "just", "some", "any", "this", "that", "it",
        "up", "at", "from", "into", "onto",
    ];
    goal.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| !STOP.contains(&t.as_str()))
        .collect()
}

fn push_unique(tokens: &mut Vec<String>, extra: &str) {
    if !tokens.iter().any(|t| t == extra) {
        tokens.push(extra.into());
    }
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

fn rank_workspace(query: &str, k: usize) -> Vec<Hit> {
    let tokens: Vec<&str> = query
        .split_whitespace()
        .filter(|t| t.len() >= 2)
        .collect();
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(f64, Hit)> = Vec::new();
    for e in workspace::Index::load().entries {
        let title_l = e.title.to_ascii_lowercase();
        let path_l = e.path.to_ascii_lowercase();
        let snip_l = e.snippet.to_ascii_lowercase();
        let mut score = 0.0;
        let mut why = String::new();
        for t in &tokens {
            if title_l.contains(t) {
                score += 4.0;
                if why.is_empty() {
                    why = format!("title matches {t}");
                }
            } else if path_l.contains(t) {
                score += 2.5;
                if why.is_empty() {
                    why = format!("path matches {t}");
                }
            } else if snip_l.contains(t) {
                score += 1.0;
                if why.is_empty() {
                    why = format!("snippet matches {t}");
                }
            }
        }
        if score <= 0.0 {
            continue;
        }
        score += e.pr; // shallow / index-like files win ties
        scored.push((
            score,
            Hit {
                title: e.title,
                url: format!("file://{}", e.path),
                why: if why.is_empty() {
                    "ranked match".into()
                } else {
                    why
                },
            },
        ));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().take(k).map(|(_, h)| h).collect()
}

/// Wire response lines for `intent.resolve`.
pub fn wire_response(plan: &Plan) -> Vec<String> {
    let mut out = vec![format!(
        "OK intent.resolve act={} query={}",
        plan.act.as_str(),
        sanitize(&plan.query)
    )];
    for p in &plan.plans {
        out.push(format!("ROW plan={}", sanitize(p)));
    }
    for h in &plan.hits {
        // Same ROW shape as search hits so the guest parser stays one path.
        out.push(format!(
            "ROW title={}|url={}|why={}",
            sanitize(&h.title),
            sanitize(&h.url),
            sanitize(&h.why)
        ));
    }
    out.push("END".into());
    out
}

fn sanitize(s: &str) -> String {
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
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn paper_ask_is_open_with_expanded_query() {
        let p = resolve("i wanna work on my paper", false, false);
        assert_eq!(p.act, Act::Open);
        assert!(p.query.contains("paper"), "{}", p.query);
        assert!(p.query.contains("thesis") || p.query.contains("draft"), "{}", p.query);
        assert!(!p.plans.is_empty());
        // Guest fills knowledge hits via agent.act — never CALL search.query.
        assert!(
            p.plans.iter().any(|s| s.contains("agent.act")),
            "fall-through plan must name agent.act: {:?}",
            p.plans
        );
        assert!(
            p.plans.iter().all(|s| !s.contains("search.query")),
            "fall-through must not name search.query: {:?}",
            p.plans
        );
    }

    #[test]
    fn inbox_ask_is_mail() {
        let p = resolve("check my inbox", false, true);
        assert_eq!(p.act, Act::Mail);
    }

    #[test]
    fn workspace_hit_surfaces_matching_file() {
        let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-intent-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let ix = dir.join("workspace.json");
        let body = serde_json::json!({
            "entries": [{
                "title": "Q2 Research Paper",
                "path": "writing/q2-research-paper.md",
                "snippet": "Draft of the research paper.",
                "pr": 0.9
            }]
        });
        std::fs::write(&ix, body.to_string()).unwrap();
        unsafe {
            std::env::set_var("OS_WORKSPACE_INDEX", &ix);
        }
        let p = resolve("i wanna work on my paper", true, false);
        assert_eq!(p.act, Act::Open);
        assert_eq!(p.hits.len(), 1);
        assert!(p.hits[0].url.contains("q2-research-paper"));
        assert!(p.hits[0].why.contains("paper") || p.hits[0].why.contains("title"));
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("OS_WORKSPACE_INDEX");
        }
    }

    #[test]
    fn wire_response_is_row_shaped() {
        let p = resolve("find capability docs", false, false);
        let lines = wire_response(&p);
        assert!(lines[0].starts_with("OK intent.resolve"));
        assert!(lines.iter().any(|l| l.starts_with("ROW plan=")));
        assert_eq!(lines.last().map(String::as_str), Some("END"));
    }
}
