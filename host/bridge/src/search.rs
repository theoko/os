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
use std::sync::OnceLock;

/// On-disk / HTTP corpus shell around `{t,u,c,b,pr}` documents.
/// Extra JSON keys (e.g. historical `crawled_at`) are ignored.
#[derive(Deserialize)]
pub(crate) struct CorpusFile {
    #[serde(default)]
    pub docs: Vec<Doc>,
}

/// Curated / teddy / projected corpus document (`{t,u,c,b,pr}`).
#[derive(Clone, Deserialize)]
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
    let title_toks = tokenize(title);
    let mut t = Vec::with_capacity(title_toks.len() * 2 + 8);
    t.extend_from_slice(&title_toks);
    t.extend(title_toks);
    t.extend(tokenize(body));
    t
}

/// tf-idf × (1 + 4·PageRank), then the exact-AND ladder bonus.
pub(crate) fn rank_score(tf_score: f64, pr: f64, hit_all: bool) -> f64 {
    let pr = if pr.is_finite() { pr.clamp(0.0, 1.0) } else { 0.0 };
    let score = tf_score * (1.0 + 4.0 * pr);
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

fn corpus_path() -> PathBuf {
    env::var("OS_SEARCH_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../search/corpus.json"))
}

/// Curated corpus, parsed once per process (same idea as teddy's cache).
static CURATED: OnceLock<Result<Vec<Doc>, String>> = OnceLock::new();

fn load_docs_from_disk() -> Result<Vec<Doc>, String> {
    let path = corpus_path();
    let raw = fs::read_to_string(&path).map_err(|e| format!("corpus_missing {}: {e}", path.display()))?;
    let file: CorpusFile = serde_json::from_str(&raw).map_err(|e| format!("corpus_json: {e}"))?;
    Ok(file.docs)
}

fn load_docs() -> Result<&'static [Doc], &'static str> {
    match CURATED.get_or_init(load_docs_from_disk).as_ref() {
        Ok(docs) => Ok(docs.as_slice()),
        Err(e) => Err(e.as_str()),
    }
}

/// ASCII-oriented tokenizer shared with the teddy index.
///
/// Tokens are ascii-alphanumeric runs, so per-token `to_ascii_lowercase`
/// avoids allocating a full lowercased copy of the haystack.
pub(crate) fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

fn search_tfidf(
    docs: &[Doc],
    q_terms: &[String],
    k: usize,
) -> Vec<(f64, usize)> {
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
        scored.push((rank_score(tf_score, d.pr, hit_all), i));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored
}

/// Guest `search::{TITLE,URL,CAT}_CHARS` (title / url / cat).
const TITLE_CHARS: usize = 56;
const URL_CHARS: usize = 72;
const CAT_CHARS: usize = 16;

/// Search the curated corpus (plus optional files / audio scopes).
///
/// `include_files` / `include_audio` default off everywhere: holding
/// `search.query` alone must not reach personal files or recordings.
/// Mail stays peek-only via `email.search` (no openable search hits).
pub fn query_all(
    q: &str,
    k: usize,
    include_files: bool,
    include_audio: bool,
) -> Vec<String> {
    let curated = match load_docs() {
        Ok(d) => d,
        Err(e) => return vec![format!("ERR search.query {e}")],
    };
    let q_terms = tokenize(q);
    if q_terms.is_empty() {
        return crate::text::framed_ok("OK search.query".into(), []);
    }
    // Default search.query only needs the curated slice — clone only when a
    // scope adds personal docs into the same scoring universe.
    let docs: std::borrow::Cow<'_, [Doc]> =
        if include_files || include_audio {
            let mut v = curated.to_vec();
            if include_files {
                v.extend(workspace_docs());
            }
            if include_audio {
                v.extend(transcript_docs());
            }
            std::borrow::Cow::Owned(v)
        } else {
            std::borrow::Cow::Borrowed(curated)
        };
    // Local indices into `docs`; teddy indices into the cached corpus — never
    // clone 64 MB bodies into the local vec just to re-address them.
    enum Src {
        Local(usize),
        Teddy(usize),
    }
    let mut hits: Vec<(f64, Src)> = search_tfidf(&docs, &q_terms, k)
        .into_iter()
        .map(|(s, i)| (s, Src::Local(i)))
        .collect();
    // The big corpus is scored from its prebuilt index, then merged. Scoring it
    // inline would re-tokenise 12k documents on every keystroke.
    // Empty teddy index: `search` returns nothing; merge is a no-op.
    let tdocs = crate::tsearch::docs();
    if !tdocs.is_empty() {
        let before = hits.len();
        let teddy = crate::tsearch::index();
        for (score, i) in teddy.search_tokens(&q_terms, k) {
            hits.push((score, Src::Teddy(i)));
        }
        if hits.len() > before {
            hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            hits.truncate(k);
        }
    }
    let rows = hits.into_iter().map(|(_score, src)| {
        let (t, c, u) = match src {
            Src::Local(i) => {
                let d = &docs[i];
                (d.t.as_str(), d.c.as_str(), d.u.as_str())
            }
            Src::Teddy(i) => {
                let d = &tdocs[i];
                (
                    d.t.as_str(),
                    if d.c.is_empty() { "teddy" } else { d.c.as_str() },
                    d.u.as_str(),
                )
            }
        };
        format!(
            "ROW title={}|cat={}|url={}",
            crate::text::guest_slot(t, TITLE_CHARS),
            crate::text::guest_slot(c, CAT_CHARS),
            crate::text::guest_slot(u, URL_CHARS)
        )
    });
    crate::text::framed_ok("OK search.query".into(), rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_mcp_docs() {
        let docs = load_docs().expect("corpus");
        let q = tokenize("capability ambient root");
        let hits = search_tfidf(&docs, &q, 5);
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
/// access — and no capability beyond the one that found them. Callers wrap
/// with [`wrap_lines`] (same path as `file://` / `audio://`).
pub fn body_for(url: &str) -> Option<&'static str> {
    load_docs()
        .ok()?
        .iter()
        .find(|d| d.u == url)
        .map(|d| d.b.as_str())
        .or_else(|| {
            crate::tsearch::docs()
                .iter()
                .find(|d| d.u == url)
                .map(|d| d.b.as_str())
        })
}

/// Soft-wrap / sanitize budget for `ROW line=` (guest `DocPage::LINE_CHARS`).
pub(crate) const LINE_CHARS: usize = 78;

/// Hard-wrap text into `ROW line=...` entries the guest can render directly.
pub(crate) fn wrap_lines(text: &str, max: usize) -> Vec<String> {
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
            if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > LINE_CHARS {
                out.push(format!("ROW line={}", crate::text::guest_slot(&cur, LINE_CHARS)));
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
            out.push(format!("ROW line={}", crate::text::guest_slot(&cur, LINE_CHARS)));
        }
    }
    out.truncate(max);
    out
}
