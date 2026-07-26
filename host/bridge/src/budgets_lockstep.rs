//! Guest buffer / peek budgets must match the kernel twins.
//!
//! Not a shared crate yet — this panel fails `make test-host` if either side
//! drifts. Paths are resolved from `CARGO_MANIFEST_DIR` (cwd-safe).

use std::fs;
use std::path::PathBuf;

use super::{GUEST_DOC_LINES, GUEST_MAIL_MAX, GUEST_MAX_HITS};
use crate::search::{CAT_CHARS, LINE_CHARS, TITLE_CHARS, URL_CHARS};
use crate::skills::{DESC_CHARS, GUEST_MAX_LISTED, NAME_CHARS};

fn kernel_src(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../kernel/src")
        .join(file)
}

/// First `const NAME: usize = N;` (optional `pub` / `pub(crate)`).
fn usize_const(src: &str, name: &str) -> usize {
    let needle = format!("const {name}: usize = ");
    let mut hits = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        let Some(rest) = t
            .strip_prefix("pub(crate) ")
            .or_else(|| t.strip_prefix("pub "))
            .unwrap_or(t)
            .strip_prefix(&needle)
        else {
            continue;
        };
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        hits.push(digits.parse::<usize>().unwrap_or_else(|_| {
            panic!("bad usize for {name} in line: {line}");
        }));
    }
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one `const {name}: usize` in kernel source, found {}",
        hits.len()
    );
    hits[0]
}

#[test]
fn guest_budgets_match_kernel() {
    let search = fs::read_to_string(kernel_src("search.rs")).expect("kernel search.rs");
    let skills = fs::read_to_string(kernel_src("skills.rs")).expect("kernel skills.rs");
    let mcp = fs::read_to_string(kernel_src("mcp.rs")).expect("kernel mcp.rs");

    assert_eq!(GUEST_MAX_HITS, usize_const(&search, "MAX_HITS"));
    assert_eq!(GUEST_MAX_HITS, 3);
    assert_eq!(GUEST_DOC_LINES, usize_const(&mcp, "MAX"));
    assert_eq!(GUEST_DOC_LINES, 18);
    assert_eq!(GUEST_MAIL_MAX, usize_const(&mcp, "MAIL_PEEK_MAX"));
    assert_eq!(GUEST_MAIL_MAX, 3);
    assert_eq!(GUEST_MAX_LISTED, usize_const(&skills, "MAX_LISTED"));
    assert_eq!(GUEST_MAX_LISTED, 6);

    assert_eq!(TITLE_CHARS, usize_const(&search, "TITLE_CHARS"));
    assert_eq!(URL_CHARS, usize_const(&search, "URL_CHARS"));
    assert_eq!(CAT_CHARS, usize_const(&search, "CAT_CHARS"));
    assert_eq!(LINE_CHARS, usize_const(&search, "LINE_CHARS"));
    assert_eq!(NAME_CHARS, usize_const(&skills, "NAME_CHARS"));
    assert_eq!(DESC_CHARS, usize_const(&skills, "DESC_CHARS"));
}
