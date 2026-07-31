//! Offline knowledge search over a baked corpus.
//!
//! On the standalone tip there is no host bridge: the guest answers from a
//! static index built from `search/corpus.json` at compile time (see
//! `build_corpus` in `build.rs`). MCP `main` may still route `search.query`
//! through COM2 when a bridge is Online; this crate always ranks the bake.
//!
//! std port of `kernel/src/search.rs`. The kernel stored every weight as Q16 /
//! Q10 fixed point because it never enables the FPU; here the same quantities
//! are `f64`. That was checked, not assumed: scoring both ways over the whole
//! baked vocabulary and several thousand generated multi-term queries produced
//! identical result orderings. The *sort* is a different story — see
//! [`take_top`].

/// One indexed document.
pub struct Doc {
    pub title: &'static str,
    pub cat: &'static str,
    pub url: &'static str,
    /// PageRank, 0..1.
    pub pr: f64,
    /// Token count, for length normalisation.
    pub n_tokens: usize,
}

/// A vocabulary entry pointing into [`POSTINGS`].
pub struct Term {
    pub word: &'static str,
    /// Inverse document frequency.
    pub idf: f64,
    pub start: usize,
    pub len: usize,
}

/// One (document, weight) pair for a term.
pub struct Posting {
    pub doc: usize,
    /// Length-normalised term frequency.
    pub tf: f64,
}

include!(concat!(env!("OUT_DIR"), "/corpus.rs"));

/// Max hits returned. Matches the bridge's home-screen peek.
///
/// This is also the cap the bridge is asked to respect. It was 3 while the
/// bridge returned 5, so the agent's sentence said "5 matches" above three
/// visible rows — the count was true of the search and false of the screen.
pub const MAX_HITS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub doc: usize,
    pub score: f64,
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
fn cmp_lower(query: &str, word: &str) -> std::cmp::Ordering {
    let (a, b) = (query.as_bytes(), word.as_bytes());
    let n = a.len().min(b.len());
    for i in 0..n {
        let ca = a[i].to_ascii_lowercase();
        match ca.cmp(&b[i]) {
            std::cmp::Ordering::Equal => {}
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
            std::cmp::Ordering::Equal => return Some(&TERMS[mid]),
            std::cmp::Ordering::Less => hi = mid,
            std::cmp::Ordering::Greater => lo = mid + 1,
        }
    }
    None
}

/// Take the best [`MAX_HITS`] of `ranked`, highest score first.
///
/// This stays a selection sort even though `sort_by` is now available. The
/// kernel comment called it an alloc workaround, but it is also a tie-break
/// rule: replacing it with a stable sort reorders equal-scoring documents on
/// ~0.3% of queries against the current corpus. Equal scores are common here
/// (16 documents, short bodies), so that is a visible change in results, not
/// an implementation detail.
fn take_top(ranked: &mut [Hit]) -> Vec<Hit> {
    let n = ranked.len();
    let mut out = Vec::with_capacity(n.min(MAX_HITS));
    for i in 0..n.min(MAX_HITS) {
        let mut best = i;
        for j in (i + 1)..n {
            if ranked[j].score > ranked[best].score {
                best = j;
            }
        }
        ranked.swap(i, best);
        out.push(ranked[i]);
    }
    out
}

/// Rank documents for `query`. Returns hits in descending score order.
///
/// Mirrors the host scorer: tf-idf, blended with PageRank, with a bonus for
/// documents containing every query term.
pub fn query(q: &str) -> Vec<Hit> {
    let mut scores = vec![0.0f64; DOCS.len()];
    let mut hit_count = vec![0u32; DOCS.len()];
    let mut n_terms = 0u32;

    for tok in tokenize(q) {
        n_terms += 1;
        let Some(term) = find_term(tok) else { continue };
        for p in &POSTINGS[term.start..term.start + term.len] {
            scores[p.doc] += p.tf * term.idf;
            hit_count[p.doc] += 1;
        }
    }

    let mut ranked: Vec<Hit> = Vec::new();
    for (i, &raw) in scores.iter().enumerate() {
        if raw <= 0.0 {
            continue;
        }
        // PageRank blend: score * (1 + 4*pr), pr in 0..1.
        let mut score = raw * (1.0 + 4.0 * DOCS[i].pr);
        // Exact-AND ladder: prefer documents carrying every query term.
        if n_terms > 0 && hit_count[i] >= n_terms {
            score *= 1.35;
        }
        ranked.push(Hit { doc: i, score });
    }

    take_top(&mut ranked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(q: &str) -> Option<&'static str> {
        query(q).first().map(|h| DOCS[h.doc].title)
    }

    #[test]
    fn index_is_populated() {
        assert!(!DOCS.is_empty(), "corpus baked empty");
        assert!(!TERMS.is_empty(), "vocabulary baked empty");
        // Every term owns at least one posting, or the bake dropped documents.
        assert!(POSTINGS.len() >= TERMS.len());
        assert_eq!(
            (DOCS.len(), TERMS.len(), POSTINGS.len()),
            (N_DOCS, N_TERMS, N_POSTINGS)
        );
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
        assert!(query("zzzz qqqq wwww").is_empty());
    }

    #[test]
    fn empty_query_returns_nothing() {
        assert!(query("").is_empty());
    }

    #[test]
    fn results_are_descending() {
        let hits = query("os agent search skills");
        for i in 1..hits.len() {
            assert!(hits[i - 1].score >= hits[i].score, "not sorted at {i}");
        }
    }

    #[test]
    fn never_returns_more_than_max_hits() {
        // A term-heavy query that touches most of the corpus.
        let hits = query("os agent kernel search skills bridge capability docs");
        assert!(
            hits.len() > MAX_HITS - 1,
            "query matched too little to test"
        );
        assert!(hits.len() <= MAX_HITS);
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let a = query("CAPABILITY");
        let b = query("capability");
        assert_eq!(a.len(), b.len());
        if let (Some(x), Some(y)) = (a.first(), b.first()) {
            assert_eq!(x.doc, y.doc);
        }
    }

    #[test]
    fn baked_weights_are_finite_and_positive() {
        // Replaces the kernel's `scoring_uses_no_floats`, which guarded an
        // invariant (integer weights) that this port deliberately drops. What
        // still has to hold is that the bake produced usable numbers: a
        // zero-length document or an empty vocabulary entry would give an
        // infinite or NaN weight and silently poison every ranking.
        for t in TERMS.iter() {
            assert!(t.idf.is_finite() && t.idf > 0.0, "idf for {}", t.word);
        }
        for p in POSTINGS.iter() {
            assert!(p.tf.is_finite(), "tf for doc {}", p.doc);
            assert!(p.tf > 0.0 && p.tf <= 1.0, "tf out of range: {}", p.tf);
        }
        for d in DOCS.iter() {
            assert!((0.0..=1.0).contains(&d.pr), "pr for {}", d.title);
            assert!(d.n_tokens > 0, "{} baked with no tokens", d.title);
        }
    }

    #[test]
    fn rare_terms_outweigh_common_ones() {
        // Scoring must apply idf, not just term presence. This checks the
        // score `query` actually delivers against the whole formula, so a
        // scorer that dropped the idf factor — or the PageRank blend, or the
        // exact-AND bonus — fails here with the term it disagreed about.
        //
        // A term in exactly one document is the case where the delivered
        // score is knowable: nothing else contributes to it, and the document
        // is certain to come back rather than falling outside MAX_HITS.
        //
        // The old version named "bridge" as a common term and asserted a
        // title. That held only for a 16-document corpus — and it was not
        // measuring idf at all: tf is divided by document length, so a
        // four-word file outscores a long one on any shared term whatever the
        // weights are. Re-baking the index broke a test about arithmetic.
        let rare = TERMS
            .iter()
            .find(|t| t.len == 1)
            .expect("a term in exactly one document");
        let p = &POSTINGS[rare.start];

        let hit = query(rare.word).first().copied().expect("corpus phrase");
        assert_eq!(hit.doc, p.doc, "{:?} answered another document", rare.word);

        // tf/len * idf, blended with PageRank, times the exact-AND bonus the
        // one-term query earns by carrying every term.
        let expected = p.tf * rare.idf * (1.0 + 4.0 * DOCS[p.doc].pr) * 1.35;
        assert!(
            (hit.score - expected).abs() < 1e-9,
            "{:?} scored {} against {expected} — the delivered score is not \
             tf * idf * PageRank * exact-AND",
            rare.word,
            hit.score
        );

        // And the weight is doing work: the commonest word in the corpus
        // would have scored the same document far lower.
        let common = TERMS.iter().max_by_key(|t| t.len).expect("vocabulary");
        assert!(
            rare.idf > common.idf,
            "a term in one document does not outweigh one in {}",
            common.len
        );
    }

    #[test]
    fn pagerank_outranks_a_stronger_tf_idf_match() {
        // The (1 + 4*pr) blend has to change an answer somewhere, or it is
        // dead arithmetic. Watch a real ranking rather than recomputing the
        // formula — recomputing it here only proves the test can multiply.
        //
        // For a one-word query every document scores tf * idf with the same
        // idf and the same exact-AND bonus, so without the blend `query` can
        // only ever return them in descending tf. A document delivered above
        // one with strictly higher tf therefore got there on PageRank and
        // nothing else. Ties are ignored on purpose: the selection sort in
        // `take_top` is free to order equal scores however it likes.
        let flipped = TERMS
            .iter()
            // Whole posting list has to fit in one answer, or the documents
            // missing from it would look like a reordering.
            .filter(|t| (2..=MAX_HITS).contains(&t.len))
            .find(|t| {
                let tf_of = |doc: usize| {
                    POSTINGS[t.start..t.start + t.len]
                        .iter()
                        .find(|p| p.doc == doc)
                        .map(|p| p.tf)
                        .expect("delivered a document with no posting")
                };
                let delivered: Vec<usize> = query(t.word).iter().map(|h| h.doc).collect();
                delivered
                    .iter()
                    .enumerate()
                    .any(|(i, &a)| delivered[i + 1..].iter().any(|&b| tf_of(a) < tf_of(b)))
            });
        assert!(
            flipped.is_some(),
            "no query is ordered by anything but term frequency — PageRank is \
             not reaching the ranking"
        );
    }

    #[test]
    fn carrying_every_term_beats_a_stronger_partial_match() {
        // "Agent skills" is the only document with all three terms; weaker
        // partials (e.g. email-triage) must lose to the exact-AND ladder.
        let hit = top("packages stubs skills").expect("corpus phrase");
        assert_eq!(hit, "Agent skills");
    }

    #[test]
    fn idf_falls_as_document_frequency_rises() {
        // `len` is the posting count, i.e. the document frequency.
        let mut rarest = &TERMS[0];
        let mut commonest = &TERMS[0];
        for t in TERMS.iter() {
            if t.len < rarest.len {
                rarest = t;
            }
            if t.len > commonest.len {
                commonest = t;
            }
        }
        assert!(
            rarest.len < commonest.len,
            "corpus has no df spread to test"
        );
        assert!(
            rarest.idf > commonest.idf,
            "{} (df {}) idf {} !> {} (df {}) idf {}",
            rarest.word,
            rarest.len,
            rarest.idf,
            commonest.word,
            commonest.len,
            commonest.idf
        );
    }

    #[test]
    fn ties_keep_the_kernels_ordering() {
        // Pins the tie-break documented on take_top: on equal scores the
        // selection sort promotes the last-swapped element, not the earlier
        // document. A stable sort would answer [2, 0, 1] here.
        let mut ranked = vec![
            Hit { doc: 0, score: 5.0 },
            Hit { doc: 1, score: 5.0 },
            Hit { doc: 2, score: 9.0 },
        ];
        let out = take_top(&mut ranked);
        assert_eq!(out.iter().map(|h| h.doc).collect::<Vec<_>>(), vec![2, 1, 0]);
    }

    #[test]
    fn every_vocabulary_word_finds_its_own_term() {
        // The runtime tokenizer and the build-time one must agree; if they
        // drift, lookups miss and search silently returns nothing.
        for t in TERMS.iter() {
            let found = find_term(t.word).expect(t.word);
            assert_eq!(found.word, t.word);
        }
    }

    #[test]
    fn tokenizer_complex_punctuation_and_spacing() {
        let mut toks = tokenize("   --hello,   world!!  (123)  test-case...  ");
        assert_eq!(toks.next(), Some("hello"));
        assert_eq!(toks.next(), Some("world"));
        assert_eq!(toks.next(), Some("123"));
        assert_eq!(toks.next(), Some("test"));
        assert_eq!(toks.next(), Some("case"));
        assert_eq!(toks.next(), None);
    }

    #[test]
    fn query_search_results_ordering_and_bounds() {
        let hits = query("skills");
        assert!(!hits.is_empty());
        assert!(hits.len() <= MAX_HITS);
        for i in 1..hits.len() {
            assert!(hits[i - 1].score >= hits[i].score, "scores must be descending");
        }

        let empty_hits = query("nonexistentterm12345");
        assert!(empty_hits.is_empty());
    }

    #[test]
    fn doc_table_properties() {
        assert!(!DOCS.is_empty());
        for doc in DOCS.iter() {
            assert!(!doc.title.is_empty());
            assert!(!doc.url.is_empty());
            assert!(doc.pr >= 0.0 && doc.pr <= 1.0);
            assert!(doc.n_tokens > 0);
        }
    }

    #[test]
    fn find_term_first_and_last_vocabulary() {
        let first = TERMS.first().unwrap();
        let last = TERMS.last().unwrap();
        assert_eq!(find_term(first.word).unwrap().word, first.word);
        assert_eq!(find_term(last.word).unwrap().word, last.word);
    }

    #[test]
    fn doc_table_index_bounds() {
        assert!(DOCS.get(0).is_some());
        assert!(DOCS.get(DOCS.len()).is_none());
        assert!(DOCS.get(999).is_none());
    }
}




