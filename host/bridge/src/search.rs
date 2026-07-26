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

/// On-disk / HTTP corpus shell around `{t,u,c,b,pr}` documents.
#[derive(Debug, Deserialize)]
pub struct CorpusFile {
    #[serde(default)]
    pub docs: Vec<Doc>,
    #[serde(default)]
    pub crawled_at: String,
}

/// Curated / teddy / projected corpus document (`{t,u,c,b,pr}`).
#[derive(Debug, Clone, Deserialize)]
pub struct Doc {
    pub t: String,
    #[serde(default)]
    pub u: String,
    #[serde(default)]
    pub c: String,
    #[serde(default)]
    pub b: String,
    #[serde(default)]
    pub pr: f64,
}

/// Inverse document frequency used by both the small-corpus scan and the teddy index.
pub(crate) fn idf(n_docs: f64, df: f64) -> f64 {
    ((n_docs + 1.0) / (df + 1.0)).ln() + 1.0
}

/// Title tokens twice, then body — tSearch client ethos.
pub(crate) fn title_body_tokens(title: &str, body: &str) -> Vec<String> {
    let mut t = tokenize(title);
    t.extend(tokenize(title));
    t.extend(tokenize(body));
    t
}

/// tf-idf × (1 + 4·PageRank), with finite/clamp on `pr`.
pub(crate) fn blend_pr(tf_score: f64, pr: f64) -> f64 {
    let pr = if pr.is_finite() { pr.clamp(0.0, 1.0) } else { 0.0 };
    tf_score * (1.0 + 4.0 * pr)
}

/// Exact-AND ladder bonus when every query term hit.
pub(crate) fn with_and_bonus(score: f64, hit_all: bool) -> f64 {
    if hit_all {
        score * 1.35
    } else {
        score
    }
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
            b: m.from,
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

/// ASCII-oriented tokenizer shared with the teddy index.
pub(crate) fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_string())
        .collect()
}

fn search_tfidf(docs: &[Doc], query: &str, k: usize, cat: Option<&str>) -> Vec<(f64, usize)> {
    let q_terms: Vec<String> = tokenize(query);
    if q_terms.is_empty() || docs.is_empty() {
        return Vec::new();
    }
    let q_set: HashSet<&str> = q_terms.iter().map(|s| s.as_str()).collect();

    let mut doc_toks: Vec<Vec<String>> = Vec::with_capacity(docs.len());
    for d in docs {
        doc_toks.push(title_body_tokens(&d.t, &d.b));
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
            tf_score += (tf / terms.len().max(1) as f64)
                * idf(n, df.get(qt).copied().unwrap_or(0.0));
        }
        if tf_score <= 0.0 {
            continue;
        }
        scored.push((with_and_bonus(blend_pr(tf_score, d.pr), hit_all), i));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored
}

fn snip(body: &str, query: &str) -> String {
    let q = tokenize(query);
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
    // Callers run `sanitize` (ASCII + pipe scrub + cap); keep the raw window here.
    body[start..end].to_string()
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
    // Guest font atlas is ASCII 0x20..=0x7E only; drop the rest.
    crate::text::sanitize(s, 90, true, false)
}

/// Search the curated corpus (plus optional email / files / audio scopes).
///
/// `include_email` / `include_files` / `include_audio` default off everywhere:
/// holding `search.query` alone must not reach mail, personal files, or
/// recordings.
pub fn query_all(
    q: &str,
    k: usize,
    cat: Option<&str>,
    include_email: bool,
    include_files: bool,
    include_audio: bool,
) -> Vec<String> {
    let mut docs = match load_docs() {
        Ok(d) => d,
        Err(e) => return vec![format!("ERR search.query {e}")],
    };
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
            let mut d = tdocs[i].clone();
            if d.c.is_empty() {
                d.c = "teddy".into();
            }
            docs.push(d);
            // `docs` just grew by one; that entry is what this score refers to.
            hits.push((score, docs.len() - 1));
        }
        hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        hits.truncate(k);
    }
    let n = hits.len();
    let rows = hits.into_iter().map(|(score, i)| {
        let d = &docs[i];
        format!(
            "ROW title={}|cat={}|score={:.3}|snip={}|url={}",
            sanitize(&d.t),
            sanitize(&d.c),
            score,
            sanitize(&snip(&d.b, q)),
            sanitize(&d.u)
        )
    });
    crate::text::framed_ok(format!("OK search.query n={n} backend=tfidf-pr"), rows)
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
    Some(wrap_lines(&body, 78, max_lines))
}

/// Hard-wrap text into `ROW line=...` entries the guest can render directly.
pub(crate) fn wrap_lines(text: &str, width: usize, max: usize) -> Vec<String> {
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
                out.push(format!("ROW line={}", sanitize(&cur)));
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
            out.push(format!("ROW line={}", sanitize(&cur)));
        }
    }
    out.truncate(max);
    out
}
