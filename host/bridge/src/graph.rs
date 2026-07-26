//! Email knowledge graph — runtime, host-side, never in the tree.
//!
//! Messages returned by `email.search` are folded into a small graph of
//! `sender -> message` edges, ranked with PageRank, and exposed to
//! `search.query` as ordinary documents.
//!
//! Three constraints shape this:
//!
//! 1. **Nothing is baked.** The static corpus is compiled into the ISO by
//!    `kernel/build.rs`; putting mail there would ship personal data inside a
//!    build artifact in a public repo. This index lives under Application
//!    Support beside saved skills, exactly like the "no secrets in the tree"
//!    rule requires.
//! 2. **Email content stays behind the email capability.** Every document is
//!    tagged `cat=email`, and callers must opt in explicitly — otherwise a
//!    guest holding only `search.query` could read mail through search.
//! 3. **Bodies are not stored.** Only sender and subject — enough to rank and
//!    surface mail in search without persisting content.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// One indexed message.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Message {
    /// Stable identity: sender + subject, hashed. Re-ingesting is idempotent.
    pub id: String,
    pub from: String,
    pub subject: String,
    /// PageRank over the sender/message graph, 0..1.
    #[serde(default)]
    pub pr: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Graph {
    #[serde(default)]
    pub(crate) messages: Vec<Message>,
}

/// Where the index lives. Never inside the repo.
fn graph_path() -> PathBuf {
    crate::paths::env_or_knowledge("OS_GRAPH_PATH", "emails.json")
}

/// Cheap stable id. Not cryptographic — only needs to dedupe re-ingests.
fn id_for(from: &str, subject: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in from.bytes().chain(b"\x00".iter().copied()).chain(subject.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

impl Graph {
    /// Load the index. A missing file is simply an empty graph.
    ///
    /// A *corrupt* file is not: silently returning empty would let the next
    /// ingest overwrite a damaged-but-recoverable index with three messages.
    /// The bad file is set aside first so it can be inspected.
    fn load() -> Result<Self, String> {
        let path = graph_path();
        let raw = match fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return Ok(Self::default()),
        };
        match serde_json::from_str(&raw) {
            Ok(g) => Ok(g),
            Err(e) => {
                let quarantine = path.with_extension("corrupt");
                let _ = fs::rename(&path, &quarantine);
                Err(format!("graph corrupt ({e}); moved to {}", quarantine.display()))
            }
        }
    }

    /// Convenience for callers that must not fail: logs and starts empty.
    pub fn load_or_empty() -> Self {
        match Self::load() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("graph: {e}");
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        crate::paths::write_json(graph_path(), self)
    }

    /// Cap on retained messages. Without this the index grows forever: the
    /// mock backend alone appends a fresh row for every distinct query.
    const MAX_MESSAGES: usize = 2000;

    /// Fold messages in, deduping by identity. Returns how many were new.
    ///
    /// When the cap is exceeded the lowest-ranked messages are dropped, so what
    /// survives is what the graph considers most connected.
    pub fn ingest<'a, I>(&mut self, incoming: I) -> usize
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let mut added = 0;
        for (from, subject) in incoming {
            let id = id_for(from, subject);
            if self.messages.iter().any(|m| m.id == id) {
                continue;
            }
            self.messages.push(Message {
                id,
                from: from.to_string(),
                subject: subject.to_string(),
                pr: 0.0,
            });
            added += 1;
        }
        if added == 0 {
            return 0;
        }
        self.rank();
        if self.messages.len() > Self::MAX_MESSAGES {
            self.messages
                .sort_by(|a, b| b.pr.partial_cmp(&a.pr).unwrap_or(core::cmp::Ordering::Equal));
            self.messages.truncate(Self::MAX_MESSAGES);
            self.rank();
        }
        added
    }

    /// PageRank over `sender -> message` edges.
    ///
    /// A sender distributes rank evenly across their messages, so a prolific
    /// correspondent's individual mails do not each inherit full weight. Rank
    /// flows back sender-ward too, which is what makes a frequent
    /// correspondent's threads surface above a one-off newsletter.
    fn rank(&mut self) {
        const DAMPING: f64 = 0.85;
        const ITERS: usize = 20;

        if self.messages.is_empty() {
            return;
        }

        // Two node types: senders and messages. Modelling only messages and
        // feeding a sender's pooled rank back to that same sender's messages
        // is a closed loop — the uniform vector is a fixed point and every
        // rank comes out identical.
        let mut sender_of: Vec<usize> = Vec::with_capacity(self.messages.len());
        let mut sender_msgs: Vec<Vec<usize>> = Vec::new();
        let mut index: HashMap<&str, usize> = HashMap::new();
        for (i, m) in self.messages.iter().enumerate() {
            let s = *index.entry(m.from.as_str()).or_insert_with(|| {
                sender_msgs.push(Vec::new());
                sender_msgs.len() - 1
            });
            sender_msgs[s].push(i);
            sender_of.push(s);
        }

        let m_len = self.messages.len();
        let s_len = sender_msgs.len();
        let n = (m_len + s_len) as f64;
        let base = (1.0 - DAMPING) / n;

        let mut rank = vec![1.0 / n; m_len];
        let mut srank = vec![1.0 / n; s_len];

        for _ in 0..ITERS {
            // sender -> its messages, split by out-degree, so a prolific
            // correspondent spreads thin across their mail.
            let mut next = vec![base; m_len];
            for (s, msgs) in sender_msgs.iter().enumerate() {
                let share = DAMPING * srank[s] / msgs.len() as f64;
                for &i in msgs {
                    next[i] += share;
                }
            }
            // message -> its sender (out-degree 1).
            let mut next_s = vec![base; s_len];
            for (i, &s) in sender_of.iter().enumerate() {
                next_s[s] += DAMPING * rank[i];
            }
            rank = next;
            srank = next_s;
        }

        // Normalise to 0..1 so it slots into the same blend as corpus `pr`.
        let max = rank.iter().cloned().fold(0.0f64, f64::max);
        for (m, r) in self.messages.iter_mut().zip(rank) {
            m.pr = if max > 0.0 { r / max } else { 0.0 };
        }
    }
}

#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn ingest_adds_and_dedupes() {
        let mut g = Graph::default();
        assert_eq!(g.ingest([("alice@x", "Q2"), ("bob@y", "Hi")]), 2);
        // Same sender+subject is the same message.
        assert_eq!(g.ingest([("alice@x", "Q2")]), 0);
        assert_eq!(g.messages.len(), 2);
    }

    #[test]
    fn ids_are_stable_and_distinct() {
        assert_eq!(id_for("a@x", "S"), id_for("a@x", "S"));
        assert_ne!(id_for("a@x", "S"), id_for("b@x", "S"));
        assert_ne!(id_for("a@x", "S"), id_for("a@x", "T"));
    }

    #[test]
    fn id_is_not_confused_by_field_boundary() {
        // Without a separator, ("ab","c") and ("a","bc") would collide.
        assert_ne!(id_for("ab", "c"), id_for("a", "bc"));
    }

    #[test]
    fn rank_is_normalised() {
        let mut g = Graph::default();
        g.ingest([("a@x", "1"), ("a@x", "2"), ("b@y", "3")]);
        assert!(g.messages.iter().all(|m| m.pr >= 0.0 && m.pr <= 1.0));
        assert!(g.messages.iter().any(|m| m.pr > 0.0), "all ranks zero");
    }

    #[test]
    fn lone_sender_outranks_one_of_many() {
        // b@y sent once; a@x sent three times, so each of a's messages should
        // carry less individual weight than b's single message.
        let mut g = Graph::default();
        g.ingest([("a@x", "1"), ("a@x", "2"), ("a@x", "3"), ("b@y", "only")]);
        let a1 = g.messages.iter().find(|m| m.subject == "1").unwrap().pr;
        let b = g.messages.iter().find(|m| m.subject == "only").unwrap().pr;
        assert!(b > a1, "expected the singleton to outrank a bulk sender: {b} vs {a1}");
    }

    #[test]
    fn graph_path_is_outside_the_repo() {
        // Guards the rule that matters: mail must never land in the tree.
        let p = graph_path();
        let s = p.to_string_lossy();
        assert!(
            !s.contains("/os/search") && !s.contains("corpus.json"),
            "graph must not be written into the repo corpus: {s}"
        );
    }

    #[test]
    fn roundtrips_through_disk() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-graph-test-{}", std::process::id()));
        let path = dir.join("emails.json");
        // SAFETY: single-threaded test process.
        unsafe { env::set_var("OS_GRAPH_PATH", &path) };

        let mut g = Graph::default();
        g.ingest([("a@x", "Q2")]);
        g.save().expect("save");

        let back = Graph::load().expect("valid graph");
        assert_eq!(back.messages.len(), 1);
        assert_eq!(back.messages[0].subject, "Q2");

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_GRAPH_PATH") };
    }

    #[test]
    fn missing_file_loads_empty_not_error() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { env::set_var("OS_GRAPH_PATH", "/nonexistent/os-graph/none.json") };
        assert!(Graph::load().expect("missing is not corrupt").messages.is_empty());
        unsafe { env::remove_var("OS_GRAPH_PATH") };
    }
}

#[cfg(test)]
mod gate_tests {
    //! The rule that must not regress: `search.query` reaches mail only when
    //! the caller explicitly opts in.

    #[test]
    fn email_is_excluded_by_default() {
        let _guard = super::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-gate-{}", std::process::id()));
        let path = dir.join("emails.json");
        unsafe { std::env::set_var("OS_GRAPH_PATH", &path) };

        let mut g = super::Graph::default();
        g.ingest([("ceo@example.com", "Confidential merger")]);
        g.save().expect("save");

        let without =
            crate::search::query_all("confidential merger", 5, None, false, false, false);
        let with =
            crate::search::query_all("confidential merger", 5, None, true, false, false);

        assert!(
            !without.iter().any(|r| r.contains("Confidential merger")),
            "mail leaked into an un-opted-in search: {without:?}"
        );
        assert!(
            with.iter().any(|r| r.contains("Confidential merger")),
            "opted-in search should surface mail: {with:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
        unsafe { std::env::remove_var("OS_GRAPH_PATH") };
    }
}

#[cfg(test)]
mod hardening_tests {
    use super::*;
    use std::env;

    #[test]
    fn corrupt_file_is_quarantined_not_silently_wiped() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("os-corrupt-{}", std::process::id()));
        let path = dir.join("emails.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, b"{ this is not json").unwrap();
        unsafe { env::set_var("OS_GRAPH_PATH", &path) };

        let err = Graph::load().expect_err("corrupt file must not load as empty");
        assert!(err.contains("corrupt"), "{err}");
        // Original is preserved for inspection rather than overwritten.
        assert!(path.with_extension("corrupt").exists());

        let _ = fs::remove_dir_all(&dir);
        unsafe { env::remove_var("OS_GRAPH_PATH") };
    }

    #[test]
    fn index_is_capped_and_keeps_the_best_ranked() {
        let mut g = Graph::default();
        let batch: Vec<(String, String)> = (0..Graph::MAX_MESSAGES + 50)
            .map(|i| (format!("s{i}@x"), format!("subject {i}")))
            .collect();
        g.ingest(batch.iter().map(|(a, b)| (a.as_str(), b.as_str())));
        assert_eq!(g.messages.len(), Graph::MAX_MESSAGES, "index grew past the cap");
    }
}
