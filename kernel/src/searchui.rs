//! The search screen — the first part of this OS you can actually *use*.
//!
//! Type a query, press Enter, read answers. Results come from the index baked
//! into the kernel (`search.rs`), so this works with no host bridge at all;
//! email is folded in by the bridge when that capability was granted.
//!
//! Previously the home screen ran searches on a card click and wrote the hits
//! to COM1 — invisible unless you were watching a serial console.

use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, SMALL_FACE, TITLE_FACE};
use crate::screens;
use crate::search;
use crate::skills::{copy_field, str_at};
use crate::ui::theme;

/// Longest query we accept. Comfortably wider than the field renders.
pub const QUERY_MAX: usize = 64;

/// A rendered result row.
///
/// Owns its text: bridge results are parsed out of a COM2 line buffer that is
/// reused on the next call, so borrowing from it would dangle. Copying into
/// fixed slots keeps the whole path free of unsafe lifetime tricks.
#[derive(Clone, Copy)]
struct Row {
    title: [u8; 56],
    url: [u8; 72],
    cat: &'static str,
}

impl Row {
    const fn empty() -> Self {
        Self { title: [0; 56], url: [0; 72], cat: "" }
    }

    fn set(&mut self, title: &str, url: &str, cat: &'static str) {
        copy_field(&mut self.title, title);
        copy_field(&mut self.url, url);
        self.cat = cat;
    }

    fn title(&self) -> &str {
        str_at(&self.title)
    }

    fn url(&self) -> &str {
        str_at(&self.url)
    }

}

/// Shown when something actually broke, as opposed to simply finding nothing.
/// Named for the search engine this OS queries.
const TEDDY: &str = "Teddy is looking into it.";

/// What the last query actually did, so the empty state can be truthful.
///
/// A bridge that answers "no matches" is NOT an offline bridge — reporting it
/// as one sent people looking for a connection problem that did not exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// No query run yet — idle tip uses live bridge status, not this.
    Idle,
    /// Queried, but COM2 was down (or the query was empty).
    Offline,
    /// `search.query` was not granted.
    Denied,
    /// Bridge answered; grants name what's missing when the result set is empty.
    Online(crate::caps::Caps),
}

impl Phase {
    /// One line explaining an empty result set, naming the fix when there is one.
    fn empty_reason(self) -> &'static str {
        use crate::caps::Cap;
        match self {
            // Idle is exhaustive only; draw never paints empty_reason while idle.
            Phase::Idle | Phase::Offline => crate::mcp::NO_MATCHES_BRIDGE_OFFLINE,
            Phase::Denied => TEDDY,
            Phase::Online(caps) => {
                if !caps.allows(Cap::WorkspaceIndex) {
                    return "No matches. Turn on workspace.index in Capabilities to search your files.";
                }
                if !caps.allows(Cap::EmailSearch) {
                    return "No matches in your files. Turn on email.search to include mail.";
                }
                "No matches. The bridge searched your files and mail."
            }
        }
    }
}

pub struct SearchView {
    rows: [Row; search::MAX_HITS],
    count: usize,
    phase: Phase,
}

impl SearchView {
    pub const fn new() -> Self {
        Self {
            rows: [Row::empty(); search::MAX_HITS],
            count: 0,
            phase: Phase::Idle,
        }
    }

    /// Load baked-index hits for `q` into `rows` (does not touch `phase`).
    fn fill_local(&mut self, q: &str) {
        self.count = 0;
        if q.trim().is_empty() {
            return;
        }
        let mut hits = [search::Hit::EMPTY; search::MAX_HITS];
        let n = search::query(q, &mut hits);
        for h in hits.iter().take(n) {
            let d = &search::DOCS[h.doc];
            self.rows[self.count].set(d.title, d.url, d.cat);
            self.count += 1;
        }
    }

    /// Run `q` against the in-kernel index only (unit tests).
    ///
    /// Production always goes through [`Self::run_via`]; the baked corpus alone
    /// is still useful for offline assertions.
    #[cfg(test)]
    fn run(&mut self, q: &str) {
        self.phase = Phase::Offline;
        self.fill_local(q);
    }

    /// Ask the bridge first, fall back to the baked index when it is down.
    ///
    /// Without this the screen only ever saw the built-in documents, so every
    /// question about the user's own files or mail came back empty even though
    /// the bridge had them indexed.
    pub fn run_via(&mut self, q: &str, caps: crate::caps::Caps) {
        self.count = 0;
        if q.trim().is_empty() {
            self.phase = Phase::Offline;
            return;
        }
        // Refuse before CALL: no PING/LIST traffic without the search cap.
        if !caps.allows(crate::caps::Cap::SearchQuery) {
            self.phase = Phase::Denied;
            self.fill_local(q);
            return;
        }
        let peek = crate::mcp::fetch_search_peek(caps, q);
        let online = matches!(peek.status, crate::mcp::BridgeStatus::Online);
        // Record reachability BEFORE any fallback, so an online bridge that
        // simply found nothing is never reported as a connection failure.
        self.phase = if online {
            Phase::Online(caps)
        } else {
            Phase::Offline
        };
        if online {
            for i in 0..peek.count.min(search::MAX_HITS) {
                self.rows[self.count].set(peek.title_at(i), peek.url_at(i), "bridge");
                self.count += 1;
            }
        }
        if self.count == 0 {
            // Nothing from the bridge: try what we shipped with.
            self.fill_local(q);
        }
    }

    /// Which drawn result contains `(x, y)`, if any.
    pub fn hit(&self, w: i32, x: i32, y: i32) -> Option<usize> {
        crate::ui::hit_among(self.count, x, y, |i| row_rect(w, i))
    }

    pub fn title_at(&self, i: usize) -> &str {
        self.rows[i].title()
    }

    pub fn url_at(&self, i: usize) -> &str {
        self.rows[i].url()
    }
}

const FIELD_H: i32 = 52;
const ROW_H: i32 = 64;

/// Geometry shared by the renderer and hit-testing.
fn field_rect(w: i32) -> crate::ui::Rect {
    let (x, cw) = screens::column(w);
    crate::ui::Rect::new(x, 150, cw, FIELD_H)
}

/// Bounding box of result row `i`, shared by drawing and hit-testing.
fn row_rect(w: i32, i: usize) -> crate::ui::Rect {
    let f = field_rect(w);
    crate::ui::Rect::new(f.x, f.y + f.h + 26 + i as i32 * (ROW_H + 10), f.w, ROW_H)
}

/// Draw the search screen.
pub fn draw(
    fb: &Surface,
    view: &SearchView,
    query: &str,
    status: crate::mcp::BridgeStatus,
) {
    let w = fb.width() as i32;
    screens::chrome(fb, Some("Search"), Some("What do you want to know?"));

    // Input field.
    let f = field_rect(w);
    crate::ui::draw_query_field(fb, f, query, "Type a query, then press Enter", None);

    // Results.
    let mut y = f.y + f.h + 26;
    if view.phase == Phase::Idle {
        let note = match status {
            crate::mcp::BridgeStatus::Online => {
                "Answers come from the local index and the host bridge."
            }
            crate::mcp::BridgeStatus::Offline => crate::mcp::BRIDGE_OFFLINE_HINT,
        };
        fb.draw_text_centered(w / 2, y + 30, note, &SMALL_FACE, 0, theme::MUTED);
        return;
    }
    if view.count == 0 {
        fb.draw_text_centered(w / 2, y + 30, view.phase.empty_reason(), &BODY_FACE, 0, theme::MUTED);
        return;
    }

    for i in 0..view.count {
        let r = &view.rows[i];
        crate::ui::outlined_round_rect(fb, crate::ui::Rect::new(f.x, y, f.w, ROW_H), 10);
        fb.draw_text(f.x + 18, y + 26, r.title(), &BRAND_FACE, 0, theme::INK);
        // Category chip, right-aligned.
        let cw = SMALL_FACE.width(r.cat, 0);
        fb.draw_text(f.x + f.w - 18 - cw, y + 26, r.cat, &SMALL_FACE, 0, theme::ACCENT);
        fb.draw_text(f.x + 18, y + 48, r.url(), &SMALL_FACE, 0, theme::MUTED);
        y += ROW_H + 10;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_a_query_populates_rows() {
        let mut v = SearchView::new();
        v.run("capability agent");
        assert_ne!(v.phase, Phase::Idle);
        assert!(v.count > 0, "expected hits from the baked index");
        assert!(!v.rows[0].title().is_empty());
    }

    #[test]
    fn empty_query_searches_nothing_but_marks_searched() {
        let mut v = SearchView::new();
        v.run("   ");
        assert_ne!(
            v.phase,
            Phase::Idle,
            "must distinguish 'ran and found nothing' from idle"
        );
        assert_eq!(v.count, 0);
    }

    #[test]
    fn nonsense_query_yields_no_rows() {
        let mut v = SearchView::new();
        v.run("zzzz qqqq");
        assert_eq!(v.count, 0);
    }

    #[test]
    fn rerunning_replaces_previous_results() {
        let mut v = SearchView::new();
        v.run("capability agent");
        let first = v.count;
        assert!(first > 0);
        v.run("zzzz qqqq");
        assert_eq!(v.count, 0, "stale rows must not survive a new search");
    }

    #[test]
    fn count_never_exceeds_row_capacity() {
        let mut v = SearchView::new();
        v.run("os agent kernel search skills bridge capability docs");
        assert!(v.count <= search::MAX_HITS);
    }

    #[test]
    fn field_fits_a_1024_screen() {
        use crate::ui::PAD_X;
        let r = field_rect(1024);
        let (x, w) = (r.x, r.w);
        assert!(x >= PAD_X);
        assert!(x + w <= 1024 - PAD_X);
    }

    #[test]
    fn results_fit_below_the_field_at_768() {
        let r = field_rect(1024);
        let (fy, fh) = (r.y, r.h);
        let bottom = fy + fh + 26 + (search::MAX_HITS as i32) * (ROW_H + 10);
        assert!(bottom < 768, "results run off a 768px screen: {bottom}");
    }
}

#[cfg(test)]
mod empty_state_tests {
    use super::*;
    use crate::caps::{Cap, Caps};

    fn online(files: bool, mail: bool) -> Phase {
        let mut caps = Caps::none();
        if mail {
            caps.toggle(Cap::EmailSearch as usize);
        }
        if files {
            caps.toggle(Cap::WorkspaceIndex as usize);
        }
        Phase::Online(caps)
    }

    #[test]
    fn an_online_bridge_is_never_reported_as_offline() {
        // The bug this replaces: a bridge that answered "n=0" was rendered as
        // "Bridge offline", sending the user to debug a working connection.
        assert!(!online(true, true).empty_reason().contains("offline"));
    }

    #[test]
    fn a_real_outage_still_says_offline() {
        let m = Phase::Offline.empty_reason();
        assert!(m.contains("offline"));
        assert!(!m.contains("Teddy"));
    }

    #[test]
    fn missing_file_grant_names_the_fix() {
        // Finding nothing is a legitimate answer and must stay actionable
        // rather than being papered over with a mascot.
        let m = online(false, true).empty_reason();
        assert!(m.contains("workspace.index"), "{m}");
        assert!(!m.contains("offline"), "{m}");
        assert_ne!(m, TEDDY);
    }

    #[test]
    fn missing_mail_grant_names_the_fix() {
        assert!(online(true, false).empty_reason().contains("email.search"));
    }

    #[test]
    fn a_real_failure_gets_the_friendly_line() {
        assert_eq!(Phase::Denied.empty_reason(), TEDDY);
    }

    #[test]
    fn every_reason_is_renderable_ascii() {
        for s in [
            Phase::Offline,
            online(false, false),
            online(true, false),
            online(true, true),
            Phase::Denied,
        ] {
            let m = s.empty_reason();
            assert!(m.bytes().all(|b| (0x20..=0x7E).contains(&b)), "{m}");
            assert!(BODY_FACE.width(m, 0) < 980, "empty-state line overflows: {m}");
        }
    }
}

/// Draw a document the user opened from a result.
pub fn draw_reader(fb: &Surface, page: &crate::mcp::DocPage) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;
    screens::chrome(fb, None, None);

    let fx = field_rect(w).x;
    fb.draw_text(
        fx,
        108,
        page.title(),
        &TITLE_FACE,
        font::tracking_pct(TITLE_FACE.px, -20),
        theme::INK,
    );

    if page.denied {
        fb.draw_text(fx, 160, TEDDY, &BODY_FACE, 0, theme::MUTED);
        return;
    }
    if page.count == 0 {
        let msg = match page.status {
            crate::mcp::BridgeStatus::Online => "Nothing readable here.",
            crate::mcp::BridgeStatus::Offline => crate::mcp::BRIDGE_OFFLINE_HINT,
        };
        fb.draw_text(fx, 160, msg, &BODY_FACE, 0, theme::MUTED);
        return;
    }

    let mut y = 156;
    for i in 0..page.count {
        fb.draw_text(fx, y, page.line_at(i), &BODY_FACE, 0, theme::INK);
        y += 26;
        if y > h - 40 {
            break;
        }
    }
}

#[cfg(test)]
mod reader_tests {
    use super::*;

    #[test]
    fn result_rows_are_clickable_at_their_centre() {
        let mut v = SearchView::new();
        v.count = search::MAX_HITS;
        for i in 0..search::MAX_HITS {
            let r = row_rect(1024, i);
            assert_eq!(v.hit(1024, r.x + r.w / 2, r.y + r.h / 2), Some(i));
        }
    }

    #[test]
    fn clicks_below_the_last_result_open_nothing() {
        let mut v = SearchView::new();
        v.count = search::MAX_HITS;
        let r = row_rect(1024, search::MAX_HITS - 1);
        assert_eq!(v.hit(1024, 512, r.y + r.h + 40), None);
    }

    #[test]
    fn rows_beyond_the_result_count_are_not_hittable() {
        // Only the rows actually drawn may be opened.
        let mut v = SearchView::new();
        v.count = 1;
        let r = row_rect(1024, 2);
        assert_eq!(v.hit(1024, r.x + r.w / 2, r.y + r.h / 2), None);
    }

    #[test]
    fn result_rows_do_not_overlap_the_query_field() {
        let r = field_rect(1024);
        let (fy, fh) = (r.y, r.h);
        let ry = row_rect(1024, 0).y;
        assert!(ry >= fy + fh, "first result overlaps the input");
    }

    #[test]
    fn a_result_carries_the_url_needed_to_open_it() {
        let mut v = SearchView::new();
        v.run("capability agent");
        assert!(v.count > 0);
        assert!(!v.url_at(0).is_empty(), "offline results must be openable too");
    }
}
