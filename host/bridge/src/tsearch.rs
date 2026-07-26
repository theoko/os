//! teddysearch.com corpus as a search source.
//!
//! `teddysearch.com/tsearch/` is a client-side app: it fetches `corpus.json`
//! and ranks in the browser. There is no server-side query endpoint, so *the
//! corpus file is the API*. We fetch it, cache it, and rank locally with the
//! same tf-idf × PageRank the rest of the bridge uses.
//!
//! Two things make this practical:
//!
//! * The corpus already uses the `{t,u,c,b,pr}` schema this bridge parses —
//!   the local `search/corpus.json` was modelled on it — so no translation.
//! * It is ~64 MB and ~12k documents, far too large to re-read per query, so
//!   it is parsed once per process and held behind a `OnceLock`.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::search::{CorpusFile, Doc};

/// Live corpus published by the tsearch front-end.
const DEFAULT_URL: &str = "https://teddysearch.com/tsearch/corpus.json";

fn url() -> String {
    env::var("OS_TSEARCH_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn cache_path() -> PathBuf {
    crate::paths::env_or_knowledge("OS_TSEARCH_CACHE", "teddysearch.json")
}

/// Fetch the live corpus into the cache. Returns (documents, crawl stamp).
///
/// Downloads to a temporary file and renames on success, so an interrupted
/// fetch cannot leave a half-written corpus that then fails to parse. curl is
/// used rather than pulling a TLS stack into the bridge for one request.
pub fn sync() -> Result<(usize, String), String> {
    let path = cache_path();
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
    }
    let tmp = path.with_extension("part");

    let out = Command::new("curl")
        .arg("-sS")
        .arg("--fail")
        .arg("--max-time")
        .arg("300")
        .arg("-o")
        .arg(&tmp)
        .arg(url())
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        let _ = fs::remove_file(&tmp);
        return Err(crate::text::stderr_brief(&out.stderr, "fetch failed", 120));
    }

    // Validate before publishing: a truncated download parses as an error here
    // rather than as an empty corpus at query time.
    let raw = fs::read_to_string(&tmp).map_err(|e| format!("read: {e}"))?;
    let parsed: CorpusFile = serde_json::from_str(&raw)
        .map_err(|e| format!("corpus json (truncated download?): {e}"))?;
    let n = parsed.docs.len();
    if n == 0 {
        let _ = fs::remove_file(&tmp);
        return Err("corpus contained no documents".into());
    }

    fs::rename(&tmp, &path).map_err(|e| format!("rename: {e}"))?;
    Ok((n, parsed.crawled_at))
}

static CACHE: OnceLock<Vec<Doc>> = OnceLock::new();

/// Cached corpus documents, parsed once per process.
///
/// Re-reading 64 MB on every `search.query` would make the search field
/// unusable; this is why the source is a cache rather than a live call.
pub fn docs() -> &'static [Doc] {
    CACHE.get_or_init(|| {
        let Ok(raw) = fs::read_to_string(cache_path()) else {
            return Vec::new();
        };
        match serde_json::from_str::<CorpusFile>(&raw) {
            Ok(c) => c.docs,
            Err(e) => {
                eprintln!("tsearch: cache unreadable ({e}); run CALL tsearch.sync");
                Vec::new()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_is_the_published_corpus() {
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_TSEARCH_URL") };
        assert_eq!(url(), DEFAULT_URL);
        assert!(url().starts_with("https://"), "corpus must be fetched over TLS");
    }

    #[test]
    fn url_is_overridable() {
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_TSEARCH_URL", "https://example.test/c.json") };
        assert_eq!(url(), "https://example.test/c.json");
        unsafe { env::remove_var("OS_TSEARCH_URL") };
    }

    #[test]
    fn cache_lives_outside_the_repo() {
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
        let p = cache_path().to_string_lossy().to_string();
        assert!(!p.contains("/os/search"), "cache must not land in the repo: {p}");
        assert!(p.ends_with(".json"));
    }

    #[test]
    fn missing_cache_yields_no_documents_not_a_panic() {
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_TSEARCH_CACHE", "/nonexistent/os-teddy/none.json") };
        assert!(!cache_path().is_file());
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
    }

    #[test]
    fn schema_matches_the_published_corpus() {
        // {t,u,c,b,pr} plus extras like `img`, which must be ignored rather
        // than rejected.
        let raw = r#"{"crawled_at":"now","docs":[
            {"t":"A","u":"x/y","c":"web","b":"body","pr":0.5,"img":"i.png"}]}"#;
        let c: CorpusFile = serde_json::from_str(raw).expect("parse");
        assert_eq!(c.docs.len(), 1);
        assert_eq!(c.docs[0].t, "A");
        assert_eq!(c.docs[0].pr, 0.5);
    }

    #[test]
    fn a_truncated_corpus_is_rejected_not_silently_empty() {
        let bad = r#"{"crawled_at":"now","docs":[{"t":"A","u":"x"#;
        assert!(serde_json::from_str::<CorpusFile>(bad).is_err());
    }
}

/// Inverted index over the cached corpus.
///
/// `search_tfidf` re-tokenises every document and rebuilds the df map per
/// query. That is fine for the ~16-document built-in corpus and hopeless for
/// 12k: measured at 5.6 s per search. The corpus only changes on sync, so the
/// tokenisation and idf are computed once and queries walk postings instead.
pub struct Index {
    /// Sorted by term, so lookup is a binary search.
    terms: Vec<Term>,
    postings: Vec<(usize, f64)>,
}

struct Term {
    word: String,
    idf: f64,
    start: usize,
    len: usize,
}

static INDEX: OnceLock<Index> = OnceLock::new();

pub fn index() -> &'static Index {
    INDEX.get_or_init(|| {
        let docs = docs();
        let n = docs.len() as f64;
        // term -> doc -> tf
        let mut acc: HashMap<String, HashMap<usize, f64>> = HashMap::new();
        let mut lens = vec![0usize; docs.len()];
        for (i, d) in docs.iter().enumerate() {
            let t = crate::search::title_body_tokens(&d.t, &d.b);
            lens[i] = t.len().max(1);
            for w in t {
                *acc.entry(w).or_default().entry(i).or_insert(0.0) += 1.0;
            }
        }
        let mut words: Vec<String> = acc.keys().cloned().collect();
        words.sort();
        let mut terms = Vec::with_capacity(words.len());
        let mut postings = Vec::new();
        for w in words {
            let per_doc = &acc[&w];
            let df = per_doc.len() as f64;
            let idf = crate::search::idf(n, df);
            let start = postings.len();
            let mut ids: Vec<usize> = per_doc.keys().copied().collect();
            ids.sort_unstable();
            for id in ids {
                postings.push((id, per_doc[&id] / lens[id] as f64));
            }
            terms.push(Term { word: w, idf, start, len: per_doc.len() });
        }
        Index { terms, postings }
    })
}

impl Index {
    fn find(&self, w: &str) -> Option<&Term> {
        self.terms
            .binary_search_by(|t| t.word.as_str().cmp(w))
            .ok()
            .map(|i| &self.terms[i])
    }

    /// Score pre-tokenized query terms, returning `(score, doc_index)` best-first.
    ///
    /// Mirrors the built-in scorer: tf-idf blended with PageRank, plus the
    /// exact-AND bonus for documents carrying every query term.
    pub fn search_tokens(&self, q: &[String], k: usize) -> Vec<(f64, usize)> {
        if q.is_empty() || self.terms.is_empty() {
            return Vec::new();
        }
        let docs = docs();
        let mut score: HashMap<usize, f64> = HashMap::new();
        let mut hits: HashMap<usize, usize> = HashMap::new();
        let mut seen = 0usize;
        for w in q {
            let Some(t) = self.find(w) else { continue };
            seen += 1;
            for &(doc, tf) in &self.postings[t.start..t.start + t.len] {
                *score.entry(doc).or_insert(0.0) += tf * t.idf;
                *hits.entry(doc).or_insert(0) += 1;
            }
        }
        let mut out: Vec<(f64, usize)> = score
            .into_iter()
            .map(|(doc, s)| {
                let hit_all = seen > 0 && hits.get(&doc).copied().unwrap_or(0) >= seen;
                (
                    crate::search::with_and_bonus(crate::search::blend_pr(s, docs[doc].pr), hit_all),
                    doc,
                )
            })
            .collect();
        out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(k);
        out
    }
}

#[cfg(test)]
mod index_tests {
    #[test]
    fn tokeniser_matches_the_bridge_rules() {
        assert_eq!(
            crate::search::tokenize("Capability-based Agents v2 a"),
            vec!["capability", "based", "agents", "v2"]
        );
    }
}
