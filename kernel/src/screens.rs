//! The Skills and Capabilities screens.
//!
//! Both cards on the home page used to just log to COM1. They now open real
//! views that share the search screen's chrome: a Back affordance, a title,
//! and a column of rows.
//!
//! Capabilities is not merely a report — the switches are live, so grants
//! chosen during setup can be changed afterwards without reinstalling.

use crate::caps::{Cap, Caps};
use crate::fb::Surface;
use crate::font::{self, BRAND_FACE, BTN_FACE, SMALL_FACE, TITLE_FACE};
use crate::setup::CAP_BLURBS;
use crate::skills::SkillPeek;
#[cfg(test)]
use crate::skills::BUILTIN;
use crate::ui::{self, theme, NAV_H, PAD_X};

/// Which full-screen view is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Home,
    Search,
    Skills,
    Caps,
    /// Reading a document opened from a search result.
    Reader,
}

/// Column width cap for Skills / Caps / Search (home uses a wider max).
const CONTENT_MAX: i32 = 720;
const ROW_H: i32 = 62;
const ROW_GAP: i32 = 8;
const TOP: i32 = 150;

/// Back affordance used by Skills, Caps, Search, and Reader.
fn back_rect() -> ui::Rect {
    ui::Rect::new(PAD_X, (NAV_H - 24) / 2, 72, 28)
}

/// Whether `(x, y)` hits the shared Back control.
pub fn back_hit(x: i32, y: i32) -> bool {
    back_rect().contains(x, y)
}

pub(crate) fn column(w: i32) -> (i32, i32) {
    ui::content_column(w, CONTENT_MAX)
}

/// Bounding box of row `i`, for both drawing and hit-testing.
fn row_rect(w: i32, i: usize) -> ui::Rect {
    let (x, cw) = column(w);
    ui::Rect::new(x, TOP + i as i32 * (ROW_H + ROW_GAP), cw, ROW_H)
}

/// Which capability row contains this point, if any.
pub fn caps_hit(w: i32, x: i32, y: i32) -> Option<usize> {
    ui::hit_among(Cap::ALL.len(), x, y, |i| row_rect(w, i))
}

/// Shared top chrome: Back, rule, and optional title + heading (always paired).
pub(crate) fn chrome(fb: &Surface, label: Option<(&str, &str)>) {
    let w = fb.width() as i32;
    fb.fill(theme::BG);
    let back = back_rect();
    fb.draw_text(back.x, back.y + BTN_FACE.ascent, "Back", &BTN_FACE, 0, theme::ACCENT);
    if let Some((title, _)) = label {
        fb.draw_text_centered(
            w / 2,
            (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.ascent,
            title,
            &BRAND_FACE,
            0,
            theme::INK,
        );
    }
    fb.fill_rect(0, NAV_H, w, 1, theme::RULE);
    if let Some((_, heading)) = label {
        let track = font::tracking_pct(TITLE_FACE.px, -20);
        fb.draw_text_centered(w / 2, 112, heading, &TITLE_FACE, track, theme::INK);
    }
}

/// A bordered row with a title and a subtitle.
fn row(fb: &Surface, w: i32, i: usize, title: &str, sub: &str) -> ui::Rect {
    let r = row_rect(w, i);
    ui::draw_titled_row(fb, r, title, sub);
    r
}

/// Skills the agent can load — names from bridge `skills.list`, else builtins.
pub fn draw_skills(fb: &Surface, peek: &SkillPeek) {
    let w = fb.width() as i32;
    chrome(fb, Some(("Skills", "Playbooks the agent can load")));

    let n = peek.count.min(6);
    for i in 0..n {
        row(fb, w, i, peek.name_at(i), peek.subtitle_at(i));
    }

    let (x, _) = column(w);
    // Boot / fetch always leave at least the ISO builtins, so the empty case
    // never reaches the screen.
    let note = if peek.from_bridge {
        "Listed live from the host bridge (skills.list)."
    } else {
        "Compiled into the ISO. Saved skills live on the host."
    };
    fb.draw_text(
        x,
        TOP + n as i32 * (ROW_H + ROW_GAP) + 26,
        note,
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
}

/// Live capability switches. Clicking a row toggles the grant.
pub fn draw_caps(fb: &Surface, grants: Caps) {
    let w = fb.width() as i32;
    chrome(fb, Some(("Capabilities", "What the agent may do")));

    for (i, cap) in Cap::ALL.iter().enumerate() {
        let on = grants.allows(*cap);
        let r = row(fb, w, i, cap.name(), CAP_BLURBS[i]);
        ui::draw_switch_in_row(fb, r, on);
    }

    let (x, _) = column(w);
    fb.draw_text(
        x,
        TOP + Cap::ALL.len() as i32 * (ROW_H + ROW_GAP) + 26,
        "Tap a row to grant or revoke. Takes effect immediately.",
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_stack_without_overlapping() {
        for i in 1..Cap::ALL.len() {
            let prev = row_rect(1024, i - 1);
            let cur = row_rect(1024, i);
            assert!(cur.y >= prev.y + prev.h, "row {i} overlaps its predecessor");
        }
    }

    #[test]
    fn every_capability_row_is_hittable_at_its_centre() {
        for i in 0..Cap::ALL.len() {
            let r = row_rect(1024, i);
            assert_eq!(caps_hit(1024, r.x + r.w / 2, r.y + r.h / 2), Some(i));
        }
    }

    #[test]
    fn clicks_between_and_outside_rows_hit_nothing() {
        let r = row_rect(1024, 0);
        let (y, h) = (r.y, r.h);
        // In the gap below the first row.
        assert_eq!(caps_hit(1024, 512, y + h + ROW_GAP / 2), None);
        // Left of the column.
        assert_eq!(caps_hit(1024, 2, y + h / 2), None);
        // Above the first row.
        assert_eq!(caps_hit(1024, 512, TOP - 5), None);
    }

    #[test]
    fn back_target_is_clickable_sized() {
        let b = back_rect();
        let (w, h) = (b.w, b.h);
        assert!(w >= 44 && h >= 24, "back target too small to hit");
    }

    #[test]
    fn all_rows_fit_a_768_screen() {
        let n = BUILTIN.len().min(6).max(Cap::ALL.len());
        let r = row_rect(1024, n - 1);
        let (y, h) = (r.y, r.h);
        assert!(y + h + 40 < 768, "rows run off the screen: {}", y + h);
    }

    #[test]
    fn copy_is_ascii_only() {
        let mut all = vec![
            "Playbooks the agent can load",
            "What the agent may do",
            "Compiled into the ISO. Saved skills live on the host.",
            "Listed live from the host bridge (skills.list).",
            "Tap a row to grant or revoke. Takes effect immediately.",
            "Skills",
            "Capabilities",
            "Back",
            "From host bridge",
            "Shipped with the ISO",
        ];
        for s in BUILTIN {
            all.push(s.name);
            all.push(s.blurb);
        }
        for s in all {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII renders as '?': {s:?}"
            );
        }
    }

    #[test]
    fn skill_text_fits_its_row() {
        let cw = row_rect(1024, 0).w;
        let peek = SkillPeek::from_builtin();
        for i in 0..peek.count.min(6) {
            assert!(
                BRAND_FACE.width(peek.name_at(i), 0) < cw - 36,
                "name overflows: {}",
                peek.name_at(i)
            );
            assert!(
                SMALL_FACE.width(peek.subtitle_at(i), 0) < cw - 36,
                "blurb overflows: {}",
                peek.subtitle_at(i)
            );
        }
    }

    #[test]
    fn capability_text_clears_the_switch() {
        let cw = row_rect(1024, 0).w;
        // Switch occupies the right 58px of the row.
        for (i, cap) in Cap::ALL.iter().enumerate() {
            let name = cap.name();
            let blurb = CAP_BLURBS[i];
            assert!(BRAND_FACE.width(name, 0) < cw - 76, "name hits the switch: {name}");
            assert!(SMALL_FACE.width(blurb, 0) < cw - 76, "blurb hits the switch: {blurb}");
        }
    }
}
