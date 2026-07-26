//! Home screen — a launcher.
//!
//! One search field, three equal destination cards with count subtext, and a
//! locked footer status strip. Type is anti-aliased proportional (`font.rs`);
//! all copy is ASCII because the atlas covers 0x20..=0x7E only.

use crate::caps::Caps;
use crate::fb::Surface;
use crate::font::{BODY_FACE, BRAND_FACE, BTN_FACE, H2_FACE, SMALL_FACE};
use crate::mcp::{BridgeStatus, MailPeek};
use crate::skills::SkillPeek;

/// Apple-inspired light palette.
pub mod theme {
    /// Page — Apple light grey.
    pub const BG: u32 = 0x00F5_F5F7;
    /// Cards, inputs, switch knobs — pure white.
    pub const SURFACE: u32 = 0x00FF_FFFF;
    /// Primary text — Apple's near-black, never pure #000.
    pub const INK: u32 = 0x001D_1D1F;
    /// Secondary copy.
    pub const MUTED: u32 = 0x0086_868B;
    /// Accent / primary action.
    pub const ACCENT: u32 = 0x0000_71E3;
    /// Hairline separators.
    pub const RULE: u32 = 0x00D2_D2D7;
    /// Card / input border.
    pub const CARD_BORDER: u32 = 0x00E5_E5EA;
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
    fb.fill_round_rect(kx, ty + 3, knob, knob, knob / 2, theme::SURFACE);
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

/// Hairline card border + white fill rounded rect (search field, tiles, rows).
pub fn outlined_round_rect(fb: &Surface, x: i32, y: i32, w: i32, h: i32, radius: i32) {
    fb.fill_round_rect(x, y, w, h, radius, theme::CARD_BORDER);
    fb.fill_round_rect(
        x + 1,
        y + 1,
        w - 2,
        h - 2,
        radius.saturating_sub(1),
        theme::SURFACE,
    );
}

/// Bordered list row: title + muted subtitle (Skills / Caps chrome).
pub fn draw_titled_row(fb: &Surface, r: Rect, title: &str, sub: &str) {
    outlined_round_rect(fb, r.x, r.y, r.w, r.h, 10);
    fb.draw_text(r.x + 18, r.y + 26, title, &BRAND_FACE, 0, theme::INK);
    fb.draw_text(r.x + 18, r.y + 46, sub, &SMALL_FACE, 0, theme::MUTED);
}

/// Shared search-field chrome used on Home and Search.
///
/// `badge`, when set, is drawn muted on the far right inside the field
/// (e.g. "Offline Ready") so helper copy never sits under the input.
pub fn draw_query_field(
    fb: &Surface,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    query: &str,
    placeholder: &str,
    badge: Option<&str>,
) {
    outlined_round_rect(fb, x, y, w, h, 12);
    let tx = x + 18;
    let base = y + (h - BODY_FACE.px) / 2 + BODY_FACE.baseline();
    let badge_w = badge.map(|b| SMALL_FACE.width(b, 0) + 18).unwrap_or(0);
    if query.is_empty() {
        fb.draw_text(tx, base, placeholder, &BODY_FACE, 0, theme::MUTED);
    } else {
        fb.draw_text(tx, base, query, &BODY_FACE, 0, theme::INK);
    }
    if let Some(badge) = badge {
        let bx = x + w - 18 - SMALL_FACE.width(badge, 0);
        let bbase = y + (h - SMALL_FACE.px) / 2 + SMALL_FACE.baseline();
        fb.draw_text(bx, bbase, badge, &SMALL_FACE, 0, theme::MUTED);
    }
    let cx = (tx + BODY_FACE.width(query, 0) + 2).min(x + w - badge_w - 8);
    fb.fill_rect(cx, y + 14, 2, h - 28, theme::INK);
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
    /// Top-right Connect — re-probe the host bridge.
    Connect,
    Card(CardId),
}

/// Combined home hit-test (search field preferred over tiles).
#[derive(Clone, Copy, Debug)]
pub struct HomeTargets {
    w: i32,
}

impl HomeTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<HomeHit> {
        if search_rect(self.w).contains(px, py) {
            return Some(HomeHit::SearchField);
        }
        if connect_rect(self.w).contains(px, py) {
            return Some(HomeHit::Connect);
        }
        const IDS: [CardId; 3] = [CardId::Search, CardId::Capabilities, CardId::Skills];
        hit_among(3, px, py, |i| tile_rect(self.w, i as i32)).map(|i| HomeHit::Card(IDS[i]))
    }
}

/// The home screen: one focal search field, three equal cards, locked footer.
pub fn draw_home_full(
    fb: &Surface,
    mail: &MailPeek,
    skills: &SkillPeek,
    grants: Caps,
    query: &str,
) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;

    fb.fill(theme::BG);
    draw_nav(fb, w, mail);

    let r = search_rect(w);
    let badge = match mail.status {
        BridgeStatus::Online => "Online",
        BridgeStatus::Offline => "Offline Ready",
    };
    draw_query_field(
        fb,
        r.x,
        r.y,
        r.w,
        r.h,
        query,
        "Search knowledge base...",
        Some(badge),
    );

    let mut gbuf = [0u8; 16];
    let granted = fmt_n_label(&mut gbuf, grants.granted_count(), "Granted");
    let mut sbuf = [0u8; 16];
    let playbooks = fmt_n_label(&mut sbuf, skills.count, "Playbooks");
    let tiles: [(&str, &str); 3] = [
        ("Search", "Knowledge + Email"),
        ("Capabilities", granted),
        ("Skills", playbooks),
    ];
    for (i, (title, sub)) in tiles.iter().enumerate() {
        let r = tile_rect(w, i as i32);
        outlined_round_rect(fb, r.x, r.y, r.w, r.h, 16);
        fb.draw_text(r.x + 18, r.y + 34, title, &H2_FACE, 0, theme::INK);
        fb.draw_text(r.x + 18, r.y + 58, sub, &SMALL_FACE, 0, theme::MUTED);
    }

    draw_status_bar(fb, w, h, mail, grants);
}

/// `"N label"` into a caller-owned buffer (digits + space + label).
fn fmt_n_label<'a>(buf: &'a mut [u8; 16], n: usize, label: &str) -> &'a str {
    let mut i = 0;
    if n >= 10 {
        buf[i] = b'0' + ((n / 10) % 10) as u8;
        i += 1;
    }
    buf[i] = b'0' + (n % 10) as u8;
    i += 1;
    buf[i] = b' ';
    i += 1;
    for &b in label.as_bytes() {
        if i < buf.len() {
            buf[i] = b;
            i += 1;
        }
    }
    core::str::from_utf8(&buf[..i]).unwrap_or("")
}

/// Centered content column capped at `max` (home uses 920; list screens 720).
pub(crate) fn content_column(w: i32, max: i32) -> (i32, i32) {
    let cw = (w - PAD_X * 2).min(max);
    ((w - cw) / 2, cw)
}

fn home_column(w: i32) -> (i32, i32) {
    content_column(w, CONTENT_MAX)
}

/// The home search field, shared by drawing and hit-testing.
pub fn search_rect(w: i32) -> Rect {
    let (x, cw) = home_column(w);
    Rect::new(x, 120, cw, 52)
}

const TILE_TOP: i32 = 200;
const TILE_H: i32 = 88;

/// Bounding box of home tile `i` (0 = Search, 1 = Capabilities, 2 = Skills).
pub fn tile_rect(w: i32, i: i32) -> Rect {
    let (x0, cw) = home_column(w);
    let gap = 16;
    let tw = (cw - gap * 2) / 3;
    Rect::new(x0 + (tw + gap) * i, TILE_TOP, tw, TILE_H)
}

/// Top-right Connect pill — primary bridge action beside the status dot.
pub fn connect_rect(w: i32) -> Rect {
    let label = "Connect";
    let pad = 14;
    let bw = (BTN_FACE.width(label, 0) + pad * 2).max(88);
    let bh = 28;
    Rect::new(w - PAD_X - bw, (NAV_H - bh) / 2, bw, bh)
}

pub fn home_targets(w: i32) -> HomeTargets {
    HomeTargets { w }
}

fn draw_nav(fb: &Surface, w: i32, mail: &MailPeek) {
    let base = (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.baseline();
    fb.draw_text(PAD_X, base, "os", &BRAND_FACE, 0, theme::INK);

    let cr = connect_rect(w);
    fb.fill_round_rect(cr.x, cr.y, cr.w, cr.h, cr.h / 2, theme::ACCENT);
    let cbase = cr.y + (cr.h - BTN_FACE.px) / 2 + BTN_FACE.baseline();
    fb.draw_text_centered(cr.x + cr.w / 2, cbase, "Connect", &BTN_FACE, 0, theme::SURFACE);

    let label = "Bridge";
    let dot = match mail.status {
        BridgeStatus::Online => theme::ONLINE,
        BridgeStatus::Offline => theme::OFFLINE,
    };
    let tw = SMALL_FACE.width(label, 0);
    let gap = 10;
    let dot_d = 7;
    let cluster_w = dot_d + 6 + tw;
    let cluster_x = cr.x - gap - cluster_w;
    let sbase = (NAV_H - SMALL_FACE.px) / 2 + SMALL_FACE.baseline();
    fb.fill_round_rect(
        cluster_x,
        NAV_H / 2 - dot_d / 2,
        dot_d,
        dot_d,
        dot_d / 2,
        dot,
    );
    fb.draw_text(cluster_x + dot_d + 6, sbase, label, &SMALL_FACE, 0, theme::MUTED);

    fb.fill_rect(0, NAV_H, w, 1, theme::RULE);
}

/// Unified bottom telemetry — no orphaned mid-page status lines.
fn draw_status_bar(fb: &Surface, w: i32, h: i32, mail: &MailPeek, grants: Caps) {
    let mut line = [0u8; 96];
    let mut n = 0;
    let push = |line: &mut [u8], n: &mut usize, s: &str| {
        for &b in s.as_bytes() {
            if *n < line.len() {
                line[*n] = b;
                *n += 1;
            }
        }
    };
    match mail.count {
        0 => push(&mut line, &mut n, "Inbox empty"),
        1 => push(&mut line, &mut n, "1 message"),
        c => {
            let mut b = [0u8; 16];
            push(&mut line, &mut n, fmt_n_label(&mut b, c, "messages"));
        }
    }
    push(&mut line, &mut n, "  |  Caps: ");
    {
        let mut b = [0u8; 16];
        push(&mut line, &mut n, fmt_n_label(&mut b, grants.granted_count(), "Active"));
    }
    push(&mut line, &mut n, "  |  Bridge: ");
    push(
        &mut line,
        &mut n,
        match mail.status {
            BridgeStatus::Online => "Online",
            BridgeStatus::Offline => "Offline",
        },
    );
    let text = core::str::from_utf8(&line[..n]).unwrap_or("");
    fb.draw_text_centered(w / 2, h - 28, text, &SMALL_FACE, 0, theme::MUTED);
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::Caps;
    use crate::mcp::MailPeek;

    #[test]
    fn search_field_is_the_primary_target() {
        let t = home_targets(1024);
        let r = search_rect(1024);
        assert_eq!(
            t.hit(r.x + r.w / 2, r.y + r.h / 2),
            Some(HomeHit::SearchField)
        );
    }

    #[test]
    fn connect_sits_in_the_nav_bar() {
        let t = home_targets(1024);
        let c = connect_rect(1024);
        assert!(c.y + c.h <= NAV_H);
        assert_eq!(
            t.hit(c.x + c.w / 2, c.y + c.h / 2),
            Some(HomeHit::Connect)
        );
    }

    #[test]
    fn tiles_do_not_overlap_the_search_field() {
        let r = search_rect(1024);
        assert!(TILE_TOP >= r.y + r.h + 16, "tiles collide with the field");
    }

    #[test]
    fn each_tile_hit_tests_to_its_own_id() {
        let t = home_targets(1024);
        for (i, want) in [CardId::Search, CardId::Capabilities, CardId::Skills]
            .iter()
            .enumerate()
        {
            let r = tile_rect(1024, i as i32);
            assert_eq!(
                t.hit(r.x + r.w / 2, r.y + r.h / 2),
                Some(HomeHit::Card(*want))
            );
        }
    }

    #[test]
    fn tiles_are_side_by_side_without_overlap() {
        for i in 1..3 {
            let prev = tile_rect(1024, i - 1);
            let cur = tile_rect(1024, i);
            assert!(cur.x >= prev.x + prev.w, "tile {i} overlaps its neighbour");
            assert_eq!(cur.h, prev.h, "cards must share height");
        }
    }

    #[test]
    fn everything_fits_a_768_screen() {
        let last = tile_rect(1024, 2);
        assert!(last.x + last.w <= 1024 - PAD_X);
        assert!(last.y + last.h + 48 < 768, "content runs off the screen");
    }

    #[test]
    fn n_labels_render() {
        let mut b = [0u8; 16];
        assert_eq!(fmt_n_label(&mut b, 3, "messages"), "3 messages");
        let mut b = [0u8; 16];
        assert_eq!(fmt_n_label(&mut b, 4, "Granted"), "4 Granted");
    }

    #[test]
    fn count_cannot_overflow_its_buffer() {
        let mut b = [0u8; 16];
        let s = fmt_n_label(&mut b, 99, "Playbooks");
        assert!(s.len() <= 16);
    }

    #[test]
    fn copy_is_ascii_only() {
        for s in [
            "Search knowledge base...",
            "Offline Ready",
            "Online",
            "Search",
            "Capabilities",
            "Skills",
            "Knowledge + Email",
            "Connect",
            "Bridge",
            "Inbox empty",
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
        assert!(SMALL_FACE.width("Knowledge + Email", 0) < r.w - 36);
        let mut b = [0u8; 16];
        assert!(SMALL_FACE.width(fmt_n_label(&mut b, 5, "Granted"), 0) < r.w - 36);
    }

    #[test]
    fn drawing_the_home_screen_does_not_panic() {
        let mail = MailPeek::empty(BridgeStatus::Offline);
        let _ = home_targets(1024);
        assert_eq!(mail.count, 0);
        assert_eq!(Caps::default_grants().granted_count(), 2);
    }
}
