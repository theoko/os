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
//!   it is parsed once and held in process memory. Memory is a `Mutex` (not
//!   `OnceLock`) so `forget` can purge it when the user revokes portal.sync.

use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Live corpus published by the tsearch front-end.
pub const DEFAULT_URL: &str = "https://teddysearch.com/tsearch/corpus.json";

#[derive(Debug, Deserialize)]
struct CorpusFile {
    #[serde(default)]
    docs: Vec<RawDoc>,
    #[serde(default)]
    crawled_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawDoc {
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

pub fn url() -> String {
    env::var("OS_TSEARCH_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

pub fn cache_path() -> PathBuf {
    env::var("OS_TSEARCH_CACHE")
        .map(PathBuf::from)
        // Renamed from teddysearch.json; an existing cache is simply re-synced
        // rather than migrated, since it is a downloadable artifact.
        .unwrap_or_else(|_| home().join("Library/Application Support/os/knowledge/teddy.json"))
}

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
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
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().last().unwrap_or("fetch failed").chars().take(120).collect());
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
    // Replace in-memory state so the next search sees the new corpus without
    // restarting the bridge (OnceLock could not do this after a revoke/resync).
    publish(Arc::new(Corpus::from_docs(parsed.docs)));
    Ok((n, parsed.crawled_at))
}

/// In-memory corpus + index. `None` means "load from disk on next use".
static CORPUS: Mutex<Option<Arc<Corpus>>> = Mutex::new(None);

struct Corpus {
    docs: Arc<Vec<RawDoc>>,
    index: Arc<Index>,
}

impl Corpus {
    fn empty() -> Self {
        let docs = Arc::new(Vec::new());
        let index = Arc::new(Index::build(&docs));
        Self { docs, index }
    }

    fn from_docs(docs: Vec<RawDoc>) -> Self {
        let docs = Arc::new(docs);
        let index = Arc::new(Index::build(&docs));
        Self { docs, index }
    }

    fn from_disk() -> Self {
        let Ok(raw) = fs::read_to_string(cache_path()) else {
            return Self::empty();
        };
        match serde_json::from_str::<CorpusFile>(&raw) {
            Ok(c) => Self::from_docs(c.docs),
            Err(e) => {
                eprintln!("tsearch: cache unreadable ({e}); run CALL tsearch.sync");
                Self::empty()
            }
        }
    }
}

fn publish(c: Arc<Corpus>) {
    if let Ok(mut g) = CORPUS.lock() {
        *g = Some(c);
    }
}

fn loaded() -> Arc<Corpus> {
    let mut g = CORPUS.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(Arc::new(Corpus::from_disk()));
    }
    Arc::clone(g.as_ref().unwrap())
}

/// Drop in-memory state so the next access reloads from disk (or emptiness).
pub fn clear_memory() {
    if let Ok(mut g) = CORPUS.lock() {
        *g = None;
    }
}

/// Delete the on-disk teddy corpus and purge the in-memory index.
///
/// Revoking `portal.sync` must not leave a 12k-doc cache behind a switch that
/// reads as off — "hidden" is not "forgotten".
pub fn forget() -> Result<&'static str, String> {
    let path = cache_path();
    let _ = fs::remove_file(path.with_extension("part"));
    let status = match fs::remove_file(&path) {
        Ok(()) => "removed",
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "nothing_to_remove",
        Err(e) => return Err(format!("{e}")),
    };
    // Publish empty immediately so concurrent searches cannot resurrect the
    // old Arc while disk is already gone.
    publish(Arc::new(Corpus::empty()));
    Ok(status)
}

/// Cached corpus documents for the current process.
///
/// Re-reading 64 MB on every `search.query` would make the search field
/// unusable; this is why the source is a memory cache rather than a live call.
pub fn docs() -> Arc<Vec<RawDoc>> {
    Arc::clone(&loaded().docs)
}

/// Borrow docs + index under one load — prefer this on the search hot path.
pub fn with_docs_index<R>(f: impl FnOnce(&[RawDoc], &Index) -> R) -> R {
    let c = loaded();
    f(&c.docs, &c.index)
}

pub fn is_available() -> bool {
    cache_path().is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_is_the_published_corpus() {
        // Serialised: these tests mutate process env, which cargo's
        // parallel runner would otherwise leak between them.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_TSEARCH_URL") };
        assert_eq!(url(), DEFAULT_URL);
        assert!(url().starts_with("https://"), "corpus must be fetched over TLS");
    }

    #[test]
    fn url_is_overridable() {
        // Serialised: these tests mutate process env, which cargo's
        // parallel runner would otherwise leak between them.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_TSEARCH_URL", "https://example.test/c.json") };
        assert_eq!(url(), "https://example.test/c.json");
        unsafe { env::remove_var("OS_TSEARCH_URL") };
    }

    #[test]
    fn cache_lives_outside_the_repo() {
        // Serialised: these tests mutate process env, which cargo's
        // parallel runner would otherwise leak between them.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
        let p = cache_path().to_string_lossy().to_string();
        assert!(!p.contains("/os/search"), "cache must not land in the repo: {p}");
        assert!(p.ends_with(".json"));
    }

    #[test]
    fn missing_cache_yields_no_documents_not_a_panic() {
        // Serialised: these tests mutate process env, which cargo's
        // parallel runner would otherwise leak between them.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_TSEARCH_CACHE", "/nonexistent/os-teddy/none.json") };
        clear_memory();
        assert!(!is_available());
        assert!(docs().is_empty());
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
        clear_memory();
    }

    #[test]
    fn forget_deletes_disk_and_memory() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-teddy-forget-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("teddy.json");
        fs::write(
            &path,
            r#"{"crawled_at":"now","docs":[{"t":"Keep","u":"u","c":"web","b":"body","pr":0.5}]}"#,
        )
        .unwrap();
        unsafe { env::set_var("OS_TSEARCH_CACHE", &path) };
        clear_memory();
        assert_eq!(docs().len(), 1);
        assert!(!index().is_empty());
        assert_eq!(forget().unwrap(), "removed");
        assert!(!path.is_file());
        assert!(docs().is_empty());
        assert!(index().is_empty());
        assert_eq!(forget().unwrap(), "nothing_to_remove");
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
        clear_memory();
        let _ = fs::remove_dir_all(&dir);
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
        // Serialised: these tests mutate process env, which cargo's
        // parallel runner would otherwise leak between them.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let bad = r#"{"crawled_at":"now","docs":[{"t":"A","u":"x"#;
        assert!(serde_json::from_str::<CorpusFile>(bad).is_err());
    }
}

/// Inverted index over the cached corpus.
///
/// `search_tfidf` re-tokenises every document and rebuilds the df map per
/// query. That is fine for the ~16-document built-in corpus and hopeless for
/// 12k: measured at 5.6 s per search. The corpus only changes on sync/forget,
/// so tokenisation and idf are computed then and queries walk postings.
pub struct Index {
    /// Sorted by term, so lookup is a binary search.
    terms: Vec<Term>,
    postings: Vec<(usize, f64)>,
}

pub struct Term {
    word: String,
    idf: f64,
    start: usize,
    len: usize,
}

/// Tokeniser shared with the rest of the bridge.
fn tok(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_string())
        .collect()
}

pub fn index() -> Arc<Index> {
    Arc::clone(&loaded().index)
}

impl Index {
    fn build(docs: &[RawDoc]) -> Self {
        let n = docs.len() as f64;
        let mut acc: HashMap<String, HashMap<usize, f64>> = HashMap::new();
        let mut lens = vec![0usize; docs.len()];
        for (i, d) in docs.iter().enumerate() {
            let mut t = tok(&d.t);
            t.extend(tok(&d.t)); // title counts double, as in the client
            t.extend(tok(&d.b));
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
            let idf = ((n + 1.0) / (df + 1.0)).ln() + 1.0;
            let start = postings.len();
            let mut ids: Vec<usize> = per_doc.keys().copied().collect();
            ids.sort_unstable();
            for id in ids {
                postings.push((id, per_doc[&id] / lens[id] as f64));
            }
            terms.push(Term {
                word: w,
                idf,
                start,
                len: per_doc.len(),
            });
        }
        Self { terms, postings }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn term_count(&self) -> usize {
        self.terms.len()
    }

    fn find(&self, w: &str) -> Option<&Term> {
        self.terms
            .binary_search_by(|t| t.word.as_str().cmp(w))
            .ok()
            .map(|i| &self.terms[i])
    }

    /// Score `query`, returning `(score, doc_index)` best-first.
    ///
    /// Mirrors the built-in scorer: tf-idf blended with PageRank, plus the
    /// exact-AND bonus for documents carrying every query term.
    pub fn search(&self, docs: &[RawDoc], query: &str, k: usize) -> Vec<(f64, usize)> {
        let q = tok(query);
        if q.is_empty() || self.terms.is_empty() {
            return Vec::new();
        }
        let mut score: HashMap<usize, f64> = HashMap::new();
        let mut hits: HashMap<usize, usize> = HashMap::new();
        let mut seen = 0usize;
        for w in &q {
            let Some(t) = self.find(w) else { continue };
            seen += 1;
            for &(doc, tf) in &self.postings[t.start..t.start + t.len] {
                *score.entry(doc).or_insert(0.0) += tf * t.idf;
                *hits.entry(doc).or_insert(0) += 1;
            }
        }
        let mut out: Vec<(f64, usize)> = score
            .into_iter()
            .map(|(doc, mut s)| {
                let pr = docs.get(doc).map(|d| d.pr.clamp(0.0, 1.0)).unwrap_or(0.0);
                s *= 1.0 + 4.0 * pr;
                if seen > 0 && hits.get(&doc).copied().unwrap_or(0) >= seen {
                    s *= 1.35;
                }
                (s, doc)
            })
            .collect();
        out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(k);
        out
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;

    #[test]
    fn an_absent_corpus_indexes_to_empty() {
        // Serialised: these tests mutate process env, which cargo's
        // parallel runner would otherwise leak between them.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_TSEARCH_CACHE", "/nonexistent/os-teddy/none.json") };
        clear_memory();
        // Must not panic when there is nothing to index.
        assert!(docs().is_empty());
        assert!(index().is_empty());
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
        clear_memory();
    }

    #[test]
    fn tokeniser_matches_the_bridge_rules() {
        assert_eq!(
            tok("Capability-based Agents v2 a"),
            vec!["capability", "based", "agents", "v2"]
        );
    }
}
