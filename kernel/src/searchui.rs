//! The search screen — the first part of this OS you can actually *use*.
//!
//! Type a query, press Enter, read answers. Results come from the index baked
//! into the kernel (`search.rs`), so this works with no host bridge at all;
//! the bridge adds corpus / files / audio when those caps are granted.
//!
//! Previously the home screen ran searches on a card click and wrote the hits
//! to COM1 — invisible unless you were watching a serial console.

use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, SMALL_FACE, TITLE_FACE};
use crate::mcp::DocOutcome;
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
    title: [u8; search::TITLE_CHARS],
    url: [u8; search::URL_CHARS],
    /// Owned like title/url — bridge `cat=` outlives the COM2 line buffer.
    cat: [u8; search::CAT_CHARS],
}

impl Row {
    const fn empty() -> Self {
        Self {
            title: [0; search::TITLE_CHARS],
            url: [0; search::URL_CHARS],
            cat: [0; search::CAT_CHARS],
        }
    }

    fn set(&mut self, title: &str, url: &str, cat: &str) {
        copy_field(&mut self.title, title);
        copy_field(&mut self.url, url);
        copy_field(&mut self.cat, cat);
    }

    fn title(&self) -> &str {
        str_at(&self.title)
    }

    fn url(&self) -> &str {
        str_at(&self.url)
    }

    fn cat(&self) -> &str {
        str_at(&self.cat)
    }
}

/// Shown when something actually broke, as opposed to simply finding nothing.
/// Named for the search engine this OS queries.
const TEDDY: &str = "Teddy is looking into it.";

/// One line explaining an empty result set for a completed query.
fn empty_reason(outcome: DocOutcome) -> &'static str {
    match outcome {
        DocOutcome::Offline => crate::mcp::NO_MATCHES_BRIDGE_OFFLINE,
        // Framed bridge ERR only — grant-miss uses local Ok (no CALL).
        DocOutcome::Err => TEDDY,
        // Do not invent a missing-grant cause — scopes already ran (or were off).
        DocOutcome::Ok => "No matches.",
    }
}

pub struct SearchView {
    rows: [Row; search::MAX_HITS],
    count: usize,
    /// `None` = no query yet. `Some` = last fetch / local-only fill.
    outcome: Option<DocOutcome>,
}

impl SearchView {
    pub const fn new() -> Self {
        Self {
            rows: [Row::empty(); search::MAX_HITS],
            count: 0,
            outcome: None,
        }
    }

    /// Load baked-index hits for `q` into `rows` (does not touch `outcome`).
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

    /// Ask the bridge first; baked index only when COM2 is down (or no search cap).
    ///
    /// Online `Ok`/`Err` with zero ROWs must stay empty — padding ISO hits would
    /// hide `"No matches."` / TEDDY and lie about what the bridge answered.
    pub fn run_via(&mut self, q: &str, caps: crate::caps::Caps) {
        self.count = 0;
        if q.trim().is_empty() {
            // Do not claim the bridge is down — nothing was queried.
            self.outcome = None;
            return;
        }
        // Refuse before CALL: no PING/CALL traffic without the search cap.
        // Local-only fill is Ok — Err/TEDDY is reserved for a framed bridge ERR.
        if !caps.allows(crate::caps::Cap::SearchQuery) {
            self.fill_local(q);
            self.outcome = Some(DocOutcome::Ok);
            return;
        }
        // Record reachability BEFORE any fallback, so an online bridge that
        // simply found nothing is never reported as a connection failure.
        // Rows are filled once from the COM2 parse — no intermediate peek buffer.
        self.outcome = Some(crate::mcp::fetch_search_rows(caps, q, |title, url, cat| {
            if self.count >= search::MAX_HITS {
                return false;
            }
            self.rows[self.count].set(title, url, cat);
            self.count += 1;
            true
        }));
        self.fill_if_offline(q);
    }

    /// Pad baked hits only after a COM2 outage (not after framed Ok/Err).
    fn fill_if_offline(&mut self, q: &str) {
        if self.count == 0 && matches!(self.outcome, Some(DocOutcome::Offline)) {
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
    let (x, cw) = crate::ui::content_column(w, crate::ui::LIST_CONTENT_MAX);
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
    online: bool,
) {
    let w = fb.width() as i32;
    screens::chrome(fb, Some(("Search", "What do you want to know?")));

    // Input field.
    let f = field_rect(w);
    crate::ui::draw_query_field(fb, f, query, "Type a query, then press Enter", "");

    // Results — same geometry as hit-testing (`row_rect`).
    let Some(outcome) = view.outcome else {
        let note = if online {
            "Answers come from the local index and the host bridge."
        } else {
            crate::mcp::BRIDGE_OFFLINE_HINT
        };
        fb.draw_text_centered(w / 2, row_rect(w, 0).y + 30, note, &SMALL_FACE, 0, theme::MUTED);
        return;
    };
    if view.count == 0 {
        fb.draw_text_centered(
            w / 2,
            row_rect(w, 0).y + 30,
            empty_reason(outcome),
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
        // Category chip (baked index or bridge `cat=`).
        let cat = row.cat();
        if !cat.is_empty() {
            let cw = SMALL_FACE.width(cat, 0);
            fb.draw_text(
                rect.x + rect.w - 18 - cw,
                rect.y + 26,
                cat,
                &SMALL_FACE,
                0,
                theme::ACCENT,
            );
        }
        fb.draw_text(rect.x + 18, rect.y + 48, row.url(), &SMALL_FACE, 0, theme::MUTED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_a_query_populates_openable_rows() {
        let mut v = SearchView::new();
        // Caps::none() never PINGs COM2; local Ok + baked fill when search.query is off.
        v.run_via("capability agent", crate::caps::Caps::none());
        assert_eq!(v.outcome, Some(DocOutcome::Ok));
        assert!(v.count > 0, "expected hits from the baked index");
        let (title, url) = v.at(0);
        assert!(!title.is_empty());
        assert!(!url.is_empty(), "offline results must be openable too");
    }

    #[test]
    fn empty_query_stays_idle() {
        let mut v = SearchView::new();
        v.run_via("   ", crate::caps::Caps::none());
        assert!(v.outcome.is_none(), "whitespace is not a bridge outage");
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
        // Grant-miss local empty is Ok — not Err/TEDDY (no CALL failed).
        assert_eq!(v.outcome, Some(DocOutcome::Ok));
    }

    #[test]
    fn online_empty_does_not_pad_baked_hits() {
        let mut v = SearchView::new();
        v.outcome = Some(DocOutcome::Ok);
        v.fill_if_offline("capability agent");
        assert_eq!(v.count, 0, "framed Ok must not invent ISO hits");
        v.outcome = Some(DocOutcome::Err);
        v.fill_if_offline("capability agent");
        assert_eq!(v.count, 0, "framed Err must keep TEDDY empty state");
        v.outcome = Some(DocOutcome::Offline);
        v.fill_if_offline("capability agent");
        assert!(v.count > 0, "Offline still uses the baked index");
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
    fn online_empty_does_not_invent_a_grant_tip() {
        let m = empty_reason(DocOutcome::Ok);
        assert_eq!(m, "No matches.");
        assert!(!m.contains("workspace.index"), "{m}");
        assert!(!m.contains("email.search"), "{m}");
        assert!(!m.contains("offline"), "{m}");
        assert_ne!(m, TEDDY);
    }

    #[test]
    fn every_reason_is_renderable_ascii() {
        assert_eq!(empty_reason(DocOutcome::Err), TEDDY);
        // A bridge that answered with zero hits must not read as offline.
        assert!(!empty_reason(DocOutcome::Ok).contains("offline"));
        let offline = empty_reason(DocOutcome::Offline);
        assert!(offline.contains("offline"));
        assert!(!offline.contains("Teddy"));
        for o in [DocOutcome::Offline, DocOutcome::Ok, DocOutcome::Err] {
            let m = empty_reason(o);
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

    let fx = crate::ui::content_column(w, crate::ui::LIST_CONTENT_MAX).0;
    fb.draw_text(
        fx,
        108,
        page.title(),
        &TITLE_FACE,
        font::TITLE_TRACK,
        theme::INK,
    );

    match page.outcome {
        DocOutcome::Err => {
            fb.draw_text(fx, 160, TEDDY, &BODY_FACE, 0, theme::MUTED);
            return;
        }
        DocOutcome::Offline => {
            fb.draw_text(fx, 160, crate::mcp::BRIDGE_OFFLINE_HINT, &BODY_FACE, 0, theme::MUTED);
            return;
        }
        DocOutcome::Ok if page.count == 0 => {
            fb.draw_text(fx, 160, "Nothing readable here.", &BODY_FACE, 0, theme::MUTED);
            return;
        }
        DocOutcome::Ok => {}
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
