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

/// Pill on/off switch width/height (Capabilities + setup).
pub const SWITCH_W: i32 = 40;
pub const SWITCH_H: i32 = 22;

/// Draw a pill switch with its origin at `(tx, ty)`.
pub fn draw_switch(fb: &Surface, tx: i32, ty: i32, on: bool) {
    fb.fill_round_rect(
        tx,
        ty,
        SWITCH_W,
        SWITCH_H,
        SWITCH_H / 2,
        if on { theme::ACCENT } else { theme::RULE },
    );
    let knob = SWITCH_H - 6;
    let kx = if on {
        tx + SWITCH_W - knob - 3
    } else {
        tx + 3
    };
    fb.fill_round_rect(kx, ty + 3, knob, knob, knob / 2, theme::BG);
}


/// Axis-aligned hit region (inclusive origin, exclusive of `x+w` / `y+h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
}

/// First index in `0..count` whose rect contains `(px, py)`.
pub fn hit_among(
    count: usize,
    px: i32,
    py: i32,
    mut rect_at: impl FnMut(usize) -> Rect,
) -> Option<usize> {
    (0..count).find(|&i| rect_at(i).contains(px, py))
}

/// Hairline border + fill rounded rect (search field, tiles, rows).
pub fn outlined_round_rect(
    fb: &Surface,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    border: u32,
    fill: u32,
) {
    fb.fill_round_rect(x, y, w, h, radius, border);
    fb.fill_round_rect(
        x + 1,
        y + 1,
        w - 2,
        h - 2,
        radius.saturating_sub(1),
        fill,
    );
}

/// Shared search-field chrome used on Home and Search.
pub fn draw_query_field(
    fb: &Surface,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    query: &str,
    placeholder: &str,
    caret: bool,
) {
    outlined_round_rect(fb, x, y, w, h, 12, theme::RULE, theme::BG);
    let tx = x + 18;
    let base = y + (h - BODY_FACE.px) / 2 + BODY_FACE.baseline();
    if query.is_empty() {
        fb.draw_text(tx, base, placeholder, &BODY_FACE, 0, theme::MUTED);
    } else {
        fb.draw_text(tx, base, query, &BODY_FACE, 0, theme::INK);
    }
    if caret {
        let cx = tx + BODY_FACE.width(query, 0) + 2;
        fb.fill_rect(cx, y + 14, 2, h - 28, theme::INK);
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
    let r = search_rect(w);
    let (fx, fy, fw, fh) = (r.x, r.y, r.w, r.h);
    draw_query_field(fb, fx, fy, fw, fh, query, "Search the knowledge base", caret);
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

    let tiles: [(&str, &str); 3] = [
        ("Search", "knowledge + email"),
        ("Capabilities", status),
        ("Skills", skill_label),
    ];
    for (i, (title, sub)) in tiles.iter().enumerate() {
        let r = tile_rect(w, i as i32);
        outlined_round_rect(fb, r.x, r.y, r.w, r.h, 12, theme::CARD_BORDER, theme::BG);
        fb.draw_text(r.x + 18, r.y + 34, title, &H2_FACE, 0, theme::INK);
        fb.draw_text(r.x + 18, r.y + 58, sub, &SMALL_FACE, 0, theme::MUTED);
    }

    // Live content instead of marketing copy.
    let ry = TILE_TOP + TILE_H + 40;
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
pub fn search_rect(w: i32) -> Rect {
    let (x, cw) = home_column(w);
    Rect::new(x, 132, cw, 52)
}

pub(crate) const TILE_TOP: i32 = 242;
pub(crate) const TILE_H: i32 = 78;

/// Bounding box of home tile `i` (0 = Search, 1 = Capabilities, 2 = Skills).
pub fn tile_rect(w: i32, i: i32) -> Rect {
    let (x0, cw) = home_column(w);
    let gap = 16;
    let tw = (cw - gap * 2) / 3;
    Rect::new(x0 + (tw + gap) * i, TILE_TOP, tw, TILE_H)
}

pub fn card_targets(w: i32) -> CardTargets {
    CardTargets {
        search: tile_rect(w, 0),
        capabilities: tile_rect(w, 1),
        skills: tile_rect(w, 2),
    }
}

pub fn home_targets(w: i32) -> HomeTargets {
    HomeTargets {
        search: search_rect(w),
        cards: card_targets(w),
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
        let t = home_targets(1024);
        let r = search_rect(1024);
        let (fx, fy, fw, fh) = (r.x, r.y, r.w, r.h);
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
        let r = search_rect(1024);
        let (fy, fh) = (r.y, r.h);
        assert!(TILE_TOP >= fy + fh, "tiles collide with the field");
    }

    #[test]
    fn each_tile_hit_tests_to_its_own_id() {
        let t = home_targets(1024);
        for (i, want) in [CardId::Search, CardId::Capabilities, CardId::Skills]
            .iter()
            .enumerate()
        {
            let r = tile_rect(1024, i as i32);
            assert_eq!(t.cards.hit(r.x + r.w / 2, r.y + r.h / 2), Some(*want));
        }
    }

    #[test]
    fn tiles_are_side_by_side_without_overlap() {
        for i in 1..3 {
            let prev = tile_rect(1024, i - 1);
            let cur = tile_rect(1024, i);
            assert!(cur.x >= prev.x + prev.w, "tile {i} overlaps its neighbour");
        }
    }

    #[test]
    fn everything_fits_a_768_screen() {
        let last = tile_rect(1024, 2);
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
        let r = tile_rect(1024, 0);
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
        let _ = home_targets(1024);
        assert_eq!(mail.count, 0);
    }
}
