//! Home screen — a launcher.
//!
//! A search field you can type into on arrival, three destinations carrying
//! live counts, the last agent Brief when one has run, and recent mail when
//! granted. Type is anti-aliased proportional (see `font.rs`); all copy is
//! ASCII because the atlas covers 0x20..=0x7E only.

use crate::agent::Brief;
use crate::caps::{Cap, Caps};
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
    /// Accent at 20% over white — that pill's border.
    pub const TINT_BORDER: u32 = 0x00CC_E3F9;
    pub const ONLINE: u32 = 0x0034_C759;
    pub const OFFLINE: u32 = 0x00FF_3B30;
}

const NAV_H: i32 = 56;
const PAD_X: i32 = 28;
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

/// Which home CTA was under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CtaId {
    Ready,
    Skills,
}

/// Which home card was under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardId {
    Connectors,
    Capabilities,
    Skills,
}

/// Any clickable region on the finished home screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeHit {
    Cta(CtaId),
    Card(CardId),
    /// Re-open the last agent Brief.
    Brief,
}

/// Hit targets for the CTA pair, computed with the same layout as `draw_home`.
#[derive(Clone, Copy, Debug)]
pub struct CtaTargets {
    pub ready: Rect,
    pub skills: Rect,
}

impl CtaTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<CtaId> {
        // Ready is a small nav control; the search field must never map here
        // (that used to restart setup whenever you clicked the query box).
        if self.ready.w > 0 && self.ready.contains(px, py) {
            Some(CtaId::Ready)
        } else if self.skills.w > 0 && self.skills.contains(px, py) {
            Some(CtaId::Skills)
        } else {
            None
        }
    }
}

/// Hit targets for the three "What's wired" cards.
#[derive(Clone, Copy, Debug)]
pub struct CardTargets {
    pub connectors: Rect,
    pub capabilities: Rect,
    pub skills: Rect,
}

impl CardTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<CardId> {
        if self.connectors.contains(px, py) {
            Some(CardId::Connectors)
        } else if self.capabilities.contains(px, py) {
            Some(CardId::Capabilities)
        } else if self.skills.contains(px, py) {
            Some(CardId::Skills)
        } else {
            None
        }
    }
}

/// Combined home hit-test (CTAs preferred over cards when overlapping — they don't).
#[derive(Clone, Copy, Debug)]
pub struct HomeTargets {
    pub ctas: CtaTargets,
    pub cards: CardTargets,
    pub brief: Rect,
}

impl HomeTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<HomeHit> {
        if let Some(c) = self.ctas.hit(px, py) {
            return Some(HomeHit::Cta(c));
        }
        if let Some(c) = self.cards.hit(px, py) {
            return Some(HomeHit::Card(c));
        }
        if self.brief.w > 0 && self.brief.contains(px, py) {
            return Some(HomeHit::Brief);
        }
        None
    }
}

/// Draw the home composition on `fb`.
///
/// `status` is a short Capabilities-tile line (ASCII). `brief` is the last
/// agent report; when present it replaces the empty mail placeholder.
pub fn draw_home(
    fb: &Surface,
    mail: &MailPeek,
    skills: &SkillPeek,
    status: &str,
    caps: Caps,
    brief: &Brief,
) {
    draw_home_full(fb, mail, skills, status, "", false, caps, brief)
}

/// The home screen: a launcher, not a landing page.
///
/// A search field you can type into immediately, three destinations carrying
/// live counts, the last Brief when a skill has run, and recent mail when
/// granted.
pub fn draw_home_full(
    fb: &Surface,
    mail: &MailPeek,
    skills: &SkillPeek,
    status: &str,
    query: &str,
    caret: bool,
    caps: Caps,
    brief: &Brief,
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
    let mut sbuf = [0u8; 28];
    let skill_label = skills_tile_sub(caps, skills.count, &mut sbuf);
    let mut src_buf = [0u8; 40];
    let search_sub = search_tile_sub(caps, &mut src_buf);

    let gap = 16;
    let tw = (cw - gap * 2) / 3;
    let ty = tile_top(h);
    let tiles: [(&str, &str); 3] = [
        ("Search", search_sub),
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

    // Last brief stays after Back; mail still shows below when granted so a
    // morning brief cannot starve the inbox forever.
    let ry = ty + TILE_H + 40;
    let mut y = ry;
    if brief.has_report() {
        y = draw_brief_residue(fb, x0, ry, cw, brief);
        y += 16;
    }
    if mail.count > 0 {
        fb.draw_text(x0, y, "Recent mail", &BRAND_FACE, 0, theme::INK);
        y += 28;
        let mail_n = if brief.has_report() { 2 } else { 3 };
        for i in 0..mail.count.min(mail_n) {
            fb.draw_text(x0, y, mail.row_subj(i), &BODY_FACE, 0, theme::INK);
            let from = mail.row_from(i);
            fb.draw_text(x0 + cw - SMALL_FACE.width(from, 0), y, from, &SMALL_FACE, 0, theme::MUTED);
            y += 12;
            fb.fill_rect(x0, y, cw, 1, theme::CARD_BORDER);
            y += 22;
        }
    } else if !brief.has_report() {
        fb.draw_text(
            x0,
            ry,
            match mail.status {
                BridgeStatus::Online => "Inbox empty, or email.search not granted.",
                BridgeStatus::Offline => "Bridge offline - run: make utm-bridged",
            },
            &SMALL_FACE,
            0,
            theme::MUTED,
        );
    }

    fb.draw_text_centered(w / 2, h - 24, mail_label, &SMALL_FACE, 0, theme::MUTED);
}

/// Skills-tile subtitle: playbook count plus whether Save skills is granted.
pub fn skills_tile_sub<'a>(caps: Caps, count: usize, buf: &'a mut [u8; 28]) -> &'a str {
    buf.fill(0);
    let mut n = 0;
    let mut push = |s: &str, n: &mut usize| {
        for &b in s.as_bytes() {
            if *n < buf.len() {
                buf[*n] = b;
                *n += 1;
            }
        }
    };
    // Keep it short for the tile width: "7 writable" / "7 read-only".
    if count >= 10 {
        push("9+", &mut n);
    } else {
        let digit = [b'0' + (count as u8)];
        push(core::str::from_utf8(&digit).unwrap_or("0"), &mut n);
    }
    push(
        if caps.allows(Cap::SkillsSave) {
            " writable"
        } else {
            " read-only"
        },
        &mut n,
    );
    match core::str::from_utf8(&buf[..n]) {
        Ok(s) => s,
        Err(e) => core::str::from_utf8(&buf[..e.valid_up_to()]).unwrap_or(""),
    }
}

/// Short Search-tile subtitle naming the sources the current grants unlock.
pub fn search_tile_sub<'a>(caps: Caps, buf: &'a mut [u8; 40]) -> &'a str {
    buf.fill(0);
    let mut n = 0;
    let mut push = |s: &str, n: &mut usize| {
        for &b in s.as_bytes() {
            if *n < buf.len() {
                buf[*n] = b;
                *n += 1;
            }
        }
    };
    if caps.allows(Cap::SearchQuery) {
        push("docs", &mut n);
    }
    if caps.allows(Cap::EmailSearch) {
        if n > 0 {
            push(" + ", &mut n);
        }
        push("mail", &mut n);
    }
    if caps.allows(Cap::WorkspaceIndex) {
        if n > 0 {
            push(" + ", &mut n);
        }
        push("files", &mut n);
    }
    if caps.allows(Cap::PortalSync) {
        if n > 0 {
            push(" + ", &mut n);
        }
        // Covers teddy corpus/portals and market.* under the same grant.
        push("online", &mut n);
    }
    if n == 0 {
        "grant search first"
    } else {
        core::str::from_utf8(&buf[..n]).unwrap_or("docs")
    }
}

/// Draw the last-brief strip; returns the y just below it.
fn draw_brief_residue(fb: &Surface, x0: i32, ry: i32, cw: i32, brief: &Brief) -> i32 {
    fb.draw_text(x0, ry, "Last brief", &BRAND_FACE, 0, theme::INK);
    let hint = "Tap to reopen";
    fb.draw_text(
        x0 + cw - SMALL_FACE.width(hint, 0),
        ry,
        hint,
        &SMALL_FACE,
        0,
        theme::MUTED,
    );
    let title = if brief.heading().is_empty() {
        brief.skill_name()
    } else {
        brief.heading()
    };
    fb.draw_text(x0, ry + 28, title, &BODY_FACE, 0, theme::INK);
    let mut y = ry + 52;
    // Two lines when mail may follow, so both fit a 768 screen.
    for i in 0..brief.count.min(2) {
        let mut line = [0u8; 72];
        let mut n = 0;
        for &b in brief.lines[i].tag().as_bytes().iter().take(8) {
            line[n] = b;
            n += 1;
        }
        if n + 2 < line.len() {
            line[n] = b':';
            line[n + 1] = b' ';
            n += 2;
        }
        for &b in brief.lines[i].text().as_bytes() {
            if n >= line.len() {
                break;
            }
            line[n] = b;
            n += 1;
        }
        let s = core::str::from_utf8(&line[..n]).unwrap_or(brief.lines[i].text());
        fb.draw_text(x0, y, s, &SMALL_FACE, 0, theme::MUTED);
        y += 20;
    }
    y
}

/// Hit box for the last-brief strip on home (empty when none).
pub fn brief_rect(w: i32, h: i32, brief: &Brief) -> Rect {
    if !brief.has_report() {
        return Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
    }
    let (x0, cw) = home_column(w);
    let ry = tile_top(h) + TILE_H + 40;
    let lines = brief.count.min(2) as i32;
    Rect {
        x: x0,
        y: ry,
        w: cw,
        h: 52 + lines * 20 + 8,
    }
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

pub fn cta_targets(w: i32, h: i32, _skills: &SkillPeek) -> CtaTargets {
    let _ = h;
    // Setup lives in the nav. Skills is reached via its tile (CardId), so the
    // duplicate CTA rect stays empty — one hit target per destination.
    let (sx, sy, sw, sh) = setup_rect(w);
    CtaTargets {
        ready: Rect {
            x: sx,
            y: sy,
            w: sw,
            h: sh,
        },
        skills: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
    }
}

/// Bounding box of home tile `i` (0 = Search, 1 = Capabilities, 2 = Skills).
pub fn tile_rect(w: i32, h: i32, i: i32) -> Rect {
    let (x0, cw) = home_column(w);
    let gap = 16;
    let tw = (cw - gap * 2) / 3;
    Rect { x: x0 + (tw + gap) * i, y: tile_top(h), w: tw, h: TILE_H }
}

pub fn card_targets(w: i32, h: i32) -> CardTargets {
    CardTargets {
        connectors: tile_rect(w, h, 0),
        capabilities: tile_rect(w, h, 1),
        skills: tile_rect(w, h, 2),
    }
}

pub fn home_targets(w: i32, h: i32, skills: &SkillPeek, brief: &Brief) -> HomeTargets {
    HomeTargets {
        ctas: cta_targets(w, h, skills),
        cards: card_targets(w, h),
        brief: brief_rect(w, h, brief),
    }
}

fn draw_nav(fb: &Surface, w: i32, mail: &MailPeek) {
    let base = (NAV_H - BRAND_FACE.px) / 2 + BRAND_FACE.baseline();
    fb.draw_text(PAD_X, base, "os", &BRAND_FACE, 0, theme::INK);

    // Quiet way to re-enter setup without stealing the search field's hit box.
    let (sx, sy, _sw, sh) = setup_rect(w);
    fb.draw_text(
        sx,
        sy + (sh - SMALL_FACE.px) / 2 + SMALL_FACE.baseline(),
        "setup",
        &SMALL_FACE,
        0,
        theme::MUTED,
    );

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

/// Nav "setup" control — restarts the first-boot journey on purpose.
pub fn setup_rect(_w: i32) -> (i32, i32, i32, i32) {
    let label_w = SMALL_FACE.width("setup", 0);
    let x = PAD_X + BRAND_FACE.width("os", 0) + 18;
    (x, (NAV_H - 28) / 2, label_w + 8, 28)
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Brief;
    use crate::caps::Caps;
    use crate::mcp::MailPeek;

    fn peek() -> SkillPeek {
        SkillPeek::from_builtin()
    }

    fn empty_brief() -> Brief {
        Brief::empty()
    }

    #[test]
    fn search_field_is_not_a_cta() {
        // Clicking the query box used to fire CtaId::Ready and restart setup.
        let t = home_targets(1024, 768, &peek(), &empty_brief());
        let (fx, fy, fw, fh) = search_rect(1024, 768);
        assert_eq!(t.hit(fx + fw / 2, fy + fh / 2), None);
        let (sx, sy, sw, sh) = setup_rect(1024);
        assert_eq!(
            t.ctas.hit(sx + sw / 2, sy + sh / 2),
            Some(CtaId::Ready)
        );
    }

    #[test]
    fn tiles_do_not_overlap_the_search_field() {
        let (_, fy, _, fh) = search_rect(1024, 768);
        assert!(tile_top(768) >= fy + fh, "tiles collide with the field");
    }

    #[test]
    fn each_tile_hit_tests_to_its_own_id() {
        let t = home_targets(1024, 768, &peek(), &empty_brief());
        for (i, want) in [CardId::Connectors, CardId::Capabilities, CardId::Skills]
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
            "Last brief",
            "Tap to reopen",
            "grant search first",
            "docs + mail + files + online",
            "7 writable",
            "7 read-only",
            "Inbox empty, or email.search not granted.",
            "Bridge offline - run: make utm-bridged",
            "bridge connected",
            "bridge offline",
            "os",
            "setup",
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
        let mut src = [0u8; 40];
        let mut caps = Caps::none();
        caps.set(Cap::SearchQuery, true);
        caps.set(Cap::EmailSearch, true);
        caps.set(Cap::WorkspaceIndex, true);
        caps.set(Cap::PortalSync, true);
        let sub = search_tile_sub(caps, &mut src);
        assert!(SMALL_FACE.width(sub, 0) < r.w - 36, "search sub overflows: {sub}");
        let mut sbuf = [0u8; 28];
        let skill_sub = skills_tile_sub(caps, 7, &mut sbuf);
        assert!(
            SMALL_FACE.width(skill_sub, 0) < r.w - 36,
            "skills sub overflows: {skill_sub}"
        );
        let mut buf = [0u8; 96];
        let n = Caps::default_grants().describe(&mut buf);
        let status = core::str::from_utf8(&buf[..n]).unwrap();
        assert!(SMALL_FACE.width(status, 0) < r.w - 36, "status overruns the tile: {status}");
    }

    #[test]
    fn search_tile_sub_follows_grants() {
        let mut buf = [0u8; 40];
        assert_eq!(search_tile_sub(Caps::none(), &mut buf), "grant search first");
        let mut caps = Caps::none();
        caps.set(Cap::SearchQuery, true);
        assert_eq!(search_tile_sub(caps, &mut buf), "docs");
        caps.set(Cap::PortalSync, true);
        assert_eq!(search_tile_sub(caps, &mut buf), "docs + online");
    }

    #[test]
    fn skills_tile_sub_names_writability() {
        let mut buf = [0u8; 28];
        assert_eq!(skills_tile_sub(Caps::none(), 7, &mut buf), "7 read-only");
        let mut caps = Caps::none();
        caps.set(Cap::SkillsSave, true);
        assert_eq!(skills_tile_sub(caps, 7, &mut buf), "7 writable");
        assert_eq!(skills_tile_sub(caps, 12, &mut buf), "9+ writable");
    }

    #[test]
    fn brief_strip_is_hittable_when_a_report_exists() {
        // capability-safe-tools never touches COM2 — safe in host unit tests.
        let brief = crate::agent::run("capability-safe-tools", Caps::default_grants());
        assert!(brief.has_report());
        let r = brief_rect(1024, 768, &brief);
        assert!(r.w > 0 && r.h > 0);
        let t = home_targets(1024, 768, &peek(), &brief);
        assert_eq!(t.hit(r.x + 8, r.y + 8), Some(HomeHit::Brief));
        let empty = home_targets(1024, 768, &peek(), &empty_brief());
        assert_ne!(empty.hit(r.x + 8, r.y + 8), Some(HomeHit::Brief));
    }

    #[test]
    fn drawing_the_home_screen_does_not_panic() {
        // Exercises the offline branch and the count formatting together.
        let mail = MailPeek::empty(BridgeStatus::Offline);
        let _ = home_targets(1024, 768, &peek(), &empty_brief());
        assert_eq!(mail.count, 0);
    }
}
