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
    /// The opening of the document, as `bake-corpus.py` stored it.
    ///
    /// This is what makes an offline row openable. An index alone can only say
    /// that a document exists, which is why every result on this device used
    /// to dead-end at "cannot open documents": the machine knew the title and
    /// had nothing to show. Empty for a document baked without one.
    pub body: &'static str,
    /// PageRank, 0..1024.
    pub pr_q10: i32,
    /// Token count, for length normalisation.
    pub n_tokens: usize,
}

/// A vocabulary entry pointing into [`POSTINGS`].
pub struct Term {
    pub word: &'static str,
    /// Inverse document frequency, Q16.
    pub idf_q16: i64,
    pub start: usize,
    pub len: usize,
}

/// One (document, weight) pair for a term.
pub struct Posting {
    pub doc: usize,
    /// Length-normalised term frequency, Q16.
    pub tf_q16: i64,
}

include!(concat!(env!("OUT_DIR"), "/corpus.rs"));

/// Widest title a result row can hold (`Row::title` in `searchui.rs`).
///
/// Titles are cut to fit by `scripts/bake-corpus.py`, deliberately and at a
/// word boundary, then made unique again — because `copy_into` would
/// otherwise cut them here, in silence, and the Brief de-duplicates rows by
/// their text.
pub const TITLE_SLOT: usize = 56;

/// Widest URL a result row can hold (`Row::url` in `searchui.rs`).
///
/// Nothing may ever be truncated into this one. A cut path is a different
/// document, and two paths that agree for the first N bytes would collapse
/// into a single row — one real document silently erased from every answer.
/// The slot is sized to the corpus rather than the corpus to the slot; if a
/// bake pushes past it, the const assert below fails the build.
pub const URL_SLOT: usize = 128;

const _: () = assert!(
    BAKED_TITLE_MAX <= TITLE_SLOT,
    "a baked title is wider than Row::title — it would be truncated on screen"
);
const _: () = assert!(
    BAKED_URL_MAX <= URL_SLOT,
    "a baked URL is wider than Row::url — distinct documents would collapse"
);

/// Max hits returned. Matches the bridge's home-screen peek.
/// Rows a result screen can hold.
///
/// This is also the cap the bridge is asked to respect. It was 3 while the
/// bridge returned 5, so the agent's sentence said "5 matches" above three
/// visible rows - the count was true of the search and false of the screen.
pub const MAX_HITS: usize = 5;

#[derive(Clone, Copy)]
pub struct Hit {
    pub doc: usize,
    pub score: i64,
}

/// Tokenizer. Must stay identical to `fold_tok` in `build.rs`, or query terms
/// will not match the baked vocabulary.
///
/// Yields lowercased ASCII-alphanumeric runs of 2+ chars.
pub struct Tokens<'a> {
    rest: &'a str,
}

pub fn tokenize(s: &str) -> Tokens<'_> {
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

/// The extract this image stores for `url`, if it stores one.
///
/// The single source of truth for "does this row open?". Rows are drawn
/// openable from this answer and the reader fills from it, so a row can never
/// promise a document the image does not hold.
pub fn body_for(url: &str) -> Option<&'static str> {
    if url.is_empty() {
        return None;
    }
    DOCS.iter()
        .find(|d| d.url == url)
        .map(|d| d.body)
        .filter(|b| !b.is_empty())
}

/// Rank documents for `query`. Returns hits in descending score order.
///
/// Mirrors the host scorer: tf-idf, blended with PageRank, with a bonus for
/// documents containing every query term.
pub fn query(q: &str, out: &mut [Hit; MAX_HITS]) -> usize {
    let mut scores = [0i64; N_DOCS];
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

    let mut n = 0usize;
    let mut ranked = [Hit { doc: 0, score: 0 }; N_DOCS];
    for (i, &raw) in scores.iter().enumerate() {
        if raw <= 0 {
            continue;
        }
        // PageRank blend: score * (1 + 4*pr), pr in 0..1.
        let mut score = raw * (1024 + 4 * DOCS[i].pr_q10 as i64) / 1024;
        // Exact-AND ladder: prefer documents carrying every query term.
        if n_terms > 0 && hit_count[i] >= n_terms {
            score = score * 135 / 100;
        }
        ranked[n] = Hit { doc: i, score };
        n += 1;
    }

    // Selection sort: N_DOCS is tiny and this avoids needing alloc.
    for i in 0..n.min(MAX_HITS) {
        let mut best = i;
        for j in (i + 1)..n {
            if ranked[j].score > ranked[best].score {
                best = j;
            }
        }
        ranked.swap(i, best);
        out[i] = ranked[i];
    }
    n.min(MAX_HITS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(q: &str) -> Option<&'static str> {
        let mut out = [Hit { doc: 0, score: 0 }; MAX_HITS];
        let n = query(q, &mut out);
        (n > 0).then(|| DOCS[out[0].doc].title)
    }

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
        let hit = top("capability ambient root");
        assert!(hit.is_some(), "expected a hit for a corpus phrase");
    }

    #[test]
    fn unknown_terms_return_nothing() {
        let mut out = [Hit { doc: 0, score: 0 }; MAX_HITS];
        assert_eq!(query("zzzz qqqq wwww", &mut out), 0);
    }

    #[test]
    fn empty_query_returns_nothing() {
        let mut out = [Hit { doc: 0, score: 0 }; MAX_HITS];
        assert_eq!(query("", &mut out), 0);
    }

    #[test]
    fn results_are_descending() {
        let mut out = [Hit { doc: 0, score: 0 }; MAX_HITS];
        let n = query("os agent search skills", &mut out);
        for i in 1..n {
            assert!(out[i - 1].score >= out[i].score, "not sorted at {i}");
        }
    }

    #[test]
    fn never_returns_more_than_max_hits() {
        let mut out = [Hit { doc: 0, score: 0 }; MAX_HITS];
        // A term-heavy query that touches most of the corpus.
        let n = query(
            "os agent kernel search skills bridge capability docs",
            &mut out,
        );
        assert!(n <= MAX_HITS);
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let mut a = [Hit { doc: 0, score: 0 }; MAX_HITS];
        let mut b = [Hit { doc: 0, score: 0 }; MAX_HITS];
        let na = query("CAPABILITY", &mut a);
        let nb = query("capability", &mut b);
        assert_eq!(na, nb);
        if na > 0 {
            assert_eq!(a[0].doc, b[0].doc);
        }
    }

    #[test]
    fn scoring_uses_no_floats() {
        // Guard the invariant that matters on a kernel with no FPU: every
        // stored weight is an integer type.
        let _: i64 = TERMS[0].idf_q16;
        let _: i64 = POSTINGS[0].tf_q16;
        let _: i32 = DOCS[0].pr_q10;
    }
}
