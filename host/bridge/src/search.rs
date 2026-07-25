//! tSearch-inspired lexical search over a curated `{t,u,c,b,pr}` corpus.
//!
//! Mirrors the **agent** path from tsearch-revival (`tsearch_mcp.t_search`):
//! tokenize → exact AND ladder → tf-idf × (1 + α·PageRank) — not the full
//! browser BM25+LSA hybrid.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Deserialize)]
struct CorpusFile {
    docs: Vec<Doc>,
}

#[derive(Debug, Clone, Deserialize)]
struct Doc {
    t: String,
    #[serde(default)]
    u: String,
    #[serde(default)]
    c: String,
    #[serde(default)]
    b: String,
    #[serde(default)]
    pr: f64,
}

/// Transcripts, projected into corpus documents so speech is searchable
/// next to files and mail.
fn transcript_docs() -> Vec<Doc> {
    crate::transcribe::Store::load()
        .items
        .into_iter()
        .map(|t| Doc {
            t: t.title,
            u: format!("audio://{}", t.source),
            c: "audio".to_string(),
            b: t.text,
            pr: 0.6,
        })
        .collect()
}

/// Workspace files, projected into corpus documents.
///
/// Reached only when the caller passed `files=1`, i.e. the user granted
/// workspace.index during setup.
fn workspace_docs() -> Vec<Doc> {
    crate::workspace::Index::load()
        .entries
        .into_iter()
        .map(|e| Doc {
            t: e.title,
            u: format!("file://{}", e.path),
            c: "file".to_string(),
            b: e.snippet,
            pr: e.pr,
        })
        .collect()
}

/// Email graph entries, projected into corpus documents.
fn email_docs() -> Vec<Doc> {
    crate::graph::Graph::load_or_empty()
        .messages
        .into_iter()
        .map(|m| Doc {
            t: m.subject,
            u: format!("email://{}", m.id),
            c: "email".to_string(),
            // Sender is indexed so "from alice" style queries hit.
            b: format!("{} {}", m.from, m.snippet),
            pr: m.pr,
        })
        .collect()
}

fn corpus_path() -> PathBuf {
    env::var("OS_SEARCH_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../search/corpus.json"))
}

fn load_docs() -> Result<Vec<Doc>, String> {
    let path = corpus_path();
    let raw = fs::read_to_string(&path).map_err(|e| format!("corpus_missing {}: {e}", path.display()))?;
    let file: CorpusFile = serde_json::from_str(&raw).map_err(|e| format!("corpus_json: {e}"))?;
    Ok(file.docs)
}

fn fold_tok(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    // ASCII-oriented split; good enough for the curated OS corpus.
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_string())
        .collect()
}

fn search_tfidf(docs: &[Doc], query: &str, k: usize, cat: Option<&str>) -> Vec<(f64, usize)> {
    let q_terms: Vec<String> = fold_tok(query);
    if q_terms.is_empty() || docs.is_empty() {
        return Vec::new();
    }
    let q_set: HashSet<&str> = q_terms.iter().map(|s| s.as_str()).collect();

    // Document tokens (title counts double — tSearch client ethos).
    let mut doc_toks: Vec<Vec<String>> = Vec::with_capacity(docs.len());
    for d in docs {
        let mut t = fold_tok(&d.t);
        t.extend(fold_tok(&d.t)); // title double
        t.extend(fold_tok(&d.b));
        doc_toks.push(t);
    }

    let n = docs.len() as f64;
    let mut df: HashMap<&str, f64> = HashMap::new();
    for terms in &doc_toks {
        let uniq: HashSet<&str> = terms.iter().map(|s| s.as_str()).collect();
        for term in uniq {
            *df.entry(term).or_insert(0.0) += 1.0;
        }
    }

    // Exact-first ladder: prefer docs that contain ALL query terms; else OR.
    let mut scored: Vec<(f64, usize)> = Vec::new();
    for (i, (d, terms)) in docs.iter().zip(doc_toks.iter()).enumerate() {
        if let Some(cat) = cat {
            if d.c != cat {
                continue;
            }
        }
        let mut tf_score = 0.0;
        let mut hit_all = true;
        for qt in &q_set {
            let tf = terms.iter().filter(|t| t.as_str() == *qt).count() as f64;
            if tf == 0.0 {
                hit_all = false;
                continue;
            }
            let idf = ((n + 1.0) / (df.get(qt).copied().unwrap_or(0.0) + 1.0)).ln() + 1.0;
            tf_score += (tf / terms.len().max(1) as f64) * idf;
        }
        if tf_score <= 0.0 {
            continue;
        }
        // tSearch MCP blend: lexical * (1 + 8*pr) — we use milder 4× on 0..1 pr.
        let pr = if d.pr.is_finite() { d.pr.clamp(0.0, 1.0) } else { 0.0 };
        let mut score = tf_score * (1.0 + 4.0 * pr);
        if hit_all {
            score *= 1.35; // exact AND bonus (ladder)
        }
        scored.push((score, i));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored
}

fn snip(body: &str, query: &str) -> String {
    let q = fold_tok(query);
    let lower = body.to_lowercase();
    // `find` returns a byte offset into `lower`; that only maps back onto
    // `body` when lowercasing didn't change byte lengths. Otherwise anchor at
    // the start rather than slicing at a wrong (possibly non-boundary) offset.
    let mut best = 0usize;
    if lower.len() == body.len() {
        for term in &q {
            if let Some(i) = lower.find(term) {
                best = i;
                break;
            }
        }
    }
    let start = floor_char_boundary(body, best.saturating_sub(40));
    let end = floor_char_boundary(body, (best + 80).min(body.len()));
    let mut s = body[start..end].to_string();
    if start > 0 {
        s = format!("…{s}");
    }
    if end < body.len() {
        s.push('…');
    }
    s.replace('|', " ").chars().take(90).collect()
}

/// Largest char boundary <= i (stable substitute for `str::floor_char_boundary`).
fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn sanitize(s: &str) -> String {
    s.chars()
        // The guest font atlas covers ASCII 0x20..=0x7E only; anything else
        // renders as '?'. Emoji in document titles are common, so drop
        // non-ASCII rather than shipping rows of question marks.
        .filter(|c| c.is_ascii())
        .map(|c| match c {
            '\n' | '\r' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .take(90)
        .collect()
}

/// `include_email` folds the runtime email graph in alongside the static
/// corpus. It defaults to *off* everywhere: email content must stay behind the
/// email capability, or a caller holding only `search.query` could read mail.
/// `include_files` folds in the user's own indexed documents. Off by default
/// for the same reason as email: holding `search.query` grants the built-in
/// corpus, not a personal file tree.
#[allow(clippy::too_many_arguments)]
pub fn query_all(
    q: &str,
    k: usize,
    cat: Option<&str>,
    include_email: bool,
    include_files: bool,
    include_audio: bool,
) -> Result<Vec<String>, String> {
    let mut docs = load_docs()?;
    if include_files {
        docs.extend(workspace_docs());
    }
    if include_audio {
        docs.extend(transcript_docs());
    }
    if include_email {
        docs.extend(email_docs());
    }
    let mut hits = search_tfidf(&docs, q, k, cat);
    // The big corpus is scored from its prebuilt index, then merged. Scoring it
    // inline would re-tokenise 12k documents on every keystroke.
    let teddy = crate::tsearch::index();
    if !teddy.is_empty() && cat.is_none() {
        let tdocs = crate::tsearch::docs();
        for (score, i) in teddy.search(q, k) {
            docs.push(Doc {
                t: tdocs[i].t.clone(),
                u: tdocs[i].u.clone(),
                c: if tdocs[i].c.is_empty() { "teddy".into() } else { tdocs[i].c.clone() },
                b: tdocs[i].b.clone(),
                pr: tdocs[i].pr,
            });
            // `docs` just grew by one; that entry is what this score refers to.
            hits.push((score, docs.len() - 1));
        }
        hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k);
    }
    let n = hits.len();
    let mut out = vec![format!("OK search.query n={n} backend=tfidf-pr")];
    for (score, i) in hits {
        let d = &docs[i];
        out.push(format!(
            "ROW title={}|cat={}|score={:.3}|snip={}|url={}",
            sanitize(&d.t),
            sanitize(&d.c),
            score,
            sanitize(&snip(&d.b, q)),
            sanitize(&d.u)
        ));
    }
    out.push("END".into());
    Ok(out)
}

pub fn query_mock(q: &str, k: usize) -> Vec<String> {
    let samples = [
        ("os identity", "docs", "Agent-centric OS with capability-based agents"),
        ("MCP connectors", "docs", "Host bridge email skills search over COM2"),
        ("tSearch revival inspiration", "web", &format!("Lexical search inspired hit for {q}")),
    ];
    let n = samples.len().min(k);
    let mut out = vec![format!("OK search.query n={n} backend=mock")];
    for (title, cat, snip) in samples.iter().take(n) {
        out.push(format!("ROW title={title}|cat={cat}|score=1.0|snip={snip}|url=os://mock"));
    }
    out.push("END".into());
    out
}

/// Optional: shell out to tsearch-revival's MCP search helper when configured.
pub fn query_tsearch(q: &str, k: usize) -> Result<Vec<String>, String> {
    let data = env::var("TSEARCH_DATA").map_err(|_| "TSEARCH_DATA unset".to_string())?;
    let script = PathBuf::from(&data).join("tsearch_mcp.py");
    if !script.is_file() {
        return Err("tsearch_mcp.py missing".into());
    }
    // Tiny driver: import t_search via python -c. The query and data path go
    // through env vars — Rust's {:?} escaping is not valid Python for
    // non-ASCII/control chars, and embedding untrusted text in code invites
    // injection.
    let py = format!(
        r#"
import json, os, sys
data = os.environ["TSEARCH_DATA"]
sys.path.insert(0, data)
import tsearch_mcp as m
g = m.Graph()
hits = m.t_search(g, os.environ["TSEARCH_QUERY"], k={k})
print(json.dumps(hits))
"#,
    );
    let output = Command::new("python3")
        .args(["-c", &py])
        .env("TSEARCH_DATA", &data)
        .env("TSEARCH_QUERY", q)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.lines().next().unwrap_or("tsearch_failed").chars().take(80).collect());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value =
        serde_json::from_str(stdout.lines().last().unwrap_or("{}")).map_err(|e| e.to_string())?;
    // Accept either list of cards or {results:[...]}
    let items = val
        .get("hits")
        .and_then(|r| r.as_array().cloned())
        .or_else(|| val.as_array().cloned())
        .or_else(|| val.get("results").and_then(|r| r.as_array().cloned()))
        .unwrap_or_default();
    let mut out = vec![format!("OK search.query n={} backend=tsearch", items.len().min(k))];
    for item in items.into_iter().take(k) {
        let title = item
            .get("t")
            .or_else(|| item.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let cat = item.get("c").or_else(|| item.get("cat")).and_then(|v| v.as_str()).unwrap_or("");
        let snip = item
            .get("snippet")
            .or_else(|| item.get("b"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let url = item.get("u").or_else(|| item.get("url")).and_then(|v| v.as_str()).unwrap_or("");
        out.push(format!(
            "ROW title={}|cat={}|score={:.3}|snip={}|url={}",
            sanitize(title),
            sanitize(cat),
            score,
            sanitize(snip),
            sanitize(url)
        ));
    }
    out.push("END".into());
    Ok(out)
}

pub fn query_scoped(
    q: &str,
    k: usize,
    cat: Option<&str>,
    backend: &str,
    include_email: bool,
    include_files: bool,
    include_audio: bool,
) -> Vec<String> {
    match backend {
        "mock" => query_mock(q, k),
        "tsearch" => query_tsearch(q, k).unwrap_or_else(|e| vec![format!("ERR search.query {e}")]),
        _ => query_all(q, k, cat, include_email, include_files, include_audio)
            .unwrap_or_else(|e| vec![format!("ERR search.query {e}")]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_mcp_docs() {
        let docs = load_docs().expect("corpus");
        let hits = search_tfidf(&docs, "capability ambient root", 5, None);
        assert!(!hits.is_empty());
        let top = &docs[hits[0].1];
        assert!(
            top.t.contains("Architecture") || top.b.contains("ambient") || top.t.contains("capability"),
            "unexpected top hit {}",
            top.t
        );
    }

    #[test]
    fn mock_ok() {
        let r = query_mock("test", 2);
        assert!(r[0].starts_with("OK search.query"));
    }
}

/// Body text for a document URL, from the built-in corpus or teddysearch.
///
/// These sources carry their text in the index, so reading needs no file
/// access — and no capability beyond the one that found them.
pub fn body_for(url: &str, max_lines: usize) -> Option<Vec<String>> {
    let body = load_docs()
        .ok()?
        .into_iter()
        .find(|d| d.u == url)
        .map(|d| d.b)
        .or_else(|| {
            crate::tsearch::docs()
                .iter()
                .find(|d| d.u == url)
                .map(|d| d.b.clone())
        })?;
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in body.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > 78 {
            out.push(format!("ROW line={}", sanitize(&cur)));
            cur.clear();
            if out.len() >= max_lines {
                return Some(out);
            }
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        out.push(format!("ROW line={}", sanitize(&cur)));
    }
    Some(out)
}
