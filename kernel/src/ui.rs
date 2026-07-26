//! Home screen — a launcher.
//!
//! One search field, three equal destination cards with count subtext, and a
//! locked footer status strip. Type is anti-aliased proportional (`font.rs`);
//! all copy is ASCII because the atlas covers 0x20..=0x7E only.

use crate::caps::Caps;
use crate::fb::{self, Surface};
use crate::font::{BODY_FACE, BRAND_FACE, BTN_FACE, H2_FACE, SMALL_FACE};
use crate::mcp::MailPeek;
use crate::skills::SkillPeek;

/// Apple-inspired light palette (SURFACE / ACCENT live on [`crate::fb`]).
pub(crate) mod theme {
    /// Primary text — Apple's near-black, never pure #000.
    pub(crate) const INK: u32 = 0x001D_1D1F;
    /// Secondary copy.
    pub(crate) const MUTED: u32 = 0x0086_868B;
    /// Hairline separators.
    pub(crate) const RULE: u32 = 0x00D2_D2D7;
    /// Card / input border.
    pub(super) const CARD_BORDER: u32 = 0x00E5_E5EA;
    pub(crate) const ONLINE: u32 = 0x0034_C759;
    pub(crate) const OFFLINE: u32 = 0x00FF_3B30;
}

/// Shared chrome height (home nav, Skills/Caps/Search Back bar).
pub(crate) const NAV_H: i32 = 56;
/// Connect pill / Back control height (vertically centred in [`NAV_H`]).
pub(crate) const NAV_CTRL_H: i32 = 28;

/// Search field height on Home and Search.
pub(crate) const FIELD_H: i32 = 52;
/// Top of the Search field / Skills·Caps list column.
pub(crate) const LIST_TOP: i32 = 150;
/// Shared horizontal page margin.
pub(crate) const PAD_X: i32 = 28;
/// Home launcher column cap (search field + tiles).
pub(crate) const HOME_CONTENT_MAX: i32 = 920;
/// Skills / Caps / Search list column cap.
pub(crate) const LIST_CONTENT_MAX: i32 = 720;

/// Pill on/off switch width/height (Capabilities + setup).
const SWITCH_W: i32 = 40;
const SWITCH_H: i32 = 22;

/// Pill switch, right-aligned inside a list row (18px pad, vertically centred).
pub(crate) fn draw_switch_in_row(fb: &Surface, row: Rect, on: bool) {
    let tx = row.x + row.w - 18 - SWITCH_W;
    let ty = row.y + (row.h - SWITCH_H) / 2;
    fb.fill_round_rect(
        tx,
        ty,
        SWITCH_W,
        SWITCH_H,
        SWITCH_H / 2,
        if on { fb::ACCENT } else { theme::RULE },
    );
    let knob = SWITCH_H - 6;
    let kx = if on {
        tx + SWITCH_W - knob - 3
    } else {
        tx + 3
    };
    fb.fill_round_rect(kx, ty + 3, knob, knob, knob / 2, fb::SURFACE);
}


/// Axis-aligned hit region (inclusive origin, exclusive of `x+w` / `y+h`).
#[derive(Clone, Copy)]
pub(crate) struct Rect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
}

impl Rect {
    pub(crate) const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub(crate) fn contains(self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
}

/// First index in `0..count` whose rect contains `(px, py)`.
pub(crate) fn hit_among(
    count: usize,
    px: i32,
    py: i32,
    mut rect_at: impl FnMut(usize) -> Rect,
) -> Option<usize> {
    (0..count).find(|&i| rect_at(i).contains(px, py))
}

/// Hairline card border + white fill rounded rect (search field, tiles, rows).
pub(crate) fn outlined_round_rect(fb: &Surface, r: Rect, radius: i32) {
    fb.fill_round_rect(r.x, r.y, r.w, r.h, radius, theme::CARD_BORDER);
    fb.fill_round_rect(
        r.x + 1,
        r.y + 1,
        r.w - 2,
        r.h - 2,
        // Callers pass radius ≥ 10 (field / row / tile chrome).
        radius - 1,
        fb::SURFACE,
    );
}

/// Bordered list row: title + muted subtitle (Skills / Caps chrome).
pub(crate) fn draw_titled_row(fb: &Surface, r: Rect, title: &str, sub: &str) {
    outlined_round_rect(fb, r, 10);
    fb.draw_text(r.x + 18, r.y + 26, title, &BRAND_FACE, 0, theme::INK);
    fb.draw_text(r.x + 18, r.y + 46, sub, &SMALL_FACE, 0, theme::MUTED);
}

/// Shared search-field chrome used on Home and Search.
///
/// Non-empty `badge` is drawn muted on the far right inside the field
/// (e.g. "Offline Ready") so helper copy never sits under the input.
pub(crate) fn draw_query_field(
    fb: &Surface,
    r: Rect,
    query: &str,
    placeholder: &str,
    badge: &str,
) {
    let Rect { x, y, w, h } = r;
    outlined_round_rect(fb, r, 12);
    let tx = x + 18;
    let base = y + (h - BODY_FACE.px) / 2 + BODY_FACE.ascent;
    let badge_tw = if badge.is_empty() {
        0
    } else {
        SMALL_FACE.width(badge, 0)
    };
    if query.is_empty() {
        fb.draw_text(tx, base, placeholder, &BODY_FACE, 0, theme::MUTED);
    } else {
        fb.draw_text(tx, base, query, &BODY_FACE, 0, theme::INK);
    }
    if !badge.is_empty() {
        let bx = x + w - 18 - badge_tw;
        let bbase = y + (h - SMALL_FACE.px) / 2 + SMALL_FACE.ascent;
        fb.draw_text(bx, bbase, badge, &SMALL_FACE, 0, theme::MUTED);
    }
    // badge_tw is a face width (tiny vs i32::MAX); +18 cannot saturate.
    let cx = (tx + BODY_FACE.width(query, 0) + 2).min(x + w - (badge_tw + 18) - 8);
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
pub fn home_hit(w: i32, px: i32, py: i32) -> Option<HomeHit> {
    if search_rect(w).contains(px, py) {
        return Some(HomeHit::SearchField);
    }
    if connect_rect(w).contains(px, py) {
        return Some(HomeHit::Connect);
    }
    const IDS: [CardId; 3] = [CardId::Search, CardId::Capabilities, CardId::Skills];
    hit_among(3, px, py, |i| tile_rect(w, i as i32)).map(|i| HomeHit::Card(IDS[i]))
}

/// The home screen: one focal search field, three equal cards, locked footer.
pub fn draw_home(
    fb: &Surface,
    mail: &MailPeek,
    skills: &SkillPeek,
    grants: Caps,
    query: &str,
) {
    let w = fb.width as i32;

    fb.fill();
    let online = mail.online();
    draw_nav(fb, online);

    let r = search_rect(w);
    let badge = if online { "Online" } else { "Offline Ready" };
    draw_query_field(fb, r, query, "Search knowledge base...", badge);

    let n_grants = grants.granted_count();
    let mut gbuf = [0u8; 16];
    let granted = fmt_n_label(&mut gbuf, n_grants, "Granted");
    let mut sbuf = [0u8; 16];
    let playbooks = fmt_n_label(&mut sbuf, skills.count, "Playbooks");
    let tiles: [(&str, &str); 3] = [
        ("Search", "Knowledge corpus"),
        ("Capabilities", granted),
        ("Skills", playbooks),
    ];
    for (i, (title, sub)) in tiles.iter().enumerate() {
        let r = tile_rect(w, i as i32);
        outlined_round_rect(fb, r, 16);
        fb.draw_text(r.x + 18, r.y + 34, title, &H2_FACE, 0, theme::INK);
        fb.draw_text(r.x + 18, r.y + 58, sub, &SMALL_FACE, 0, theme::MUTED);
    }

    draw_status_bar(fb, mail, n_grants);
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

/// Centered content column capped at `max` ([`HOME_CONTENT_MAX`] / [`LIST_CONTENT_MAX`]).
pub(crate) fn content_column(w: i32, max: i32) -> (i32, i32) {
    let cw = (w - PAD_X * 2).min(max);
    ((w - cw) / 2, cw)
}

/// The home search field, shared by drawing and hit-testing.
fn search_rect(w: i32) -> Rect {
    let (x, cw) = content_column(w, HOME_CONTENT_MAX);
    Rect::new(x, 120, cw, FIELD_H)
}

const TILE_TOP: i32 = 200;
const TILE_H: i32 = 88;

/// Bounding box of home tile `i` (0 = Search, 1 = Capabilities, 2 = Skills).
fn tile_rect(w: i32, i: i32) -> Rect {
    let (x0, cw) = content_column(w, HOME_CONTENT_MAX);
    let gap = 16;
    let tw = (cw - gap * 2) / 3;
    Rect::new(x0 + (tw + gap) * i, TILE_TOP, tw, TILE_H)
}

/// Top-right Connect pill — primary bridge action beside the status dot.
const CONNECT: &str = "Connect";

fn connect_rect(w: i32) -> Rect {
    let pad = 14;
    let bw = (BTN_FACE.width(CONNECT, 0) + pad * 2).max(88);
    Rect::new(w - PAD_X - bw, (NAV_H - NAV_CTRL_H) / 2, bw, NAV_CTRL_H)
}

fn draw_nav(fb: &Surface, online: bool) {
    let w = fb.width as i32;
    let base = (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.ascent;
    fb.draw_text(PAD_X, base, "os", &BRAND_FACE, 0, theme::INK);

    let cr = connect_rect(w);
    fb.fill_round_rect(cr.x, cr.y, cr.w, cr.h, cr.h / 2, fb::ACCENT);
    let cbase = cr.y + (cr.h - BTN_FACE.px) / 2 + BTN_FACE.ascent;
    fb.draw_text_centered(cr.x + cr.w / 2, cbase, CONNECT, &BTN_FACE, 0, fb::SURFACE);

    let label = "Bridge";
    let dot = if online { theme::ONLINE } else { theme::OFFLINE };
    let tw = SMALL_FACE.width(label, 0);
    let gap = 10;
    let dot_d = 7;
    let cluster_w = dot_d + 6 + tw;
    let cluster_x = cr.x - gap - cluster_w;
    let sbase = (NAV_H - SMALL_FACE.px) / 2 + SMALL_FACE.ascent;
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
/// Inbox count is only shown after a granted peek (`Online { inbox: Some(_) }`).
fn draw_status_bar(fb: &Surface, mail: &MailPeek, grant_count: usize) {
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
    let mut first = true;
    let mut sep = |line: &mut [u8], n: &mut usize| {
        if !first {
            push(line, n, "  |  ");
        }
        first = false;
    };
    if let MailPeek::Online {
        inbox: Some(count),
    } = *mail
    {
        sep(&mut line, &mut n);
        match count {
            0 => push(&mut line, &mut n, "Inbox empty"),
            1 => push(&mut line, &mut n, "1 message"),
            c => {
                let mut b = [0u8; 16];
                push(&mut line, &mut n, fmt_n_label(&mut b, c, "messages"));
            }
        }
    }
    sep(&mut line, &mut n);
    push(&mut line, &mut n, "Caps: ");
    {
        let mut b = [0u8; 16];
        push(&mut line, &mut n, fmt_n_label(&mut b, grant_count, "Active"));
    }
    sep(&mut line, &mut n);
    push(&mut line, &mut n, "Bridge: ");
    push(
        &mut line,
        &mut n,
        if mail.online() { "Online" } else { "Offline" },
    );
    let text = core::str::from_utf8(&line[..n]).unwrap_or("");
    fb.draw_text_centered(
        fb.width as i32 / 2,
        fb.height as i32 - 28,
        text,
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_field_is_the_primary_target() {
        let r = search_rect(1024);
        assert_eq!(
            home_hit(1024, r.x + r.w / 2, r.y + r.h / 2),
            Some(HomeHit::SearchField)
        );
    }

    #[test]
    fn connect_sits_in_the_nav_bar() {
        let c = connect_rect(1024);
        assert!(c.y + c.h <= NAV_H);
        assert_eq!(
            home_hit(1024, c.x + c.w / 2, c.y + c.h / 2),
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
        for (i, want) in [CardId::Search, CardId::Capabilities, CardId::Skills]
            .iter()
            .enumerate()
        {
            let r = tile_rect(1024, i as i32);
            assert_eq!(
                home_hit(1024, r.x + r.w / 2, r.y + r.h / 2),
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
        let mut b = [0u8; 16];
        assert!(fmt_n_label(&mut b, 99, "Playbooks").len() <= 16);
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
            "Knowledge corpus",
            CONNECT,
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
        assert!(SMALL_FACE.width("Knowledge corpus", 0) < r.w - 36);
        let mut b = [0u8; 16];
        assert!(SMALL_FACE.width(fmt_n_label(&mut b, 5, "Granted"), 0) < r.w - 36);
    }
}
