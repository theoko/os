//! Understand what the person meant, then act on it.
//!
//! The search box used to hand whatever you typed straight to a tf-idf index.
//! That works for `q2 planning` and fails completely for "i wanna work on my
//! paper" — the words carrying the intent (`wanna`, `work on`) are exactly the
//! words a keyword index throws away, and the one word left (`paper`) gets
//! matched against titles with no idea that you meant *resume the thing you
//! were last writing*.
//!
//! So this module does what an agent does: read the goal, decide what kind of
//! request it is, gather evidence from the sources you actually granted, and
//! come back with a sentence plus steps you can click. Every step is a real
//! URL that `doc.read` can open, not advice.
//!
//! It stays read-only. It cannot write files, send mail, or reach the network,
//! and it can only see a source whose capability is granted — an ungranted
//! source is reported as a switch you could flip, never quietly read.

use crate::{graph, transcribe, workspace};

/// What the person is trying to do. Chosen from the phrasing, not the nouns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// "work on", "continue", "get back to" — they had something in progress.
    Resume,
    /// "find", "where is", "show me" — they know it exists.
    Find,
    /// "open", "read" — they named one thing.
    Read,
    /// "what's new", "catch me up" — no subject, wants a state of the world.
    Catchup,
    /// A question. Wants an answer assembled from sources, not a file.
    Ask,
    /// Bare keywords. Behave like search, because that is what was asked for.
    Unknown,
}

impl Intent {
    pub fn name(self) -> &'static str {
        match self {
            Intent::Resume => "resume",
            Intent::Find => "find",
            Intent::Read => "read",
            Intent::Catchup => "catchup",
            Intent::Ask => "ask",
            Intent::Unknown => "search",
        }
    }
}

/// One thing the person can do next. `url` is openable; `why` earns its place.
#[derive(Debug, Clone)]
pub struct Step {
    pub label: String,
    pub url: String,
    pub why: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Grants {
    pub files: bool,
    pub email: bool,
    pub audio: bool,
    pub portal: bool,
}

#[derive(Debug)]
pub struct Answer {
    pub say: String,
    pub intent: Intent,
    pub subject: String,
    pub steps: Vec<Step>,
}

/// Read the goal, gather what the grants allow, answer with clickable steps.
///
/// `limit` is how many rows the caller can actually show. It is applied before
/// the sentence is written, because the sentence counts them: truncating
/// afterwards is how "5 matches" ended up printed above three visible rows.
pub fn act(goal: &str, grants: Grants, limit: usize) -> Answer {
    let intent = classify(goal);
    let subject = subject_of(goal);
    let terms = terms(&subject);

    let mut found = Vec::new();
    if grants.files {
        // "What did I miss" has no subject to match on. Matching the leftover
        // words anyway is how it answered with 43-day-old notes containing the
        // word "missing" - recency is the whole question, so ask that instead.
        if intent == Intent::Catchup {
            found.extend(recent_files());
        } else {
            found.extend(from_files(&terms, intent));
        }
    }
    if grants.email {
        found.extend(from_mail(&terms));
    }
    if grants.audio {
        found.extend(from_audio(&terms));
    }

    // Best first. Ties break toward whatever was touched most recently, which
    // is the right guess for "the thing I was working on".
    found.sort_by(|a, b| b.score.cmp(&a.score));
    found.truncate(limit);

    let say = narrate(intent, &subject, &found, grants);
    let steps = found
        .into_iter()
        .map(|c| Step { label: c.label, url: c.url, why: c.why })
        .collect();

    Answer { say, intent, subject, steps }
}

/// A candidate answer with the score that ranked it.
struct Cand {
    label: String,
    url: String,
    why: String,
    score: i64,
    /// True when nothing in the goal actually matched this - it is here
    /// because it *looks like* the kind of thing asked for. Tracked so the
    /// sentence can admit it rather than presenting a guess as a find.
    guessed: bool,
}

// --------------------------------------------------------------- understanding

/// Phrases that reveal intent. Longest first: "get back to" must beat "back".
const RESUME: &[&str] = &[
    "work on", "working on", "get back to", "back to", "continue", "carry on",
    "keep going", "pick up", "finish", "resume",
];
const FIND: &[&str] = &["where is", "where's", "find", "show me", "look for", "locate", "search for"];
const READ: &[&str] = &["open", "read", "pull up"];
const CATCHUP: &[&str] = &[
    "what's new", "whats new", "catch me up", "catch up", "what happened",
    "what did i miss", "brief me", "my day",
];
const QUESTION: &[&str] = &["what ", "why ", "how ", "when ", "who ", "which ", "is ", "does ", "did "];

pub fn classify(goal: &str) -> Intent {
    let g = normalize(goal);
    if CATCHUP.iter().any(|p| g.contains(p)) {
        return Intent::Catchup;
    }
    if RESUME.iter().any(|p| g.contains(p)) {
        return Intent::Resume;
    }
    if FIND.iter().any(|p| g.contains(p)) {
        return Intent::Find;
    }
    // "open" reads as a command only where a command goes.
    if READ.iter().any(|p| g.starts_with(p)) {
        return Intent::Read;
    }
    if g.ends_with('?') || QUESTION.iter().any(|p| g.starts_with(p)) {
        return Intent::Ask;
    }
    Intent::Unknown
}

/// Words that carry intent or politeness but never identify a document.
const FILLER: &[&str] = &[
    "i", "im", "ive", "id", "me", "my", "mine", "we", "our", "you", "your",
    "wanna", "want", "wants", "wanted", "need", "needs", "gotta", "should",
    "would", "could", "can", "will", "lets", "let", "please", "help", "hey",
    "the", "a", "an", "some", "any", "that", "this", "those", "these",
    "to", "of", "on", "in", "for", "with", "about", "from", "at", "it",
    "and", "or", "but", "is", "are", "was", "were", "be", "been", "do", "does",
    "did", "get", "got", "go", "going", "now", "today", "again", "up", "back",
    "work", "working", "continue", "finish", "resume", "open", "read", "find",
    "show", "where", "look", "search", "pull", "keep", "pick", "carry",
    // Question and catch-up words. Without these, "what did i miss" searched
    // for the term "miss" and confidently returned month-old notes that
    // happened to contain "missing".
    "what", "whats", "why", "how", "when", "who", "which", "miss", "missed",
    "happened", "new", "anything", "else", "catch", "brief", "day", "days",
];

/// What the goal is *about*, with the intent words stripped out.
pub fn subject_of(goal: &str) -> String {
    normalize(goal)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty() && !FILLER.contains(w))
        .collect::<Vec<_>>()
        .join(" ")
}

fn terms(subject: &str) -> Vec<String> {
    subject
        .split_whitespace()
        // Two characters is enough to be a real term ("q2", "ml", "os").
        .filter(|w| w.len() >= 2)
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

fn normalize(s: &str) -> String {
    s.trim().to_ascii_lowercase()
}

// -------------------------------------------------------------------- gathering

/// Extensions that mean "something a person writes", used when the subject is
/// a document *kind* rather than a name.
const WRITING: &[&str] = &[".md", ".txt", ".tex", ".doc", ".docx", ".odt", ".rtf", ".pages"];

/// Files that are markdown but are not anybody's paper. Asking for "my paper"
/// and getting CHANGELOG.md back is worse than getting nothing: it is a
/// confident wrong answer, and every repo is full of these.
const FURNITURE: &[&str] = &[
    "readme", "changelog", "license", "licence", "contributing", "agents",
    "claude", "todo", "notes.md", "index.md", "makefile", "codeowners",
];

fn is_furniture(path: &str) -> bool {
    let stem = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    FURNITURE.iter().any(|f| stem.starts_with(f))
}

/// Nouns describing a kind of document rather than naming one. "my paper"
/// says what to look for even though no file is called "paper".
fn kind_of(terms: &[String]) -> Option<&'static str> {
    terms.iter().find_map(|t| match t.as_str() {
        "paper" | "essay" | "thesis" | "manuscript" | "draft" | "article" | "notes" | "note" => {
            Some("writing")
        }
        _ => None,
    })
}

/// How much a day of staleness costs. Resuming is about *recent*; finding is
/// about *matching*, so recency must not outvote the name there.
fn recency_weight(intent: Intent) -> i64 {
    if intent == Intent::Resume {
        12
    } else {
        4
    }
}

fn from_files(terms: &[String], intent: Intent) -> Vec<Cand> {
    let kind = kind_of(terms);
    let roots = workspace::roots();
    workspace::Index::load()
        .entries
        .into_iter()
        .filter_map(|e| {
            let hay = format!("{} {}", e.title, e.path).to_ascii_lowercase();
            let hits = terms.iter().filter(|t| hay.contains(t.as_str())).count() as i64;
            let is_writing = WRITING.iter().any(|ext| hay.ends_with(ext));
            let kind_hit = kind == Some("writing") && is_writing && !is_furniture(&e.path);
            if hits == 0 && !kind_hit {
                return None;
            }
            // A named match dominates; a kind match is a weaker, honest guess.
            let mut score = hits * 1000 + if kind_hit { 120 } else { 0 };
            let days = age_days(&roots, &e.path);
            if let Some(d) = days {
                score += (60 - d.min(60)) * recency_weight(intent);
            }
            score += (e.pr * 50.0) as i64;
            Some(Cand {
                label: clean(&e.title),
                url: format!("file://{}", e.path),
                why: match days {
                    Some(0) => "edited today".into(),
                    Some(1) => "edited yesterday".into(),
                    Some(d) if d < 60 => format!("edited {d} days ago"),
                    _ => "in your files".into(),
                },
                score,
                guessed: hits == 0,
            })
        })
        .collect()
}

/// How recent counts as "new" when someone asks what they missed.
const CATCHUP_DAYS: i64 = 14;

/// Everything touched lately, newest first. No term matching at all.
fn recent_files() -> Vec<Cand> {
    let roots = workspace::roots();
    workspace::Index::load()
        .entries
        .into_iter()
        .filter_map(|e| {
            let days = age_days(&roots, &e.path)?;
            if days > CATCHUP_DAYS {
                return None;
            }
            Some(Cand {
                label: clean(&e.title),
                url: format!("file://{}", e.path),
                why: match days {
                    0 => "edited today".into(),
                    1 => "edited yesterday".into(),
                    d => format!("edited {d} days ago"),
                },
                // Newest first, and nothing else gets a say.
                score: (CATCHUP_DAYS - days) * 100,
                guessed: false,
            })
        })
        .collect()
}

/// Days since the file was last written, if it can still be found on disk.
fn age_days(roots: &[std::path::PathBuf], rel: &str) -> Option<i64> {
    let meta = roots.iter().find_map(|root| std::fs::metadata(root.join(rel)).ok())?;
    let secs = std::time::SystemTime::now()
        .duration_since(meta.modified().ok()?)
        .ok()?
        .as_secs();
    Some((secs / 86_400) as i64)
}

fn from_mail(terms: &[String]) -> Vec<Cand> {
    graph::Graph::load_or_empty()
        .messages
        .into_iter()
        .filter_map(|m| {
            let hay = format!("{} {}", m.subject, m.from).to_ascii_lowercase();
            let hits = terms.iter().filter(|t| hay.contains(t.as_str())).count() as i64;
            if hits == 0 {
                return None;
            }
            Some(Cand {
                label: clean(&m.subject),
                url: format!("email://{}", m.id),
                why: format!("email from {}", clean(&m.from)),
                score: hits * 1000 + (m.pr * 40.0) as i64,
                guessed: false,
            })
        })
        .collect()
}

fn from_audio(terms: &[String]) -> Vec<Cand> {
    transcribe::Store::load()
        .items
        .into_iter()
        .filter_map(|t| {
            let hay = format!("{} {}", t.source, t.text).to_ascii_lowercase();
            let hits = terms.iter().filter(|q| hay.contains(q.as_str())).count() as i64;
            if hits == 0 {
                return None;
            }
            Some(Cand {
                label: clean(&t.source),
                url: format!("audio://{}", t.source),
                why: "from a recording".into(),
                score: hits * 900,
                guessed: false,
            })
        })
        .collect()
}

// ------------------------------------------------------------------- answering

/// The sentence shown above the steps. It must describe what actually
/// happened — including having found nothing, and including which switch is
/// off. A confident sentence over an empty list is how search UIs lie.
fn narrate(intent: Intent, subject: &str, found: &[Cand], grants: Grants) -> String {
    if !grants.files && !grants.email && !grants.audio {
        return "Everything is switched off, so I have nothing to look at. \
                Turn on a source in Capabilities and ask me again."
            .into();
    }
    let subj = if subject.is_empty() { "that" } else { subject };

    if found.is_empty() {
        let mut s = match intent {
            Intent::Resume => format!("I could not find anything of yours about {subj}."),
            Intent::Catchup => {
                format!("Nothing has changed in the last {CATCHUP_DAYS} days that I can see.")
            }
            _ => format!("Nothing here matches {subj}."),
        };
        if !grants.files {
            s.push_str(" Your files are switched off - turn them on and I can look there.");
        } else if !grants.email {
            s.push_str(" Your email is switched off - turn it on to include mail.");
        }
        return s;
    }

    let top = &found[0];

    // Nothing matched by name; these are here on a hunch about the file kind.
    // Presenting that as a find is how "my paper" confidently returned the
    // changelog - say it is a guess and the person can judge for themselves.
    if top.guessed {
        return format!(
            "Nothing of yours is named {subj}. Showing your most recent documents - {} is the freshest ({}).",
            top.label, top.why
        );
    }

    match intent {
        Intent::Resume => format!("Picking {subj} back up. Freshest is {} ({}).", top.label, top.why),
        Intent::Find => format!("Found {} for {subj} - closest is {}.", found.len(), top.label),
        Intent::Read => format!("Opening {}.", top.label),
        Intent::Catchup => {
            format!("{} things worth a look, starting with {}.", found.len(), top.label)
        }
        Intent::Ask => format!("Here is what I have on {subj}; {} looks closest.", top.label),
        Intent::Unknown => format!("{} matches for {subj}.", found.len()),
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() || c == '|' { ' ' } else { c })
        .take(90)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Grants {
        Grants { files: true, email: true, audio: true, portal: true }
    }

    #[test]
    fn a_first_person_sentence_is_understood_as_resuming() {
        assert_eq!(classify("i wanna work on my paper"), Intent::Resume);
        assert_eq!(classify("let me get back to the thesis"), Intent::Resume);
    }

    #[test]
    fn the_subject_survives_and_the_filler_does_not() {
        assert_eq!(subject_of("i wanna work on my paper"), "paper");
        assert_eq!(subject_of("can you find the q2 planning notes please"), "q2 planning notes");
    }

    #[test]
    fn questions_and_catchups_are_told_apart() {
        assert_eq!(classify("what did i miss"), Intent::Catchup);
        assert_eq!(classify("why did the build fail?"), Intent::Ask);
        assert_eq!(classify("q2 planning"), Intent::Unknown);
    }

    #[test]
    fn open_is_a_command_only_in_command_position() {
        assert_eq!(classify("open the budget"), Intent::Read);
        // "open questions" as a subject would be misread as a command if the
        // verb were matched anywhere in the sentence.
        assert_eq!(classify("the open questions from review"), Intent::Unknown);
    }

    #[test]
    fn nothing_is_read_from_a_source_that_was_not_granted() {
        let a = act("i wanna work on my paper", Grants::default(), 5);
        assert!(a.steps.is_empty(), "ungranted sources must produce no steps");
        assert!(a.say.contains("switched off"), "say why it is empty: {}", a.say);
    }

    #[test]
    fn an_empty_result_says_so_rather_than_sounding_confident() {
        let a = act("work on my zzzznonexistent", Grants { files: true, ..Grants::default() }, 5);
        assert!(a.steps.is_empty());
        assert!(a.say.starts_with("I could not find"), "{}", a.say);
    }

    #[test]
    fn steps_are_openable_urls_not_prose() {
        for s in &act("q2 planning notes", all(), 5).steps {
            assert!(
                s.url.starts_with("file://")
                    || s.url.starts_with("email://")
                    || s.url.starts_with("audio://"),
                "step {s:?} is not openable"
            );
        }
    }

    #[test]
    fn a_paper_matches_the_kind_of_file_people_write_in() {
        // No file is literally named "paper" - the kind noun has to carry it.
        assert_eq!(kind_of(&["paper".into()]), Some("writing"));
        assert_eq!(kind_of(&["kubernetes".into()]), None);
    }

    #[test]
    fn resuming_weighs_staleness_harder_than_finding_does() {
        assert!(
            recency_weight(Intent::Resume) > recency_weight(Intent::Find),
            "otherwise 'work on' and 'find' are one feature with two names"
        );
    }
}

#[cfg(test)]
mod understanding_tests {
    use super::*;

    #[test]
    fn catchup_words_are_not_treated_as_search_terms() {
        // "miss" once matched files containing "missing", so asking what was
        // new returned month-old notes with total confidence.
        assert_eq!(subject_of("what did i miss"), "");
        assert_eq!(subject_of("what's new"), "s");
        assert_eq!(subject_of("catch me up"), "");
    }

    #[test]
    fn repo_furniture_is_not_anybodys_paper() {
        assert!(is_furniture("CHANGELOG.md"));
        assert!(is_furniture("docs/README.md"));
        assert!(is_furniture("AGENTS.md"));
        assert!(!is_furniture("docs/thesis-chapter-3.md"));
        assert!(!is_furniture("papers/energy-markets.tex"));
    }

    #[test]
    fn a_named_match_always_outranks_a_guess_from_the_file_kind() {
        // A kind hit is worth 120 plus at most 60*12 recency; a single named
        // hit is worth 1000. If that ever inverts, "my paper" starts returning
        // whatever was edited most recently regardless of what it is.
        let best_kind_guess = 120 + 60 * recency_weight(Intent::Resume);
        assert!(
            1000 > best_kind_guess,
            "a guess ({best_kind_guess}) must not beat a real match"
        );
    }
}

#[cfg(test)]
mod honesty_tests {
    use super::*;

    #[test]
    fn the_sentence_never_counts_rows_the_caller_cannot_show() {
        // "5 matches for nvda." printed above three rows: the number was true
        // of the search and false of the screen, which is the only place
        // anybody reads it.
        let grants = Grants { files: true, email: true, audio: true, portal: true };
        for limit in 1..=5 {
            let a = act("planning", grants, limit);
            assert!(a.steps.len() <= limit, "returned more rows than asked for");
            if let Some(n) = leading_number(&a.say) {
                assert_eq!(n, a.steps.len(), "said {n}, returned {}", a.steps.len());
            }
        }
    }

    /// The count a sentence opens with, if it opens with one.
    fn leading_number(say: &str) -> Option<usize> {
        say.split_whitespace().next()?.parse().ok()
    }
}

#[cfg(test)]
mod guess_tests {
    use super::*;

    fn cand(guessed: bool) -> Cand {
        Cand {
            label: "Changelog".into(),
            url: "file://CHANGELOG.md".into(),
            why: "edited today".into(),
            score: 1,
            guessed,
        }
    }

    #[test]
    fn a_guess_is_labelled_as_one() {
        let g = Grants { files: true, ..Grants::default() };
        let say = narrate(Intent::Resume, "paper", &[cand(true)], g);
        assert!(say.starts_with("Nothing of yours is named paper"), "{say}");
    }

    #[test]
    fn a_real_match_is_not_hedged() {
        // Over-hedging is its own failure: if every answer says "maybe", the
        // warning stops carrying information.
        let g = Grants { files: true, ..Grants::default() };
        let say = narrate(Intent::Resume, "changelog", &[cand(false)], g);
        assert!(say.starts_with("Picking changelog back up"), "{say}");
    }

    #[test]
    fn catch_up_results_are_not_treated_as_guesses() {
        // Recency *is* the question there, so recent files are the answer,
        // not a hunch about one.
        let g = Grants { files: true, ..Grants::default() };
        let say = narrate(Intent::Catchup, "", &[cand(false)], g);
        assert!(say.contains("worth a look"), "{say}");
    }
}
