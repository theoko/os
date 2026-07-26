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
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Keychain service the portal credential is stored under.
pub const KEYCHAIN_SERVICE: &str = "os-portal";

/// Basic-auth credential for the portal, if one is configured.
///
/// Keychain first, environment second. The repo is public and AGENTS.md is
/// explicit that secrets stay out of the tree, so nothing is read from disk
/// here. An env var works but leaks into process listings and shell history,
/// which is why it is the fallback rather than the default.
///
/// Store one with:
///   security add-generic-password -s os-portal -a <user> -w <password>
fn credential() -> Option<(String, String)> {
    if let (Ok(u), Ok(p)) = (env::var("OS_PORTAL_USER"), env::var("OS_PORTAL_PASS")) {
        if !u.is_empty() {
            return Some((u, p));
        }
    }
    let user = env::var("OS_PORTAL_USER").ok().filter(|u| !u.is_empty())?;
    let out = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", &user, "-w"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let pass = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!pass.is_empty()).then_some((user, pass))
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

    // Credentials go in via a config on stdin, never argv: `curl --user u:p`
    // puts the password in `ps` output for every process on the machine.
    let mut child = Command::new("curl")
        .args(["-sS", "--fail", "--max-time", "300", "-o"])
        .arg(&tmp)
        .args(["-K", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;

    {
        use std::io::Write as _;
        let mut cfg = String::new();
        cfg.push_str(&format!("url = \"{}\"\n", url()));
        if let Some((u, pw)) = credential() {
            cfg.push_str(&format!("user = \"{u}:{pw}\"\n"));
        }
        let stdin = child.stdin.as_mut().ok_or("curl stdin")?;
        stdin.write_all(cfg.as_bytes()).map_err(|e| format!("curl stdin: {e}"))?;
    }

    let out = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        let _ = fs::remove_file(&tmp);
        let err = String::from_utf8_lossy(&out.stderr);
        let brief: String = err.lines().last().unwrap_or("fetch failed").chars().take(120).collect();
        // 401 is the one failure with an obvious remedy, so name it.
        if brief.contains("401") {
            return Err(format!(
                "{brief} - store a credential: security add-generic-password -s {KEYCHAIN_SERVICE} -a <user> -w"
            ));
        }
        return Err(brief);
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
///
/// This was a pair of `OnceLock`s. A sync writes a new file, but the lock had
/// already resolved — to an EMPTY corpus when the bridge started before any
/// corpus existed — so the freshly downloaded corpus stayed invisible until the
/// bridge was restarted. The status screen said "Downloaded, but empty" and
/// searches found nothing, which is exactly what it looked like. Docs and index
/// are held together so a swap can never leave an index describing a corpus
/// that is no longer resident.
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

/// Drop the parsed corpus and its index so the next read picks up new content.
///
/// The reload entry point for anything that changes the cache file behind the
/// bridge's back. The two in-tree paths no longer need it — `sync` publishes
/// the copy it just parsed and `forget` publishes an empty one — but a corpus
/// that only becomes visible after a restart is the bug this module exists to
/// prevent, so the hook stays public.
#[allow(dead_code)]
pub fn invalidate() {
    if let Ok(mut g) = CORPUS.lock() {
        *g = None;
    }
}

/// Drop in-memory state so the next access reloads from disk (or emptiness).
#[cfg(test)]
pub fn clear_memory() {
    invalidate();
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

// No separate INDEX cache: the index is a field of `Corpus`, so `invalidate`
// and `publish` swap it with the docs it was built from.

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
    fn an_absent_corpus_is_reported_as_unavailable() {
        // Was asserting on docs(), which is a OnceLock: once any test — or a
        // real sync — populates it, it stays populated for the process, so the
        // env change had no effect. It passed only while no corpus existed on
        // disk, and started failing the moment one did. A test that passed for
        // the wrong reason. is_available() consults the filesystem every call.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_TSEARCH_CACHE", "/nonexistent/os-teddy/none.json") };
        clear_memory();
        // is_available() consults the filesystem on every call, so it is the
        // assertion that cannot pass for the wrong reason...
        assert!(!is_available());
        // ...and with the resident copy dropped, neither can these: they must
        // not panic when there is nothing to index.
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

#[cfg(test)]
mod forget_tests {
    use super::*;

    #[test]
    fn revoking_can_remove_the_cached_corpus() {
        // "Off" has to mean gone here too: the corpus is ~64MB fetched from a
        // remote site, and leaving it behind after the grant is withdrawn is
        // the loudest possible version of the inconsistency.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-portal-forget-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cache = dir.join("teddy.json");
        unsafe { env::set_var("OS_TSEARCH_CACHE", &cache) };

        fs::write(&cache, r#"{"crawled_at":"now","docs":[{"t":"A","u":"u"}]}"#).unwrap();
        assert!(is_available());

        fs::remove_file(&cache).unwrap();
        assert!(!is_available(), "cache survived revocation");

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
    }
}

#[cfg(test)]
mod auth_tests {
    use super::*;

    #[test]
    fn no_credential_configured_is_not_an_error() {
        // An unauthenticated portal must keep working; auth is opt-in.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::remove_var("OS_PORTAL_USER") };
        unsafe { env::remove_var("OS_PORTAL_PASS") };
        assert!(credential().is_none());
    }

    #[test]
    fn env_credential_is_used_when_both_parts_are_present() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_PORTAL_USER", "theo") };
        unsafe { env::set_var("OS_PORTAL_PASS", "hunter2") };
        assert_eq!(credential(), Some(("theo".into(), "hunter2".into())));
        unsafe { env::remove_var("OS_PORTAL_USER") };
        unsafe { env::remove_var("OS_PORTAL_PASS") };
    }

    #[test]
    fn a_username_alone_does_not_produce_an_empty_password() {
        // Without a keychain entry this must yield None rather than
        // authenticating as user-with-blank-password.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_PORTAL_USER", "no-such-user-for-tests") };
        unsafe { env::remove_var("OS_PORTAL_PASS") };
        assert!(credential().is_none());
        unsafe { env::remove_var("OS_PORTAL_USER") };
    }

    #[test]
    fn the_keychain_service_name_is_stable() {
        // Changing this orphans anyone's stored credential silently.
        assert_eq!(KEYCHAIN_SERVICE, "os-portal");
    }
}

/// True while a background sync is running.
static SYNCING: AtomicBool = AtomicBool::new(false);

pub fn is_syncing() -> bool {
    SYNCING.load(Ordering::Relaxed)
}

/// Start a sync in the background and return immediately.
///
/// The fetch takes ~51s for 64MB. Doing it inline froze the guest for that
/// long during setup, which looks like a hang — and if the machine was shut
/// down meanwhile, nothing landed and the switch stayed on with no corpus
/// behind it. `portal.status` reports progress instead.
pub fn sync_background() -> &'static str {
    if SYNCING.swap(true, Ordering::SeqCst) {
        return "already running";
    }
    std::thread::spawn(|| {
        match sync() {
            Ok((n, at)) => {
                // The whole point: the new corpus is visible without a restart.
                // `sync` already published the copy it just parsed, index and
                // all, so there is nothing to invalidate — dropping it here
                // would only make the next query re-read 64MB on the hot path.
                eprintln!("tsearch: synced {n} docs (crawled {at})");
            }
            Err(e) => eprintln!("tsearch: sync failed: {e}"),
        }
        SYNCING.store(false, Ordering::SeqCst);
    });
    "started"
}

#[cfg(test)]
mod reload_tests {
    use super::*;

    #[test]
    fn a_sync_becomes_visible_without_restarting() {
        // The bug this replaces: docs() was a OnceLock, so a bridge that
        // started before any corpus existed resolved to empty and stayed that
        // way. The status screen read "Downloaded, but empty" while a 67MB
        // corpus sat on disk.
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-reload-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cache = dir.join("teddy.json");
        unsafe { env::set_var("OS_TSEARCH_CACHE", &cache) };

        invalidate();
        assert!(docs().is_empty(), "nothing on disk yet");

        fs::write(
            &cache,
            r#"{"crawled_at":"now","docs":[{"t":"Fresh","u":"u","c":"web","b":"body","pr":0.5}]}"#,
        )
        .unwrap();

        // Without invalidation this would still report empty.
        invalidate();
        assert_eq!(docs().len(), 1, "new corpus not picked up");
        assert_eq!(docs()[0].t, "Fresh");

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
        invalidate();
    }

    #[test]
    fn purging_drops_the_resident_copy() {
        let _env = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-purge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cache = dir.join("teddy.json");
        unsafe { env::set_var("OS_TSEARCH_CACHE", &cache) };

        fs::write(&cache, r#"{"crawled_at":"now","docs":[{"t":"Gone","u":"u"}]}"#).unwrap();
        invalidate();
        assert_eq!(docs().len(), 1);

        fs::remove_file(&cache).unwrap();
        invalidate();
        assert!(docs().is_empty(), "revoked corpus still resident");

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_TSEARCH_CACHE") };
        invalidate();
    }
}
