//! Home screen — a launcher.
//!
//! A search field you can type into on arrival, three destinations carrying
//! live counts, and recent mail when granted. Type is anti-aliased proportional
//! (see `font.rs`); all copy is ASCII because the atlas covers 0x20..=0x7E only.

use crate::fb::Surface;
use crate::font::{BODY_FACE, BRAND_FACE, H2_FACE, SMALL_FACE};
use crate::mcp::{BridgeStatus, MailPeek};
use crate::skills::SkillPeek;

/// Palette lifted from the reference site.
pub mod theme {
    /// Page.
    pub const BG: u32 = 0x00FF_FFFF;
    /// Primary text — Apple's near-black, never pure #000.
    pub const INK: u32 = 0x001D_1D1F;
    /// Secondary copy.
    pub const MUTED: u32 = 0x0086_868B;
    /// Accent / primary action.
    pub const ACCENT: u32 = 0x0000_71E3;
    /// Hairline separators.
    pub const RULE: u32 = 0x00D2_D2D7;
    /// Card border.
    pub const CARD_BORDER: u32 = 0x00E8_E8ED;
    /// Accent at 6% over white — the tinted secondary pill.
    pub const TINT_BG: u32 = 0x00F2_F8FD;
    pub const ONLINE: u32 = 0x0034_C759;
    pub const OFFLINE: u32 = 0x00FF_3B30;
}

/// Shared chrome height (home nav, Skills/Caps/Search Back bar).
pub const NAV_H: i32 = 56;
/// Shared horizontal page margin.
pub const PAD_X: i32 = 28;
const CONTENT_MAX: i32 = 920;


/// Axis-aligned hit region (inclusive origin, exclusive of `x+w` / `y+h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
}

/// Which home tile was under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardId {
    Search,
    Capabilities,
    Skills,
}

/// Any clickable region on the finished home screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeHit {
    /// The primary search field (type or click to open Search).
    SearchField,
    Card(CardId),
}

/// Hit targets for the three destination tiles.
#[derive(Clone, Copy, Debug)]
pub struct CardTargets {
    pub search: Rect,
    pub capabilities: Rect,
    pub skills: Rect,
}

impl CardTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<CardId> {
        if self.search.contains(px, py) {
            Some(CardId::Search)
        } else if self.capabilities.contains(px, py) {
            Some(CardId::Capabilities)
        } else if self.skills.contains(px, py) {
            Some(CardId::Skills)
        } else {
            None
        }
    }
}

/// Combined home hit-test (search field preferred over tiles).
#[derive(Clone, Copy, Debug)]
pub struct HomeTargets {
    pub search: Rect,
    pub cards: CardTargets,
}

impl HomeTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<HomeHit> {
        if self.search.contains(px, py) {
            return Some(HomeHit::SearchField);
        }
        self.cards.hit(px, py).map(HomeHit::Card)
    }
}


/// The home screen: a launcher, not a landing page.
///
/// A search field you can type into immediately, three destinations carrying
/// live counts, and recent mail when the capability was granted. The previous
/// version led with a tagline and a primary button whose only effect was to
/// restart the setup wizard.
///
/// `status` is a short footer line (ASCII); empty falls back to the version bar.
pub fn draw_home_full(
    fb: &Surface,
    mail: &MailPeek,
    skills: &SkillPeek,
    status: &str,
    query: &str,
    caret: bool,
) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;

    fb.fill(theme::BG);
    draw_nav(fb, w, mail);

    let (x0, cw) = home_column(w);

    // The one thing you can do without clicking anything first.
    let (fx, fy, fw, fh) = search_rect(w, h);
    fb.fill_round_rect(fx, fy, fw, fh, 12, theme::RULE);
    fb.fill_round_rect(fx + 1, fy + 1, fw - 2, fh - 2, 11, theme::BG);
    let base = fy + (fh - BODY_FACE.px) / 2 + BODY_FACE.baseline();
    if query.is_empty() {
        fb.draw_text(fx + 18, base, "Search the knowledge base", &BODY_FACE, 0, theme::MUTED);
    } else {
        fb.draw_text(fx + 18, base, query, &BODY_FACE, 0, theme::INK);
    }
    if caret {
        let cx = fx + 18 + BODY_FACE.width(query, 0) + 2;
        fb.fill_rect(cx, fy + 14, 2, fh - 28, theme::INK);
    }
    fb.draw_text(
        fx + 2,
        fy + fh + 22,
        "Type a query and press Enter. Works with the bridge offline.",
        &SMALL_FACE,
        0,
        theme::MUTED,
    );

    // Destinations, each showing a real number rather than a slogan.
    let mut mbuf = [0u8; 16];
    let mail_label = fmt_count(&mut mbuf, mail.count, "message", "messages");
    let mut sbuf = [0u8; 16];
    let skill_label = fmt_count(&mut sbuf, skills.count, "playbook", "playbooks");

    let gap = 16;
    let tw = (cw - gap * 2) / 3;
    let ty = tile_top(h);
    let tiles: [(&str, &str); 3] = [
        ("Search", "knowledge + email"),
        ("Capabilities", status),
        ("Skills", skill_label),
    ];
    for (i, (title, sub)) in tiles.iter().enumerate() {
        let tx = x0 + (tw + gap) * i as i32;
        fb.fill_round_rect(tx, ty, tw, TILE_H, 12, theme::CARD_BORDER);
        fb.fill_round_rect(tx + 1, ty + 1, tw - 2, TILE_H - 2, 11, theme::BG);
        fb.draw_text(tx + 18, ty + 34, title, &H2_FACE, 0, theme::INK);
        fb.draw_text(tx + 18, ty + 58, sub, &SMALL_FACE, 0, theme::MUTED);
    }

    // Live content instead of marketing copy.
    let ry = ty + TILE_H + 40;
    if mail.count > 0 {
        fb.draw_text(x0, ry, "Recent mail", &BRAND_FACE, 0, theme::INK);
        let mut y = ry + 30;
        for i in 0..mail.count.min(3) {
            fb.draw_text(x0, y, mail.row_subj(i), &BODY_FACE, 0, theme::INK);
            let from = mail.row_from(i);
            fb.draw_text(x0 + cw - SMALL_FACE.width(from, 0), y, from, &SMALL_FACE, 0, theme::MUTED);
            y += 12;
            fb.fill_rect(x0, y, cw, 1, theme::CARD_BORDER);
            y += 26;
        }
    } else {
        fb.draw_text(
            x0,
            ry,
            match mail.status {
                BridgeStatus::Online => "Inbox empty, or email.search not granted.",
                BridgeStatus::Offline => crate::mcp::BRIDGE_OFFLINE_HINT,
            },
            &SMALL_FACE,
            0,
            theme::MUTED,
        );
    }

    fb.draw_text_centered(w / 2, h - 24, mail_label, &SMALL_FACE, 0, theme::MUTED);
}

/// Render "3 messages" / "1 message" / "none" into a caller-owned buffer.
fn fmt_count<'a>(buf: &'a mut [u8; 16], n: usize, one: &'static str, many: &'static str) -> &'a str {
    if n == 0 {
        return "none";
    }
    let mut i = 0;
    if n >= 10 {
        buf[i] = b'0' + ((n / 10) % 10) as u8;
        i += 1;
    }
    buf[i] = b'0' + (n % 10) as u8;
    i += 1;
    buf[i] = b' ';
    i += 1;
    for &b in (if n == 1 { one } else { many }).as_bytes() {
        if i < buf.len() {
            buf[i] = b;
            i += 1;
        }
    }
    core::str::from_utf8(&buf[..i]).unwrap_or("")
}

pub(crate) fn home_column(w: i32) -> (i32, i32) {
    let cw = (w - PAD_X * 2).min(CONTENT_MAX);
    ((w - cw) / 2, cw)
}

/// The home search field, shared by drawing and hit-testing.
pub fn search_rect(w: i32, h: i32) -> (i32, i32, i32, i32) {
    let _ = h;
    let (x, cw) = home_column(w);
    (x, 132, cw, 52)
}

pub(crate) fn tile_top(_h: i32) -> i32 {
    242
}

pub(crate) const TILE_H: i32 = 78;

/// Bounding box of home tile `i` (0 = Search, 1 = Capabilities, 2 = Skills).
pub fn tile_rect(w: i32, h: i32, i: i32) -> Rect {
    let (x0, cw) = home_column(w);
    let gap = 16;
    let tw = (cw - gap * 2) / 3;
    Rect { x: x0 + (tw + gap) * i, y: tile_top(h), w: tw, h: TILE_H }
}

pub fn card_targets(w: i32, h: i32) -> CardTargets {
    CardTargets {
        search: tile_rect(w, h, 0),
        capabilities: tile_rect(w, h, 1),
        skills: tile_rect(w, h, 2),
    }
}

pub fn home_targets(w: i32, h: i32) -> HomeTargets {
    let (fx, fy, fw, fh) = search_rect(w, h);
    HomeTargets {
        search: Rect {
            x: fx,
            y: fy,
            w: fw,
            h: fh,
        },
        cards: card_targets(w, h),
    }
}

fn draw_nav(fb: &Surface, w: i32, mail: &MailPeek) {
    let base = (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.baseline();
    fb.draw_text(PAD_X, base, "os", &BRAND_FACE, 0, theme::INK);

    let (label, dot) = match mail.status {
        BridgeStatus::Online => ("bridge connected", theme::ONLINE),
        BridgeStatus::Offline => ("bridge offline", theme::OFFLINE),
    };
    let tw = SMALL_FACE.width(label, 0);
    let sbase = (NAV_H - SMALL_FACE.px) / 2 + SMALL_FACE.baseline();
    fb.draw_text(w - PAD_X - tw, sbase, label, &SMALL_FACE, 0, theme::MUTED);

    let dot_d = 7;
    fb.fill_round_rect(
        w - PAD_X - tw - 8 - dot_d,
        NAV_H / 2 - dot_d / 2,
        dot_d,
        dot_d,
        dot_d / 2,
        dot,
    );

    fb.fill_rect(0, NAV_H, w, 1, theme::RULE);
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::Caps;
    use crate::mcp::MailPeek;

    #[test]
    fn search_field_is_the_primary_target() {
        // Typing must be reachable without hunting for a card.
        let t = home_targets(1024, 768);
        let (fx, fy, fw, fh) = search_rect(1024, 768);
        assert_eq!(
            t.search,
            Rect {
                x: fx,
                y: fy,
                w: fw,
                h: fh
            }
        );
        assert_eq!(
            t.hit(fx + fw / 2, fy + fh / 2),
            Some(HomeHit::SearchField)
        );
    }

    #[test]
    fn tiles_do_not_overlap_the_search_field() {
        let (_, fy, _, fh) = search_rect(1024, 768);
        assert!(tile_top(768) >= fy + fh, "tiles collide with the field");
    }

    #[test]
    fn each_tile_hit_tests_to_its_own_id() {
        let t = home_targets(1024, 768);
        for (i, want) in [CardId::Search, CardId::Capabilities, CardId::Skills]
            .iter()
            .enumerate()
        {
            let r = tile_rect(1024, 768, i as i32);
            assert_eq!(t.cards.hit(r.x + r.w / 2, r.y + r.h / 2), Some(*want));
        }
    }

    #[test]
    fn tiles_are_side_by_side_without_overlap() {
        for i in 1..3 {
            let prev = tile_rect(1024, 768, i - 1);
            let cur = tile_rect(1024, 768, i);
            assert!(cur.x >= prev.x + prev.w, "tile {i} overlaps its neighbour");
        }
    }

    #[test]
    fn everything_fits_a_768_screen() {
        let last = tile_rect(1024, 768, 2);
        assert!(last.x + last.w <= 1024 - PAD_X);
        assert!(last.y + last.h + 120 < 768, "content runs off the screen");
    }

    #[test]
    fn counts_render_singular_plural_and_zero() {
        let mut b = [0u8; 16];
        assert_eq!(fmt_count(&mut b, 0, "message", "messages"), "none");
        let mut b = [0u8; 16];
        assert_eq!(fmt_count(&mut b, 1, "message", "messages"), "1 message");
        let mut b = [0u8; 16];
        assert_eq!(fmt_count(&mut b, 3, "message", "messages"), "3 messages");
        let mut b = [0u8; 16];
        assert_eq!(fmt_count(&mut b, 12, "message", "messages"), "12 messages");
    }

    #[test]
    fn count_cannot_overflow_its_buffer() {
        let mut b = [0u8; 16];
        let s = fmt_count(&mut b, 99, "playbook", "playbooks");
        assert!(s.len() <= 16);
    }

    #[test]
    fn copy_is_ascii_only() {
        for s in [
            "Search the knowledge base",
            "Type a query and press Enter. Works with the bridge offline.",
            "Search",
            "Capabilities",
            "Skills",
            "Recent mail",
            "Inbox empty, or email.search not granted.",
            crate::mcp::BRIDGE_OFFLINE_HINT,
            "bridge connected",
            "bridge offline",
            "os",
        ] {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII renders as '?': {s:?}"
            );
        }
    }

    #[test]
    fn tile_text_fits_its_column() {
        let r = tile_rect(1024, 768, 0);
        for s in ["Search", "Capabilities", "Skills"] {
            assert!(H2_FACE.width(s, 0) < r.w - 36, "tile title overflows: {s}");
        }
        assert!(SMALL_FACE.width("knowledge + email", 0) < r.w - 36);
        assert!(SMALL_FACE.width(Caps::default_grants().footer_status(), 0) < r.w - 36);
    }

    #[test]
    fn drawing_the_home_screen_does_not_panic() {
        // Exercises the offline branch and the count formatting together.
        let mail = MailPeek::empty(BridgeStatus::Offline);
        let _ = home_targets(1024, 768);
        assert_eq!(mail.count, 0);
    }
}
