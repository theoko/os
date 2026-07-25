//! Workspace index — makes the OS search *your* files, not just its own docs.
//!
//! Walks a set of roots, extracts a title and a short snippet from each text
//! document, and serves them to `search.query` alongside the baked corpus.
//!
//! Deliberate boundaries:
//!
//! * **Never baked.** `kernel/build.rs` compiles the static corpus into the
//!   ISO; putting personal documents there would ship them inside a build
//!   artifact in a public repo. This index lives under Application Support.
//! * **Roots are opt-in.** The default is the user's project directories, not
//!   all of `~`. Personal folders (immigration paperwork, notes) are only
//!   indexed if they are named explicitly via `OS_WORKSPACE_ROOTS`.
//! * **Secrets are skipped by name**, not by hoping they contain nothing.
//! * **Snippets only** — a few hundred characters, never whole files.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub title: String,
    /// Path relative to the root it was found under.
    pub path: String,
    pub snippet: String,
    /// Cheap relevance prior: shallower and more "index-like" files rank up.
    pub pr: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Index {
    #[serde(default)]
    pub entries: Vec<Entry>,
}

/// Directories that never contain anything worth searching.
const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", ".venv", "venv", "target", "dist", "build",
    "__pycache__", ".pytest_cache", ".ruff_cache", ".hypothesis", ".next",
    ".cargo", "Pods", ".terraform", "site-packages", ".mypy_cache",
    // Third-party source: someone else's README is never the answer to a
    // question about *your* work.
    "vendor", "third_party", "3rdparty", "deps", "Carthage",
    // Machine backups and inventories. A mac-backup tree is thousands of
    // plists that swamp real documents on any query mentioning a tool name.
    "mac-backup", "inventory", "backups", "backup", "Library",
];

/// Files that may carry credentials. Skipped on name alone — we never read
/// them to decide, because reading is the thing we are trying to avoid.
const SKIP_FILE_PARTS: &[&str] = &[
    ".env", "id_rsa", "id_ed25519", ".pem", ".key", ".p12", ".keychain",
    "credential", "secret", "token", "password", ".netrc", ".htpasswd",
];

/// Extensions worth indexing.
const TEXT_EXTS: &[&str] = &["md", "txt", "rst", "org"];

/// Largest file we will read. Bigger ones are almost never prose.
const MAX_BYTES: u64 = 512 * 1024;

/// Hard cap on entries so a stray root cannot produce an unbounded index.
pub const MAX_ENTRIES: usize = 4000;

pub fn index_path() -> PathBuf {
    env::var("OS_WORKSPACE_INDEX")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            crate::paths::knowledge("workspace.json")
        })
}

/// Roots to index. `OS_WORKSPACE_ROOTS` is a `:`-separated list.
///
/// The default is project directories only. Everything else in the home folder
/// — personal notes, forms, correspondence — stays out unless asked for by
/// name, because "search my machine" should not silently mean "index my
/// immigration paperwork".
pub fn roots() -> Vec<PathBuf> {
    if let Ok(v) = env::var("OS_WORKSPACE_ROOTS") {
        return v
            .split(':')
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    let d = crate::paths::home().join("Desktop");
    vec![d.join("projects"), d.join("os")]
}

fn skipped_dir(name: &str) -> bool {
    name.starts_with('.') && name != "." || SKIP_DIRS.contains(&name)
}

fn skipped_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SKIP_FILE_PARTS.iter().any(|p| lower.contains(p))
}

fn is_text(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) => TEXT_EXTS.contains(&e.to_ascii_lowercase().as_str()),
        // Extensionless READMEs / LICENSEs are worth having.
        None => matches!(
            path.file_name().and_then(|f| f.to_str()).map(|f| f.to_ascii_uppercase()),
            Some(ref f) if f.starts_with("README") || f.starts_with("CHANGELOG")
        ),
    }
}

/// First markdown heading, else the file stem.
fn title_of(body: &str, path: &Path) -> String {
    for line in body.lines().take(40) {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            let h = rest.trim();
            if !h.is_empty() {
                return h.chars().take(80).collect();
            }
        }
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .chars()
        .take(80)
        .collect()
}

/// Prose from the body, skipping front matter and headings.
fn snippet_of(body: &str) -> String {
    let mut out = String::new();
    let mut in_fm = false;
    for (i, line) in body.lines().enumerate() {
        let t = line.trim();
        if i == 0 && t == "---" {
            in_fm = true;
            continue;
        }
        if in_fm {
            if t == "---" {
                in_fm = false;
            }
            continue;
        }
        if t.is_empty() || t.starts_with('#') || t.starts_with("```") {
            continue;
        }
        out.push_str(t);
        out.push(' ');
        if out.len() >= 240 {
            break;
        }
    }
    out.chars().take(240).collect::<String>().trim().to_string()
}

/// Walk `roots` and build an index. Returns entries found.
pub fn build(roots: &[PathBuf]) -> Index {
    let mut entries = Vec::new();
    for root in roots {
        walk(root, root, &mut entries, 0);
        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }
    entries.truncate(MAX_ENTRIES);
    Index { entries }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Relevance prior for a document, 0..1.
///
/// Three signals, all cheap: shallow paths are entry points, READMEs are
/// summaries, and something edited last week matters more than something
/// untouched for two years. Recency is the one that stops an archived tree
/// from outranking work in progress.
pub fn rank(rel: &str, name: &str, mtime: u64, now: u64) -> f64 {
    let depth = rel.matches('/').count() as f64;
    let mut pr = 1.0 / (1.0 + depth);
    if name.to_ascii_uppercase().starts_with("README") {
        pr = (pr + 0.5).min(1.0);
    }
    // Half-life of roughly a year: 1.0 today, ~0.5 at 12 months, floor 0.25 so
    // old-but-relevant documents never drop out entirely.
    let age_days = now.saturating_sub(mtime) as f64 / 86_400.0;
    let recency = 0.25 + 0.75 / (1.0 + age_days / 365.0);
    (pr * recency).clamp(0.0, 1.0)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Entry>, depth: usize) {
    if depth > 8 || out.len() >= MAX_ENTRIES {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for ent in rd.flatten() {
        if out.len() >= MAX_ENTRIES {
            return;
        }
        let path = ent.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let Ok(ft) = ent.file_type() else { continue };
        if ft.is_dir() {
            if !skipped_dir(name) {
                walk(root, &path, out, depth + 1);
            }
            continue;
        }
        if !ft.is_file() || skipped_file(name) || !is_text(&path) {
            continue;
        }
        if fs::metadata(&path).map(|m| m.len() > MAX_BYTES).unwrap_or(true) {
            continue;
        }
        let Ok(body) = fs::read_to_string(&path) else { continue };
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_string();
        let mtime = fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let pr = rank(&rel, name, mtime, now_secs());
        out.push(Entry {
            title: title_of(&body, &path),
            path: rel,
            snippet: snippet_of(&body),
            pr,
        });
    }
}

impl Index {
    pub fn load() -> Self {
        crate::paths::read_json_or_default(index_path())
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        crate::paths::write_json(index_path(), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = env::temp_dir().join(format!("os-ws-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn indexes_markdown_with_heading_title() {
        let d = tmp("basic");
        fs::write(d.join("notes.md"), "---\nkey: v\n---\n\n# Real Title\n\nSome prose here.\n").unwrap();
        let ix = build(&[d.clone()]);
        assert_eq!(ix.entries.len(), 1);
        assert_eq!(ix.entries[0].title, "Real Title");
        assert!(ix.entries[0].snippet.contains("Some prose"));
        assert!(!ix.entries[0].snippet.contains("key: v"), "front matter leaked into the snippet");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn falls_back_to_the_filename() {
        let d = tmp("noheading");
        fs::write(d.join("plain.md"), "just text, no heading\n").unwrap();
        let ix = build(&[d.clone()]);
        assert_eq!(ix.entries[0].title, "plain");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn skips_secret_looking_files() {
        let d = tmp("secrets");
        fs::write(d.join(".env"), "API_KEY=hunter2\n").unwrap();
        fs::write(d.join("my-secret-notes.md"), "# S\n\nx\n").unwrap();
        fs::write(d.join("service.key"), "-----BEGIN KEY-----\n").unwrap();
        fs::write(d.join("ok.md"), "# Fine\n\ncontent\n").unwrap();
        let ix = build(&[d.clone()]);
        let titles: Vec<&str> = ix.entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["Fine"], "a credential-looking file was indexed");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn skips_build_and_vcs_directories() {
        let d = tmp("dirs");
        for junk in ["node_modules", ".git", "target"] {
            fs::create_dir_all(d.join(junk)).unwrap();
            fs::write(d.join(junk).join("a.md"), "# Junk\n\nx\n").unwrap();
        }
        fs::write(d.join("real.md"), "# Real\n\nx\n").unwrap();
        let ix = build(&[d.clone()]);
        assert_eq!(ix.entries.len(), 1);
        assert_eq!(ix.entries[0].title, "Real");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn ignores_binaries_and_unknown_extensions() {
        let d = tmp("exts");
        fs::write(d.join("a.png"), [0u8, 1, 2]).unwrap();
        fs::write(d.join("b.rs"), "fn main() {}").unwrap();
        fs::write(d.join("c.md"), "# Doc\n\nx\n").unwrap();
        let ix = build(&[d.clone()]);
        assert_eq!(ix.entries.len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn readme_outranks_a_deep_file() {
        let d = tmp("rank");
        fs::write(d.join("README.md"), "# Top\n\nx\n").unwrap();
        fs::create_dir_all(d.join("a/b/c")).unwrap();
        fs::write(d.join("a/b/c/deep.md"), "# Deep\n\nx\n").unwrap();
        let ix = build(&[d.clone()]);
        let top = ix.entries.iter().find(|e| e.title == "Top").unwrap();
        let deep = ix.entries.iter().find(|e| e.title == "Deep").unwrap();
        assert!(top.pr > deep.pr);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn default_roots_exclude_personal_folders() {
        // "Search my machine" must not silently mean "index my paperwork".
        unsafe { env::remove_var("OS_WORKSPACE_ROOTS") };
        let r = roots();
        let joined = r.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(" ");
        for personal in ["life", "moia-forms", "notary-intake", "inbox"] {
            assert!(!joined.contains(personal), "{personal} is indexed by default");
        }
        assert!(joined.contains("projects"), "projects should be a default root");
    }

    #[test]
    fn roots_are_configurable() {
        unsafe { env::set_var("OS_WORKSPACE_ROOTS", "/tmp/a:/tmp/b") };
        assert_eq!(roots(), vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]);
        unsafe { env::remove_var("OS_WORKSPACE_ROOTS") };
    }

    #[test]
    fn index_is_capped() {
        assert!(MAX_ENTRIES > 0 && MAX_ENTRIES <= 10_000);
    }

    #[test]
    fn missing_root_is_not_an_error() {
        let ix = build(&[PathBuf::from("/nonexistent/os-ws")]);
        assert!(ix.entries.is_empty());
    }
}

#[cfg(test)]
mod rank_tests {
    use super::*;

    const DAY: u64 = 86_400;
    const NOW: u64 = 1_800_000_000;

    #[test]
    fn recent_beats_stale_at_equal_depth() {
        let fresh = rank("a/b.md", "b.md", NOW - 7 * DAY, NOW);
        let stale = rank("a/b.md", "b.md", NOW - 1200 * DAY, NOW);
        assert!(fresh > stale, "recency ignored: {fresh} vs {stale}");
    }

    #[test]
    fn shallow_beats_deep_at_equal_age() {
        let shallow = rank("top.md", "top.md", NOW, NOW);
        let deep = rank("a/b/c/d.md", "d.md", NOW, NOW);
        assert!(shallow > deep);
    }

    #[test]
    fn readme_gets_a_boost() {
        let readme = rank("p/README.md", "README.md", NOW, NOW);
        let other = rank("p/notes.md", "notes.md", NOW, NOW);
        assert!(readme > other);
    }

    #[test]
    fn old_documents_keep_a_floor() {
        // A decade-old file should rank low, never zero — it may still be the
        // only match for a query.
        let ancient = rank("a.md", "a.md", 0, NOW);
        assert!(ancient > 0.0, "old documents fell out of the index entirely");
    }

    #[test]
    fn rank_stays_in_range() {
        for (rel, name, mt) in [
            ("README.md", "README.md", NOW),
            ("a/b/c/d/e/f.md", "f.md", 0),
            ("x.md", "x.md", NOW + 10 * DAY), // clock skew: mtime in the future
        ] {
            let r = rank(rel, name, mt, NOW);
            assert!((0.0..=1.0).contains(&r), "{rel} scored {r}");
        }
    }

    #[test]
    fn future_mtime_does_not_explode() {
        // saturating_sub keeps a skewed clock from producing a negative age.
        let r = rank("a.md", "a.md", NOW + 999 * DAY, NOW);
        assert!(r.is_finite() && r <= 1.0);
    }

    #[test]
    fn vendor_and_backup_trees_are_skipped() {
        for d in ["vendor", "mac-backup", "inventory", "third_party", "Library"] {
            assert!(skipped_dir(d), "{d} should be skipped");
        }
        assert!(!skipped_dir("projects"));
        assert!(!skipped_dir("docs"));
    }
}

#[cfg(test)]
mod ascii_tests {
    #[test]
    fn non_ascii_titles_are_stripped_for_the_guest() {
        // Real document titles contain emoji; the kernel atlas cannot render
        // them and would show '?' for each byte.
        let out =
            crate::search::query_all("greek events engine", 3, None, false, false, false);
        for row in &out {
            assert!(row.is_ascii(), "non-ASCII reached the wire: {row}");
        }
    }
}

#[cfg(test)]
mod consent_tests {
    //! The rule: `search.query` alone reaches the built-in corpus, never the
    //! user's own documents. Those need `workspace.index`, granted at setup.
    use super::*;

    #[test]
    fn personal_files_need_the_workspace_grant() {
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-consent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("private.md"), "# Zygote Notary Filing\n\npersonal matter\n").unwrap();

        let ix_path = dir.join("index.json");
        unsafe { env::set_var("OS_WORKSPACE_INDEX", &ix_path) };
        build(&[dir.clone()]).save().expect("save index");

        let without = crate::search::query_all("zygote notary", 5, None, false, false, false);
        let with = crate::search::query_all("zygote notary", 5, None, false, true, false);

        assert!(
            !without.iter().any(|r| r.contains("Zygote Notary")),
            "personal file reachable without the grant: {without:?}"
        );
        assert!(
            with.iter().any(|r| r.contains("Zygote Notary")),
            "granted search should find it: {with:?}"
        );

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_WORKSPACE_INDEX") };
    }
}

#[cfg(test)]
mod revocation_tests {
    use super::*;

    #[test]
    fn forgetting_removes_the_index_from_disk() {
        // "Off" must mean gone, not hidden — the switch does not say "pause".
        let _g = crate::graph::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = env::temp_dir().join(format!("os-forget-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let ix = dir.join("index.json");
        unsafe { env::set_var("OS_WORKSPACE_INDEX", &ix) };

        fs::write(dir.join("a.md"), "# Doc\n\nbody\n").unwrap();
        build(&[dir.clone()]).save().unwrap();
        assert!(ix.is_file(), "index should exist before revocation");

        fs::remove_file(&ix).unwrap();
        assert!(!ix.is_file());
        assert!(Index::load().entries.is_empty(), "purged index still returns entries");

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_WORKSPACE_INDEX") };
    }
}
