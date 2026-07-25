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
use crate::setup::CAPS;
use crate::skills::SkillPeek;
#[cfg(test)]
use crate::skills::BUILTIN;
use crate::ui::theme;

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

use crate::ui::{NAV_H, PAD_X};

/// Column width cap for Skills / Caps / Search (home uses a wider max).
pub const CONTENT_MAX: i32 = 720;
const ROW_H: i32 = 62;
const ROW_GAP: i32 = 8;
const TOP: i32 = 150;

/// Back affordance used by Skills, Caps, Search, and Reader.
pub fn back_rect(w: i32) -> (i32, i32, i32, i32) {
    let _ = w;
    (PAD_X, (NAV_H - 24) / 2, 72, 28)
}

fn column(w: i32) -> (i32, i32) {
    let cw = (w - PAD_X * 2).min(CONTENT_MAX);
    ((w - cw) / 2, cw)
}

/// Bounding box of row `i`, for both drawing and hit-testing.
pub fn row_rect(w: i32, i: usize) -> (i32, i32, i32, i32) {
    let (x, cw) = column(w);
    (x, TOP + i as i32 * (ROW_H + ROW_GAP), cw, ROW_H)
}

/// Which capability row contains this point, if any.
pub fn caps_hit(w: i32, x: i32, y: i32) -> Option<usize> {
    (0..CAPS.len()).find(|&i| {
        let (rx, ry, rw, rh) = row_rect(w, i);
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    })
}

/// Shared top chrome: Back, centered title, rule, optional heading.
pub fn chrome(fb: &Surface, w: i32, title: &str, heading: Option<&str>) {
    fb.fill(theme::BG);
    let (bx, by, _, _) = back_rect(w);
    fb.draw_text(bx, by + BTN_FACE.baseline(), "Back", &BTN_FACE, 0, theme::ACCENT);
    fb.draw_text_centered(
        w / 2,
        (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.baseline(),
        title,
        &BRAND_FACE,
        0,
        theme::INK,
    );
    fb.fill_rect(0, NAV_H, w, 1, theme::RULE);
    if let Some(heading) = heading {
        let track = font::tracking_pct(TITLE_FACE.px, -20);
        fb.draw_text_centered(w / 2, 112, heading, &TITLE_FACE, track, theme::INK);
    }
}

/// A bordered row with a title and a subtitle.
fn row(fb: &Surface, w: i32, i: usize, title: &str, sub: &str, accent: bool) -> (i32, i32, i32, i32) {
    let (x, y, cw, h) = row_rect(w, i);
    let border = if accent { theme::ACCENT } else { theme::CARD_BORDER };
    fb.fill_round_rect(x, y, cw, h, 10, border);
    fb.fill_round_rect(x + 1, y + 1, cw - 2, h - 2, 9, theme::BG);
    fb.draw_text(x + 18, y + 26, title, &BRAND_FACE, 0, theme::INK);
    fb.draw_text(x + 18, y + 46, sub, &SMALL_FACE, 0, theme::MUTED);
    (x, y, cw, h)
}

/// Skills the agent can load — names from bridge `skills.list`, else builtins.
pub fn draw_skills(fb: &Surface, peek: &SkillPeek) {
    let w = fb.width() as i32;
    chrome(fb, w, "Skills", Some("Playbooks the agent can load"));

    let n = peek.count.min(6);
    for i in 0..n {
        let desc = peek.desc_at(i);
        let sub = if desc.is_empty() {
            if peek.from_bridge {
                "From host bridge"
            } else {
                "Shipped with the ISO"
            }
        } else {
            desc
        };
        row(fb, w, i, peek.name_at(i), sub, false);
    }

    let (x, cw) = column(w);
    let note = if peek.from_bridge {
        "Listed live from the host bridge (skills.list)."
    } else if peek.count > 0 {
        "Compiled into the ISO. Saved skills live on the host."
    } else {
        "No skills loaded."
    };
    fb.draw_text(
        x,
        TOP + n as i32 * (ROW_H + ROW_GAP) + 26,
        note,
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
    let _ = cw;
}

/// Which skill row contains this point, if any.
pub fn skills_hit(w: i32, count: usize, x: i32, y: i32) -> Option<usize> {
    (0..count.min(6)).find(|&i| {
        let (rx, ry, rw, rh) = row_rect(w, i);
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    })
}

/// Live capability switches. Clicking a row toggles the grant.
pub fn draw_caps(fb: &Surface, grants: Caps) {
    let w = fb.width() as i32;
    chrome(fb, w, "Capabilities", Some("What the agent may do"));

    for (i, (name, blurb)) in CAPS.iter().enumerate() {
        let on = Cap::ALL
            .get(i)
            .map(|c| grants.allows(*c))
            .unwrap_or(false);
        let (x, y, cw, h) = row(fb, w, i, name, blurb, false);

        // Pill switch, filled when granted.
        let tw = 40;
        let th = 22;
        let tx = x + cw - 18 - tw;
        let ty = y + (h - th) / 2;
        fb.fill_round_rect(tx, ty, tw, th, th / 2, if on { theme::ACCENT } else { theme::RULE });
        let knob = th - 6;
        let kx = if on { tx + tw - knob - 3 } else { tx + 3 };
        fb.fill_round_rect(kx, ty + 3, knob, knob, knob / 2, theme::BG);
    }

    let (x, _) = column(w);
    fb.draw_text(
        x,
        TOP + CAPS.len() as i32 * (ROW_H + ROW_GAP) + 26,
        "Tap a row to grant or revoke. Takes effect immediately.",
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
}

/// Toggle capability `i`, returning the new grant set.
pub fn toggle(grants: Caps, i: usize) -> Caps {
    let mut g = grants;
    if let Some(c) = Cap::ALL.get(i) {
        g.set(*c, !g.allows(*c));
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_stack_without_overlapping() {
        for i in 1..CAPS.len() {
            let (_, prev_y, _, prev_h) = row_rect(1024, i - 1);
            let (_, y, _, _) = row_rect(1024, i);
            assert!(y >= prev_y + prev_h, "row {i} overlaps its predecessor");
        }
    }

    #[test]
    fn every_capability_row_is_hittable_at_its_centre() {
        for i in 0..CAPS.len() {
            let (x, y, w, h) = row_rect(1024, i);
            assert_eq!(caps_hit(1024, x + w / 2, y + h / 2), Some(i));
        }
    }

    #[test]
    fn clicks_between_and_outside_rows_hit_nothing() {
        let (_, y, _, h) = row_rect(1024, 0);
        // In the gap below the first row.
        assert_eq!(caps_hit(1024, 512, y + h + ROW_GAP / 2), None);
        // Left of the column.
        assert_eq!(caps_hit(1024, 2, y + h / 2), None);
        // Above the first row.
        assert_eq!(caps_hit(1024, 512, TOP - 5), None);
    }

    #[test]
    fn toggle_flips_only_the_named_capability() {
        let g = Caps::none();
        let after = toggle(g, 0);
        assert!(after.allows(Cap::ALL[0]));
        assert!(!after.allows(Cap::ALL[1]), "toggling one must not affect another");
        assert!(!toggle(after, 0).allows(Cap::ALL[0]), "must toggle back off");
    }

    #[test]
    fn toggle_out_of_range_is_a_noop() {
        let g = Caps::default_grants();
        assert_eq!(toggle(g, 99).footer_status(), g.footer_status());
    }

    #[test]
    fn caps_screen_labels_match_the_capability_list() {
        // The switch for row i reflects Cap::ALL[i]; a mismatch would show the
        // wrong state against the wrong name.
        assert_eq!(CAPS.len(), Cap::ALL.len());
    }

    #[test]
    fn all_rows_fit_a_768_screen() {
        let n = BUILTIN.len().min(6).max(CAPS.len());
        let (_, y, _, h) = row_rect(1024, n - 1);
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
            "No skills loaded.",
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
        let (_, _, cw, _) = row_rect(1024, 0);
        let peek = SkillPeek::from_builtin();
        for i in 0..peek.count.min(6) {
            assert!(
                BRAND_FACE.width(peek.name_at(i), 0) < cw - 36,
                "name overflows: {}",
                peek.name_at(i)
            );
            assert!(
                SMALL_FACE.width(peek.desc_at(i), 0) < cw - 36,
                "blurb overflows: {}",
                peek.desc_at(i)
            );
        }
    }

    #[test]
    fn skills_hit_finds_first_row() {
        let (x, y, _, _) = row_rect(1024, 0);
        assert_eq!(skills_hit(1024, 3, x + 4, y + 4), Some(0));
        assert_eq!(skills_hit(1024, 0, x + 4, y + 4), None);
    }

    #[test]
    fn capability_text_clears_the_switch() {
        let (_, _, cw, _) = row_rect(1024, 0);
        // Switch occupies the right 58px of the row.
        for (name, blurb) in CAPS {
            assert!(BRAND_FACE.width(name, 0) < cw - 76, "name hits the switch: {name}");
            assert!(SMALL_FACE.width(blurb, 0) < cw - 76, "blurb hits the switch: {blurb}");
        }
    }
}
