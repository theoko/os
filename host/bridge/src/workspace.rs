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
        .unwrap_or_else(|_| home().join("Library/Application Support/os/knowledge/workspace.json"))
}

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
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
    let d = home().join("Desktop");
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
        // Shallow files and READMEs are usually the entry points people want.
        let d = rel.matches('/').count() as f64;
        let mut pr = 1.0 / (1.0 + d);
        if name.to_ascii_uppercase().starts_with("README") {
            pr = (pr + 0.5).min(1.0);
        }
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
        match fs::read_to_string(index_path()) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let path = index_path();
        if let Some(d) = path.parent() {
            fs::create_dir_all(d).map_err(|e| format!("mkdir {}: {e}", d.display()))?;
        }
        let raw = serde_json::to_string(self).map_err(|e| e.to_string())?;
        fs::write(&path, raw).map_err(|e| format!("write {}: {e}", path.display()))?;
        Ok(path)
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
