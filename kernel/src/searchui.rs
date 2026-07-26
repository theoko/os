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
    /// Bridge answered; which scopes were granted at query time.
    Online { files: bool, mail: bool },
}

impl Phase {
    /// One line explaining an empty result set, naming the fix when there is one.
    fn empty_reason(self) -> &'static str {
        match self {
            // Idle is exhaustive only; draw never paints empty_reason while idle.
            Phase::Idle | Phase::Offline => crate::mcp::NO_MATCHES_BRIDGE_OFFLINE,
            Phase::Denied => TEDDY,
            Phase::Online { files, mail } => {
                if !files {
                    return "No matches. Turn on workspace.index in Capabilities to search your files.";
                }
                if !mail {
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
        let mut hits = [0usize; search::MAX_HITS];
        let n = search::query(q, &mut hits);
        for &doc in hits.iter().take(n) {
            let d = &search::DOCS[doc];
            self.rows[self.count].set(d.title, d.url, d.cat);
            self.count += 1;
        }
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
        // Record reachability BEFORE any fallback, so an online bridge that
        // simply found nothing is never reported as a connection failure.
        // Rows are filled once from the COM2 parse — no intermediate peek buffer.
        if crate::mcp::fetch_search_rows(caps, q, |title, url| {
            if self.count >= search::MAX_HITS {
                return false;
            }
            self.rows[self.count].set(title, url, "bridge");
            self.count += 1;
            true
        }) {

            use crate::caps::Cap;
            self.phase = Phase::Online {
                files: caps.allows(Cap::WorkspaceIndex),
                mail: caps.allows(Cap::EmailSearch),
            };
        } else {
            self.phase = Phase::Offline;
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

    pub fn at(&self, i: usize) -> (&str, &str) {
        (self.rows[i].title(), self.rows[i].url())
    }
}

const ROW_H: i32 = 64;

/// Geometry shared by the renderer and hit-testing.
fn field_rect(w: i32) -> crate::ui::Rect {
    let (x, cw) = crate::ui::content_column(w, screens::CONTENT_MAX);
    crate::ui::Rect::new(x, crate::ui::LIST_TOP, cw, crate::ui::FIELD_H)
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
    screens::chrome(fb, Some(("Search", "What do you want to know?")));

    // Input field.
    let f = field_rect(w);
    crate::ui::draw_query_field(fb, f, query, "Type a query, then press Enter", "");

    // Results — same geometry as hit-testing (`row_rect`).
    if view.phase == Phase::Idle {
        let note = match status {
            crate::mcp::BridgeStatus::Online => {
                "Answers come from the local index and the host bridge."
            }
            crate::mcp::BridgeStatus::Offline => crate::mcp::BRIDGE_OFFLINE_HINT,
        };
        fb.draw_text_centered(w / 2, row_rect(w, 0).y + 30, note, &SMALL_FACE, 0, theme::MUTED);
        return;
    }
    if view.count == 0 {
        fb.draw_text_centered(
            w / 2,
            row_rect(w, 0).y + 30,
            view.phase.empty_reason(),
            &BODY_FACE,
            0,
            theme::MUTED,
        );
        return;
    }

    for i in 0..view.count {
        let rect = row_rect(w, i);
        let row = &view.rows[i];
        crate::ui::outlined_round_rect(fb, rect, 10);
        fb.draw_text(rect.x + 18, rect.y + 26, row.title(), &BRAND_FACE, 0, theme::INK);
        // Category chip, right-aligned.
        let cw = SMALL_FACE.width(row.cat, 0);
        fb.draw_text(rect.x + rect.w - 18 - cw, rect.y + 26, row.cat, &SMALL_FACE, 0, theme::ACCENT);
        fb.draw_text(rect.x + 18, rect.y + 48, row.url(), &SMALL_FACE, 0, theme::MUTED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_a_query_populates_openable_rows() {
        let mut v = SearchView::new();
        // Caps::none() never PINGs COM2; Denied + baked fill exercises the
        // offline index the same way production does when search.query is off.
        v.run_via("capability agent", crate::caps::Caps::none());
        assert_ne!(v.phase, Phase::Idle);
        assert!(v.count > 0, "expected hits from the baked index");
        let (title, url) = v.at(0);
        assert!(!title.is_empty());
        assert!(!url.is_empty(), "offline results must be openable too");
    }

    #[test]
    fn empty_query_searches_nothing_but_marks_searched() {
        let mut v = SearchView::new();
        v.run_via("   ", crate::caps::Caps::none());
        assert_ne!(
            v.phase,
            Phase::Idle,
            "must distinguish 'ran and found nothing' from idle"
        );
        assert_eq!(v.count, 0);
    }

    #[test]
    fn rerunning_replaces_previous_results() {
        let mut v = SearchView::new();
        v.run_via("capability agent", crate::caps::Caps::none());
        let first = v.count;
        assert!(first > 0);
        v.run_via("zzzz qqqq", crate::caps::Caps::none());
        assert_eq!(v.count, 0, "stale rows must not survive a new search");
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

    #[test]
    fn missing_grants_name_the_fix() {
        // Finding nothing is a legitimate answer and must stay actionable
        // rather than being papered over with a mascot.
        let m = Phase::Online { files: false, mail: true }.empty_reason();
        assert!(m.contains("workspace.index"), "{m}");
        assert!(!m.contains("offline"), "{m}");
        assert_ne!(m, TEDDY);
        assert!(
            Phase::Online { files: true, mail: false }
                .empty_reason()
                .contains("email.search")
        );
    }

    #[test]
    fn every_reason_is_renderable_ascii() {
        assert_eq!(Phase::Denied.empty_reason(), TEDDY);
        // A bridge that answered n=0 must not read as a connection failure.
        assert!(
            !Phase::Online { files: true, mail: true }
                .empty_reason()
                .contains("offline")
        );
        let offline = Phase::Offline.empty_reason();
        assert!(offline.contains("offline"));
        assert!(!offline.contains("Teddy"));
        for s in [
            Phase::Offline,
            Phase::Online { files: false, mail: false },
            Phase::Online { files: true, mail: false },
            Phase::Online { files: true, mail: true },
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
    screens::chrome(fb, None);

    let fx = crate::ui::content_column(w, screens::CONTENT_MAX).0;
    fb.draw_text(
        fx,
        108,
        page.title(),
        &TITLE_FACE,
        font::TITLE_TRACK,
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

}
