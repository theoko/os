//! Offline knowledge search, baked into the kernel.
//!
//! `search.query` normally goes to the host bridge over COM2. When the bridge
//! isn't there — which is the common case, since it has to be started by hand —
//! the guest still answers from a static index built from `search/corpus.json`
//! at compile time (see `build_corpus` in `build.rs`).
//!
//! This is the only connector that can legitimately live here: it needs no
//! network, no credentials and no host state. Email stays on the bridge, where
//! its OAuth tokens belong.
//!
//! All weights are precomputed as fixed point. The kernel never enables the
//! FPU, so there is no `ln()` and no float division at runtime — scoring is
//! pure integer arithmetic.

/// One indexed document.
pub struct Doc {
    pub title: &'static str,
    pub cat: &'static str,
    pub url: &'static str,
    /// PageRank, 0..1024.
    pr_q10: i32,
}

/// A vocabulary entry pointing into [`POSTINGS`].
struct Term {
    word: &'static str,
    /// Inverse document frequency, Q16.
    idf_q16: i64,
    start: usize,
    len: usize,
}

/// One (document, weight) pair for a term.
struct Posting {
    doc: usize,
    /// Length-normalised term frequency, Q16.
    tf_q16: i64,
}

include!(concat!(env!("OUT_DIR"), "/corpus.rs"));

/// Cap on search hits (offline index, SearchView rows, and guest `k=`).
pub const MAX_HITS: usize = 3;

/// Tokenizer. Must stay identical to `fold_tok` in `build.rs`, or query terms
/// will not match the baked vocabulary.
///
/// Yields lowercased ASCII-alphanumeric runs of 2+ chars.
struct Tokens<'a> {
    rest: &'a str,
}

fn tokenize(s: &str) -> Tokens<'_> {
    Tokens { rest: s }
}

impl<'a> Iterator for Tokens<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let bytes = self.rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() && !bytes[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
            i += 1;
        }
        if start == i {
            self.rest = "";
            return None;
        }
        let tok = &self.rest[start..i];
        self.rest = &self.rest[i..];
        if tok.len() < 2 {
            return self.next();
        }
        Some(tok)
    }
}

/// Case-insensitive ASCII compare against a lowercase vocabulary word.
fn cmp_lower(query: &str, word: &str) -> core::cmp::Ordering {
    let (a, b) = (query.as_bytes(), word.as_bytes());
    let n = a.len().min(b.len());
    for i in 0..n {
        let ca = a[i].to_ascii_lowercase();
        match ca.cmp(&b[i]) {
            core::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    a.len().cmp(&b.len())
}

/// Look up a query token in the sorted vocabulary.
fn find_term(tok: &str) -> Option<&'static Term> {
    let (mut lo, mut hi) = (0usize, TERMS.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        match cmp_lower(tok, TERMS[mid].word) {
            core::cmp::Ordering::Equal => return Some(&TERMS[mid]),
            core::cmp::Ordering::Less => hi = mid,
            core::cmp::Ordering::Greater => lo = mid + 1,
        }
    }
    None
}

/// Rank documents for `query`. Writes up to [`MAX_HITS`] document indices into
/// `out` in descending score order. Returns how many were written.
///
/// Mirrors the host scorer: tf-idf, blended with PageRank, with a bonus for
/// documents containing every query term. Scores stay local.
pub fn query(q: &str, out: &mut [usize; MAX_HITS]) -> usize {
    let mut scores = [0i64; N_DOCS];
    score_docs(q, &mut scores);
    pick_top(&mut scores, out)
}

/// Accumulate tf-idf, blend PageRank / exact-AND, leave non-hits at 0.
fn score_docs(q: &str, scores: &mut [i64; N_DOCS]) {
    let mut hit_count = [0u32; N_DOCS];
    let mut n_terms = 0u32;

    for tok in tokenize(q) {
        n_terms += 1;
        let Some(term) = find_term(tok) else { continue };
        for p in &POSTINGS[term.start..term.start + term.len] {
            // (tf/len) * idf, both Q16 -> Q16 after the shift.
            scores[p.doc] += (p.tf_q16 * term.idf_q16) >> 16;
            hit_count[p.doc] += 1;
        }
    }

    for (i, slot) in scores.iter_mut().enumerate() {
        let raw = *slot;
        if raw <= 0 {
            *slot = 0;
            continue;
        }
        // PageRank blend: score * (1 + 4*pr), pr in 0..1.
        let mut score = raw * (1024 + 4 * DOCS[i].pr_q10 as i64) / 1024;
        // Exact-AND ladder: prefer documents carrying every query term.
        if n_terms > 0 && hit_count[i] >= n_terms {
            score = score * 135 / 100;
        }
        *slot = score;
    }
}

/// Selection-sort the top [`MAX_HITS`] into `out`, clearing taken scores.
fn pick_top(scores: &mut [i64; N_DOCS], out: &mut [usize; MAX_HITS]) -> usize {
    let mut n = 0usize;
    for slot in out.iter_mut() {
        let mut best = None;
        for (i, &s) in scores.iter().enumerate() {
            if s <= 0 {
                continue;
            }
            if best.map_or(true, |b| s > scores[b]) {
                best = Some(i);
            }
        }
        let Some(i) = best else {
            break;
        };
        *slot = i;
        scores[i] = 0;
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_is_populated() {
        assert!(N_DOCS > 0, "corpus baked empty");
        assert!(N_TERMS > 0);
        assert!(N_POSTINGS >= N_TERMS);
    }

    #[test]
    fn vocabulary_is_sorted_for_binary_search() {
        for w in TERMS.windows(2) {
            assert!(w[0].word < w[1].word, "{} !< {}", w[0].word, w[1].word);
        }
    }

    #[test]
    fn postings_point_inside_the_table() {
        for t in TERMS.iter() {
            assert!(t.start + t.len <= N_POSTINGS);
            for p in &POSTINGS[t.start..t.start + t.len] {
                assert!(p.doc < N_DOCS);
            }
        }
    }

    #[test]
    fn tokenizer_matches_build_rules() {
        // Punctuation splits, 1-char runs are dropped, digits are kept.
        let toks: Vec<&str> = tokenize("Capability-based Agents, v2 a").collect();
        assert_eq!(toks, vec!["Capability", "based", "Agents", "v2"]);
    }

    #[test]
    fn tokenizer_terminates_on_trailing_punctuation() {
        let toks: Vec<&str> = tokenize("os...").collect();
        assert_eq!(toks, vec!["os"]);
    }

    #[test]
    fn finds_a_relevant_document() {
        let mut out = [0usize; MAX_HITS];
        let n = query("capability ambient root", &mut out);
        assert!(n > 0, "expected a hit for a corpus phrase");
    }

    #[test]
    fn empty_or_unknown_queries_return_nothing() {
        for q in ["", "zzzz qqqq wwww"] {
            let mut out = [0usize; MAX_HITS];
            assert_eq!(query(q, &mut out), 0, "q={q:?}");
        }
    }

    #[test]
    fn results_are_descending() {
        let mut scores = [0i64; N_DOCS];
        score_docs("os agent search skills", &mut scores);
        let snapshot = scores;
        let mut out = [0usize; MAX_HITS];
        let n = pick_top(&mut scores, &mut out);
        for i in 1..n {
            assert!(
                snapshot[out[i - 1]] >= snapshot[out[i]],
                "not sorted at {i}"
            );
        }
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let mut a = [0usize; MAX_HITS];
        let mut b = [0usize; MAX_HITS];
        let na = query("CAPABILITY", &mut a);
        let nb = query("capability", &mut b);
        assert_eq!(na, nb);
        if na > 0 {
            assert_eq!(a[0], b[0]);
        }
    }
}
