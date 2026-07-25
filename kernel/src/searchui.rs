//! The search screen — the first part of this OS you can actually *use*.
//!
//! Type a query, press Enter, read answers. Results come from the index baked
//! into the kernel (`search.rs`), so this works with no host bridge at all;
//! email is folded in by the bridge when that capability was granted.
//!
//! Previously the home screen ran searches on a card click and wrote the hits
//! to COM1 — invisible unless you were watching a serial console.

use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, BTN_FACE, SMALL_FACE, TITLE_FACE};
use crate::search;
use crate::ui::theme;

/// Longest query we accept. Comfortably wider than the field renders.
pub const QUERY_MAX: usize = 64;

/// Host media extensions the Search field may send to `audio.transcribe`.
/// Keep in sync with `host/bridge` `MEDIA_EXTS` (ASCII, lowercase).
const MEDIA_EXTS: &[&str] = &[
    "wav", "mp3", "m4a", "aac", "flac", "ogg", "opus", "aiff", "mp4", "mov", "mkv",
    "webm", "avi",
];

/// True when `q` looks like an absolute host media path (no spaces).
///
/// Search Enter uses this to call `audio.transcribe` instead of `search.query`
/// when Recordings is granted — the field is the path picker.
pub fn is_media_path(q: &str) -> bool {
    let q = q.trim();
    if q.len() < 3 || q.len() > QUERY_MAX {
        return false;
    }
    if !q.starts_with('/') || q.contains(' ') || q.contains('|') {
        return false;
    }
    let Some(dot) = q.rfind('.') else {
        return false;
    };
    if dot == 0 || dot + 1 >= q.len() {
        return false;
    }
    let ext = &q[dot + 1..];
    let mut buf = [0u8; 8];
    if ext.is_empty() || ext.len() > buf.len() {
        return false;
    }
    for (i, b) in ext.bytes().enumerate() {
        if !b.is_ascii_alphabetic() {
            return false;
        }
        buf[i] = b.to_ascii_lowercase();
    }
    let lower = core::str::from_utf8(&buf[..ext.len()]).unwrap_or("");
    MEDIA_EXTS.contains(&lower)
}

/// File stem of a media path, for a follow-up search after transcribe.
pub fn media_stem(q: &str) -> &str {
    let q = q.trim();
    let name = q.rsplit('/').next().unwrap_or(q);
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// A rendered result row.
///
/// Owns its text: bridge results are parsed out of a COM2 line buffer that is
/// reused on the next call, so borrowing from it would dangle. Copying into
/// fixed slots keeps the whole path free of unsafe lifetime tricks.
#[derive(Clone, Copy)]
pub struct Row {
    title: [u8; 56],
    url: [u8; 72],
    cat: &'static str,
}

impl Row {
    pub const fn empty() -> Self {
        Self { title: [0; 56], url: [0; 72], cat: "" }
    }

    fn set(&mut self, title: &str, url: &str, cat: &'static str) {
        copy_into(&mut self.title, title);
        copy_into(&mut self.url, url);
        self.cat = cat;
    }

    pub fn title(&self) -> &str {
        as_str(&self.title)
    }

    pub fn url(&self) -> &str {
        as_str(&self.url)
    }

    pub fn cat(&self) -> &str {
        self.cat
    }
}

fn copy_into(dst: &mut [u8], src: &str) {
    dst.fill(0);
    let b = src.as_bytes();
    let n = b.len().min(dst.len());
    dst[..n].copy_from_slice(&b[..n]);
}

fn as_str(buf: &[u8]) -> &str {
    let n = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    core::str::from_utf8(&buf[..n]).unwrap_or("")
}

/// What the last query actually did, so the empty state can be truthful.
///
/// A bridge that answers "no matches" is NOT an offline bridge — reporting it
/// as one sent people looking for a connection problem that did not exist.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Source {
    /// COM2 answered.
    pub bridge_online: bool,
    /// The bridge answered with ERR, or the capability was refused.
    pub errored: bool,
    /// The caller held workspace.index, so the user's own files were in scope.
    pub files_in_scope: bool,
    /// The caller held email.search.
    pub mail_in_scope: bool,
    /// The caller held audio.transcribe (recordings in search scope).
    pub audio_in_scope: bool,
}

impl Source {
    pub const fn offline() -> Self {
        Self {
            bridge_online: false,
            errored: false,
            files_in_scope: false,
            mail_in_scope: false,
            audio_in_scope: false,
        }
    }

    /// Shown when something actually broke, as opposed to simply finding
    /// nothing. Named for the search engine this OS queries.
    pub const TEDDY: &'static str = "Teddy is looking into it.";

    /// One line explaining an empty result set, naming the fix when there is one.
    pub fn empty_reason(self) -> &'static str {
        // A failure is not the same as an empty result set, and only the
        // former gets the friendly line.
        if self.errored {
            return Self::TEDDY;
        }
        if !self.bridge_online {
            return "No matches. Bridge offline - only built-in docs are searchable.";
        }
        if !self.files_in_scope {
            return "No matches. Turn on workspace.index in Capabilities to search your files.";
        }
        if !self.mail_in_scope {
            return "No matches in your files. Turn on email.search to include mail.";
        }
        if !self.audio_in_scope {
            return "No matches in files and mail. Turn on audio.transcribe for recordings.";
        }
        // Deliberately claims nothing about WHAT was searched. The guest
        // cannot see whether an index has content, and this line previously
        // asserted "the bridge searched your files and mail" while the
        // workspace index did not exist — permission taken, nothing gained,
        // and a message that lied about it.
        "No matches."
    }
}

pub struct SearchView {
    pub rows: [Row; search::MAX_HITS],
    pub count: usize,
    /// True once a query has been run, so we can tell "no results" from "idle".
    pub searched: bool,
    pub source: Source,
}

impl SearchView {
    pub const fn new() -> Self {
        Self {
            rows: [Row::empty(); search::MAX_HITS],
            count: 0,
            searched: false,
            source: Source::offline(),
        }
    }

    /// Run `q` against the in-kernel index only.
    ///
    /// The baked corpus is a handful of documents about the OS itself, so a
    /// question about the user's own work legitimately finds nothing here.
    pub fn run(&mut self, q: &str) {
        self.searched = true;
        self.count = 0;
        self.source = Source::offline();
        if q.trim().is_empty() {
            return;
        }
        let mut hits = [search::Hit { doc: 0, score: 0 }; search::MAX_HITS];
        let n = search::query(q, &mut hits);
        for h in hits.iter().take(n) {
            let d = &search::DOCS[h.doc];
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
        self.searched = true;
        self.count = 0;
        if q.trim().is_empty() {
            self.source = Source::offline();
            return;
        }
        let peek = crate::mcp::fetch_search_peek(caps, q);
        let online = matches!(peek.status, crate::mcp::BridgeStatus::Online) && !peek.denied;
        // Record reachability BEFORE any fallback, so an online bridge that
        // simply found nothing is never reported as a connection failure.
        let source = Source {
            bridge_online: online,
            errored: peek.denied,
            files_in_scope: caps.allows(crate::caps::Cap::WorkspaceIndex),
            mail_in_scope: caps.allows(crate::caps::Cap::EmailSearch),
            audio_in_scope: caps.allows(crate::caps::Cap::AudioTranscribe),
        };
        if online {
            for i in 0..peek.count.min(search::MAX_HITS) {
                self.rows[self.count].set(peek.title_at(i), peek.url_at(i), "bridge");
                self.count += 1;
            }
        }
        if self.count == 0 {
            // Nothing from the bridge: try what we shipped with.
            self.run(q);
        }
        self.source = source;
    }
}

const PAD_X: i32 = 28;
const NAV_H: i32 = 56;
const FIELD_H: i32 = 52;
const ROW_H: i32 = 64;
const CONTENT_MAX: i32 = 720;

/// Geometry shared by the renderer and hit-testing.
pub fn field_rect(w: i32, h: i32) -> (i32, i32, i32, i32) {
    let _ = h;
    let cw = (w - PAD_X * 2).min(CONTENT_MAX);
    ((w - cw) / 2, 150, cw, FIELD_H)
}

/// Where the "Back" affordance sits.
pub fn back_rect(w: i32) -> (i32, i32, i32, i32) {
    let _ = w;
    (PAD_X, (NAV_H - 24) / 2, 72, 28)
}

/// Bounding box of result row `i`, shared by drawing and hit-testing.
pub fn row_rect(w: i32, h: i32, i: usize) -> (i32, i32, i32, i32) {
    let (fx, fy, fw, fh) = field_rect(w, h);
    (fx, fy + fh + 26 + i as i32 * (ROW_H + 10), fw, ROW_H)
}

/// Which result was clicked, if any.
pub fn result_hit(w: i32, h: i32, count: usize, x: i32, y: i32) -> Option<usize> {
    (0..count.min(search::MAX_HITS)).find(|&i| {
        let (rx, ry, rw, rh) = row_rect(w, h, i);
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    })
}

/// Draw the search screen. `caret` blinks the insertion point on.
pub fn draw(fb: &Surface, view: &SearchView, query: &str, caret: bool, bridge_note: &str) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;
    fb.fill(theme::BG);

    // Nav: a way back, and the brand.
    let (bx, by, bw, bh) = back_rect(w);
    fb.draw_text(bx, by + BTN_FACE.baseline(), "Back", &BTN_FACE, 0, theme::ACCENT);
    let _ = (bw, bh);
    fb.draw_text_centered(w / 2, (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.baseline(), "Search", &BRAND_FACE, 0, theme::INK);
    fb.fill_rect(0, NAV_H, w, 1, theme::RULE);

    let track = font::tracking_pct(TITLE_FACE.px, -20);
    fb.draw_text_centered(w / 2, 112, "What do you want to know?", &TITLE_FACE, track, theme::INK);

    // Input field.
    let (fx, fy, fw, fh) = field_rect(w, h);
    fb.fill_round_rect(fx, fy, fw, fh, 12, theme::RULE);
    fb.fill_round_rect(fx + 1, fy + 1, fw - 2, fh - 2, 11, theme::BG);

    let tx = fx + 18;
    let base = fy + (fh - BODY_FACE.px) / 2 + BODY_FACE.baseline();
    if query.is_empty() {
        fb.draw_text(tx, base, "Type a query or /path.wav", &BODY_FACE, 0, theme::MUTED);
    } else {
        fb.draw_text(tx, base, query, &BODY_FACE, 0, theme::INK);
    }
    if caret {
        let cx = tx + if query.is_empty() { 0 } else { BODY_FACE.width(query, 0) } + 2;
        fb.fill_rect(cx, fy + 14, 2, fh - 28, theme::INK);
    }

    // Results.
    let mut y = fy + fh + 26;
    if !view.searched {
        fb.draw_text_centered(
            w / 2,
            y + 30,
            bridge_note,
            &SMALL_FACE,
            0,
            theme::MUTED,
        );
        return;
    }
    if view.count == 0 {
        fb.draw_text_centered(w / 2, y + 30, view.source.empty_reason(), &BODY_FACE, 0, theme::MUTED);
        return;
    }

    for i in 0..view.count {
        let r = &view.rows[i];
        fb.fill_round_rect(fx, y, fw, ROW_H, 10, theme::CARD_BORDER);
        fb.fill_round_rect(fx + 1, y + 1, fw - 2, ROW_H - 2, 9, theme::BG);
        fb.draw_text(fx + 18, y + 26, r.title(), &BRAND_FACE, 0, theme::INK);
        // Category chip, right-aligned.
        let cw = SMALL_FACE.width(r.cat(), 0);
        fb.draw_text(fx + fw - 18 - cw, y + 26, r.cat(), &SMALL_FACE, 0, theme::ACCENT);
        fb.draw_text(fx + 18, y + 48, r.url(), &SMALL_FACE, 0, theme::MUTED);
        y += ROW_H + 10;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_paths_are_recognised() {
        assert!(is_media_path("/tmp/rec.wav"));
        assert!(is_media_path("/Users/a/Desktop/note.MP3"));
        assert!(!is_media_path("rec.wav"), "must be absolute");
        assert!(!is_media_path("/tmp/notes.md"));
        assert!(!is_media_path("/tmp/has space.wav"));
        assert_eq!(media_stem("/tmp/os-smoke-rec.wav"), "os-smoke-rec");
    }

    #[test]
    fn idle_view_reports_nothing_searched() {
        let v = SearchView::new();
        assert!(!v.searched);
        assert_eq!(v.count, 0);
    }

    #[test]
    fn running_a_query_populates_rows() {
        let mut v = SearchView::new();
        v.run("capability agent");
        assert!(v.searched);
        assert!(v.count > 0, "expected hits from the baked index");
        assert!(!v.rows[0].title().is_empty());
    }

    #[test]
    fn empty_query_searches_nothing_but_marks_searched() {
        let mut v = SearchView::new();
        v.run("   ");
        assert!(v.searched, "must distinguish 'ran and found nothing' from idle");
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
        let (x, _y, w, _h) = field_rect(1024, 768);
        assert!(x >= PAD_X);
        assert!(x + w <= 1024 - PAD_X);
    }

    #[test]
    fn results_fit_below_the_field_at_768() {
        let (_x, fy, _w, fh) = field_rect(1024, 768);
        let bottom = fy + fh + 26 + (search::MAX_HITS as i32) * (ROW_H + 10);
        assert!(bottom < 768, "results run off a 768px screen: {bottom}");
    }

    #[test]
    fn back_target_is_clickable_sized() {
        let (_x, _y, w, h) = back_rect(1024);
        assert!(w >= 44 && h >= 24, "back target too small to hit");
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn an_online_bridge_is_never_reported_as_offline() {
        // The bug this replaces: a bridge that answered "n=0" was rendered as
        // "Bridge offline", sending the user to debug a working connection.
        let s = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: true,
            mail_in_scope: true,
            audio_in_scope: true,
        };
        assert!(!s.empty_reason().contains("offline"));
    }

    #[test]
    fn a_real_outage_still_says_offline() {
        assert!(Source::offline().empty_reason().contains("offline"));
    }

    #[test]
    fn missing_file_grant_names_the_fix() {
        let s = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: false,
            mail_in_scope: true,
            audio_in_scope: false,
        };
        let m = s.empty_reason();
        assert!(m.contains("workspace.index"), "{m}");
        assert!(!m.contains("offline"), "{m}");
    }

    #[test]
    fn missing_mail_grant_names_the_fix() {
        let s = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: true,
            mail_in_scope: false,
            audio_in_scope: false,
        };
        assert!(s.empty_reason().contains("email.search"));
    }

    #[test]
    fn missing_audio_grant_names_the_fix() {
        let s = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: true,
            mail_in_scope: true,
            audio_in_scope: false,
        };
        assert!(s.empty_reason().contains("audio.transcribe"));
    }

    #[test]
    fn every_reason_is_renderable_ascii() {
        for s in [
            Source::offline(),
            Source {
                bridge_online: true,
                errored: false,
                files_in_scope: false,
                mail_in_scope: false,
                audio_in_scope: false,
            },
            Source {
                bridge_online: true,
                errored: false,
                files_in_scope: true,
                mail_in_scope: false,
                audio_in_scope: false,
            },
            Source {
                bridge_online: true,
                errored: false,
                files_in_scope: true,
                mail_in_scope: true,
                audio_in_scope: false,
            },
            Source {
                bridge_online: true,
                errored: false,
                files_in_scope: true,
                mail_in_scope: true,
                audio_in_scope: true,
            },
        ] {
            let m = s.empty_reason();
            assert!(m.bytes().all(|b| (0x20..=0x7E).contains(&b)), "{m}");
            assert!(BODY_FACE.width(m, 0) < 980, "empty-state line overflows: {m}");
        }
    }
}

#[cfg(test)]
mod teddy_tests {
    use super::*;

    #[test]
    fn a_real_failure_gets_the_friendly_line() {
        let s = Source {
            bridge_online: true,
            errored: true,
            files_in_scope: true,
            mail_in_scope: true,
            audio_in_scope: true,
        };
        assert_eq!(s.empty_reason(), Source::TEDDY);
    }

    #[test]
    fn an_empty_result_is_not_a_failure() {
        // Finding nothing is a legitimate answer and must stay actionable
        // rather than being papered over with a mascot.
        let s = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: false,
            mail_in_scope: true,
            audio_in_scope: false,
        };
        assert_ne!(s.empty_reason(), Source::TEDDY);
        assert!(s.empty_reason().contains("workspace.index"));
    }

    #[test]
    fn an_outage_still_says_offline_not_teddy() {
        assert!(Source::offline().empty_reason().contains("offline"));
    }

    #[test]
    fn the_line_renders() {
        assert!(Source::TEDDY.bytes().all(|b| (0x20..=0x7E).contains(&b)));
        assert!(BODY_FACE.width(Source::TEDDY, 0) < 980);
    }
}

/// Draw a document the user opened from a result.
pub fn draw_reader(fb: &Surface, title: &str, page: &crate::mcp::DocPage) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;
    fb.fill(theme::BG);

    let (bx, by, _, _) = back_rect(w);
    fb.draw_text(bx, by + BTN_FACE.baseline(), "Back", &BTN_FACE, 0, theme::ACCENT);
    fb.fill_rect(0, NAV_H, w, 1, theme::RULE);

    let (fx, _, fw, _) = field_rect(w, h);
    fb.draw_text(fx, 108, title, &TITLE_FACE, font::tracking_pct(TITLE_FACE.px, -20), theme::INK);

    if page.denied {
        let msg = page.deny.message();
        let msg = if msg.is_empty() {
            "Could not open this document."
        } else {
            msg
        };
        fb.draw_text(fx, 160, msg, &BODY_FACE, 0, theme::MUTED);
        return;
    }
    if page.count == 0 {
        let msg = match page.status {
            crate::mcp::BridgeStatus::Online => "Nothing readable here.",
            crate::mcp::BridgeStatus::Offline => "Bridge offline - cannot open documents.",
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
    let _ = fw;
}

#[cfg(test)]
mod reader_tests {
    use super::*;

    #[test]
    fn denied_reader_names_the_missing_grant() {
        let mut page = crate::mcp::DocPage::empty(crate::mcp::BridgeStatus::Online, true);
        page.deny = crate::mcp::DocDeny::NeedFiles;
        assert!(page.deny.message().contains("Your files"));
        page.deny = crate::mcp::DocDeny::NeedEmail;
        assert!(page.deny.message().contains("Email"));
        // Not the search empty-state mascot — open failures are grant issues.
        assert_ne!(page.deny.message(), Source::TEDDY);
    }

    #[test]
    fn result_rows_are_clickable_at_their_centre() {
        for i in 0..search::MAX_HITS {
            let (x, y, w, h) = row_rect(1024, 768, i);
            assert_eq!(result_hit(1024, 768, search::MAX_HITS, x + w / 2, y + h / 2), Some(i));
        }
    }

    #[test]
    fn clicks_below_the_last_result_open_nothing() {
        let (_, y, _, h) = row_rect(1024, 768, search::MAX_HITS - 1);
        assert_eq!(result_hit(1024, 768, search::MAX_HITS, 512, y + h + 40), None);
    }

    #[test]
    fn rows_beyond_the_result_count_are_not_hittable() {
        // Only the rows actually drawn may be opened.
        let (x, y, w, h) = row_rect(1024, 768, 2);
        assert_eq!(result_hit(1024, 768, 1, x + w / 2, y + h / 2), None);
    }

    #[test]
    fn result_rows_do_not_overlap_the_query_field() {
        let (_, fy, _, fh) = field_rect(1024, 768);
        let (_, ry, _, _) = row_rect(1024, 768, 0);
        assert!(ry >= fy + fh, "first result overlaps the input");
    }

    #[test]
    fn a_result_carries_the_url_needed_to_open_it() {
        let mut v = SearchView::new();
        v.run("capability agent");
        assert!(v.count > 0);
        assert!(!v.rows[0].url().is_empty(), "offline results must be openable too");
    }
}

#[cfg(test)]
mod honesty_tests {
    use super::*;

    #[test]
    fn the_all_granted_message_claims_nothing_it_cannot_verify() {
        // The guest cannot tell whether an index has content, so it must not
        // say what was searched — only that nothing matched.
        let s = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: true,
            mail_in_scope: true,
            audio_in_scope: true,
        };
        let m = s.empty_reason();
        assert!(!m.contains("searched"), "claims knowledge it does not have: {m}");
        assert!(m.contains("No matches"));
    }

    #[test]
    fn actionable_states_still_name_the_missing_grant() {
        // Softening the all-granted line must not soften the useful ones.
        let no_files = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: false,
            mail_in_scope: true,
            audio_in_scope: false,
        };
        assert!(no_files.empty_reason().contains("workspace.index"));
        let no_audio = Source {
            bridge_online: true,
            errored: false,
            files_in_scope: true,
            mail_in_scope: true,
            audio_in_scope: false,
        };
        assert!(no_audio.empty_reason().contains("audio.transcribe"));
    }
}
