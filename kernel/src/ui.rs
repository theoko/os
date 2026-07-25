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
use crate::mcp::{BridgeStatus, FilePeek, MailPeek};
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

pub const NAV_H: i32 = 56;

/// Soft nav hairline that breathes with the 60 Hz chill loop.
///
/// Cheap: one 1px strip. Keeps the UI alive when the pointer is still —
/// the difference between a frozen form and a quiet game menu.
pub fn paint_chill_rule(fb: &Surface, w: i32, phase: u32) {
    let t = crate::anim::breath(phase);
    // Keep it subtle: at most ~35% of the way from RULE toward ACCENT.
    let amt = (t as i64 * (ONE_CHILL as i64) / (crate::anim::ONE as i64)) as i32;
    let c = crate::anim::lerp_color(theme::RULE, theme::ACCENT, amt);
    fb.fill_rect(0, NAV_H, w, 1, c);
}

/// Peak blend strength for the chill rule (Q16) — about a third of the way.
const ONE_CHILL: i32 = crate::anim::ONE / 3;
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
    /// Open a Recent mail row (`email://` via the peek's graph id).
    Mail(usize),
    /// Open a Recent files row (`file://` via workspace.recent).
    File(usize),
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
    pub mail: [Rect; 3],
    pub mail_n: usize,
    pub files: [Rect; 3],
    pub files_n: usize,
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
        for i in 0..self.mail_n.min(self.mail.len()) {
            if self.mail[i].w > 0 && self.mail[i].contains(px, py) {
                return Some(HomeHit::Mail(i));
            }
        }
        for i in 0..self.files_n.min(self.files.len()) {
            if self.files[i].w > 0 && self.files[i].contains(px, py) {
                return Some(HomeHit::File(i));
            }
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
    files: &FilePeek,
    skills: &SkillPeek,
    status: &str,
    caps: Caps,
    brief: &Brief,
    level: crate::level::Level,
) {
    draw_home_full(fb, mail, files, skills, status, "", false, caps, brief, level)
}

/// The home screen: a launcher, not a landing page.
///
/// A search field you can type into immediately, three destinations carrying
/// live counts, the last Brief when a skill has run, and recent mail when
/// granted.
pub fn draw_home_full(
    fb: &Surface,
    mail: &MailPeek,
    files: &FilePeek,
    skills: &SkillPeek,
    status: &str,
    query: &str,
    caret: bool,
    caps: Caps,
    brief: &Brief,
    level: crate::level::Level,
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
        fb.draw_text(
            fx + 18,
            base,
            level.home_search_placeholder(),
            &BODY_FACE,
            0,
            theme::MUTED,
        );
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
        level.home_search_hint(),
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

    // Last brief stays after Back; mail + files still show below when granted
    // so a morning brief cannot starve the inbox forever.
    let ry = ty + TILE_H + 40;
    let mut y = ry;
    if brief.has_report() {
        y = draw_brief_residue(fb, x0, ry, cw, brief);
        y += 16;
    }
    let mail_n = home_mail_n(brief, files);
    let files_n = home_files_n(brief, mail);
    if mail.count > 0 {
        fb.draw_text(x0, y, "Recent mail", &BRAND_FACE, 0, theme::INK);
        y += 28;
        for i in 0..mail.count.min(mail_n) {
            fb.draw_text(x0, y, mail.row_subj(i), &BODY_FACE, 0, theme::INK);
            let from = mail.row_from(i);
            fb.draw_text(x0 + cw - SMALL_FACE.width(from, 0), y, from, &SMALL_FACE, 0, theme::MUTED);
            y += 12;
            fb.fill_rect(x0, y, cw, 1, theme::CARD_BORDER);
            y += 22;
        }
    } else if !brief.has_report() && files.count == 0 {
        let empty_mail = match mail.status {
            BridgeStatus::Offline => "Bridge offline - run: make utm-bridged",
            BridgeStatus::Online if caps.allows(Cap::EmailSearch) => "Inbox empty right now.",
            BridgeStatus::Online => "Grant Email to show recent mail.",
        };
        fb.draw_text(x0, y, empty_mail, &SMALL_FACE, 0, theme::MUTED);
        y += 22;
    }
    if files.count > 0 {
        if mail.count > 0 {
            y += 8;
        }
        fb.draw_text(x0, y, "Recent files", &BRAND_FACE, 0, theme::INK);
        y += 28;
        for i in 0..files.count.min(files_n) {
            fb.draw_text(x0, y, files.title_at(i), &BODY_FACE, 0, theme::INK);
            y += 12;
            fb.fill_rect(x0, y, cw, 1, theme::CARD_BORDER);
            y += 22;
        }
    }

    fb.draw_text_centered(w / 2, h - 24, mail_label, &SMALL_FACE, 0, theme::MUTED);
}

fn home_mail_n(brief: &Brief, files: &FilePeek) -> usize {
    if brief.has_report() || files.count > 0 {
        2
    } else {
        3
    }
}

fn home_files_n(brief: &Brief, mail: &MailPeek) -> usize {
    if brief.has_report() || mail.count > 0 {
        2
    } else {
        3
    }
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
    if caps.allows(Cap::AudioTranscribe) {
        if n > 0 {
            push(" + ", &mut n);
        }
        push("audio", &mut n);
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

pub fn home_targets(
    w: i32,
    h: i32,
    skills: &SkillPeek,
    brief: &Brief,
    mail: &MailPeek,
    files: &FilePeek,
) -> HomeTargets {
    let (mail_rects, mail_n) = mail_targets(w, h, brief, mail, files);
    let (file_rects, files_n) = file_targets(w, h, brief, mail, files);
    HomeTargets {
        ctas: cta_targets(w, h, skills),
        cards: card_targets(w, h),
        brief: brief_rect(w, h, brief),
        mail: mail_rects,
        mail_n,
        files: file_rects,
        files_n,
    }
}

/// Y origin of the Recent mail / files stack (heading), matching `draw_home_full`.
fn mail_block_top(_w: i32, h: i32, brief: &Brief) -> i32 {
    let ry = tile_top(h) + TILE_H + 40;
    if !brief.has_report() {
        return ry;
    }
    // Same end Y as `draw_brief_residue`, then the 16px gap before mail/files.
    let lines = brief.count.min(2) as i32;
    ry + 52 + lines * 20 + 16
}

const PEEK_ROW_PITCH: i32 = 34;
const PEEK_HEADING: i32 = 28;

/// Hit boxes for Recent mail rows. Empty when there is nothing to open.
pub fn mail_targets(
    w: i32,
    h: i32,
    brief: &Brief,
    mail: &MailPeek,
    files: &FilePeek,
) -> ([Rect; 3], usize) {
    let mut rects = [Rect {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    }; 3];
    if mail.count == 0 {
        return (rects, 0);
    }
    let (x0, cw) = home_column(w);
    let mail_n = home_mail_n(brief, files);
    let n = mail.count.min(mail_n).min(rects.len());
    // Heading ("Recent mail") is 28px; each row is subject + rule = 34px.
    let y0 = mail_block_top(w, h, brief) + PEEK_HEADING;
    for i in 0..n {
        rects[i] = Rect {
            x: x0,
            y: y0 + i as i32 * PEEK_ROW_PITCH,
            w: cw,
            h: PEEK_ROW_PITCH,
        };
    }
    (rects, n)
}

/// Y origin of the Recent files heading, matching `draw_home_full`.
fn files_block_top(w: i32, h: i32, brief: &Brief, mail: &MailPeek, files: &FilePeek) -> i32 {
    let y = mail_block_top(w, h, brief);
    if mail.count == 0 {
        return y;
    }
    let shown = mail.count.min(home_mail_n(brief, files));
    y + PEEK_HEADING + shown as i32 * PEEK_ROW_PITCH + 8
}

/// Hit boxes for Recent files rows. Empty when there is nothing to open.
pub fn file_targets(
    w: i32,
    h: i32,
    brief: &Brief,
    mail: &MailPeek,
    files: &FilePeek,
) -> ([Rect; 3], usize) {
    let mut rects = [Rect {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    }; 3];
    if files.count == 0 {
        return (rects, 0);
    }
    let (x0, cw) = home_column(w);
    let files_n = home_files_n(brief, mail);
    let n = files.count.min(files_n).min(rects.len());
    let y0 = files_block_top(w, h, brief, mail, files) + PEEK_HEADING;
    for i in 0..n {
        rects[i] = Rect {
            x: x0,
            y: y0 + i as i32 * PEEK_ROW_PITCH,
            w: cw,
            h: PEEK_ROW_PITCH,
        };
    }
    (rects, n)
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
    use crate::mcp::{FilePeek, MailPeek};

    fn peek() -> SkillPeek {
        SkillPeek::from_builtin()
    }

    fn empty_brief() -> Brief {
        Brief::empty()
    }

    #[test]
    fn search_field_is_not_a_cta() {
        // Clicking the query box used to fire CtaId::Ready and restart setup.
        let mail = MailPeek::empty(BridgeStatus::Offline);
        let files = FilePeek::empty(BridgeStatus::Offline, false);
        let t = home_targets(1024, 768, &peek(), &empty_brief(), &mail, &files);
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
        let mail = MailPeek::empty(BridgeStatus::Offline);
        let files = FilePeek::empty(BridgeStatus::Offline, false);
        let t = home_targets(1024, 768, &peek(), &empty_brief(), &mail, &files);
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
        let mut all = vec![
            "Search",
            "Capabilities",
            "Skills",
            "Recent mail",
            "Recent files",
            "Last brief",
            "Tap to reopen",
            "grant search first",
            "docs + mail + files + online + audio",
            "7 writable",
            "7 read-only",
            "Inbox empty right now.",
            "Grant Email to show recent mail.",
            "Bridge offline - run: make utm-bridged",
            "bridge connected",
            "bridge offline",
            "os",
            "setup",
        ];
        for level in crate::level::Level::ALL {
            all.push(level.home_search_placeholder());
            all.push(level.home_search_hint());
        }
        for s in all {
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
        caps.set(Cap::AudioTranscribe, true);
        let sub = search_tile_sub(caps, &mut src);
        assert!(SMALL_FACE.width(sub, 0) < r.w - 36, "search sub overflows: {sub}");
        assert!(sub.contains("audio"), "{sub}");
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
        caps.set(Cap::AudioTranscribe, true);
        assert_eq!(search_tile_sub(caps, &mut buf), "docs + online + audio");
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
        let mail = MailPeek::empty(BridgeStatus::Online);
        let files = FilePeek::empty(BridgeStatus::Online, false);
        let t = home_targets(1024, 768, &peek(), &brief, &mail, &files);
        assert_eq!(t.hit(r.x + 8, r.y + 8), Some(HomeHit::Brief));
        let empty = home_targets(1024, 768, &peek(), &empty_brief(), &mail, &files);
        assert_ne!(empty.hit(r.x + 8, r.y + 8), Some(HomeHit::Brief));
    }

    #[test]
    fn recent_mail_rows_are_hittable() {
        let mut mail = MailPeek::empty(BridgeStatus::Online);
        copy_ascii(&mut mail.rows[0].id, "0123456789abcdef");
        copy_ascii(&mut mail.rows[0].from, "ada@x.com");
        copy_ascii(&mut mail.rows[0].subj, "Q2 planning notes");
        copy_ascii(&mut mail.rows[1].id, "fedcba9876543210");
        copy_ascii(&mut mail.rows[1].from, "bob@x.com");
        copy_ascii(&mut mail.rows[1].subj, "Hello");
        mail.count = 2;
        let files = FilePeek::empty(BridgeStatus::Online, false);

        let (rects, n) = mail_targets(1024, 768, &empty_brief(), &mail, &files);
        assert_eq!(n, 2);
        let t = home_targets(1024, 768, &peek(), &empty_brief(), &mail, &files);
        assert_eq!(
            t.hit(rects[0].x + 8, rects[0].y + 8),
            Some(HomeHit::Mail(0))
        );
        assert_eq!(
            t.hit(rects[1].x + 8, rects[1].y + 8),
            Some(HomeHit::Mail(1))
        );
        // With a brief above, mail still hits and does not steal the brief.
        let brief = crate::agent::run("capability-safe-tools", Caps::default_grants());
        let t2 = home_targets(1024, 768, &peek(), &brief, &mail, &files);
        let br = brief_rect(1024, 768, &brief);
        assert_eq!(t2.hit(br.x + 8, br.y + 8), Some(HomeHit::Brief));
        assert_eq!(t2.mail_n, 2);
        assert_eq!(
            t2.hit(t2.mail[0].x + 8, t2.mail[0].y + 8),
            Some(HomeHit::Mail(0))
        );
    }

    #[test]
    fn recent_file_rows_are_hittable() {
        let mail = MailPeek::empty(BridgeStatus::Online);
        let mut files = FilePeek::empty(BridgeStatus::Online, false);
        copy_ascii(&mut files.rows[0].title, "notes.md");
        copy_ascii(&mut files.rows[0].url, "file://docs/notes.md");
        copy_ascii(&mut files.rows[1].title, "plan.txt");
        copy_ascii(&mut files.rows[1].url, "file://docs/plan.txt");
        files.count = 2;

        let (rects, n) = file_targets(1024, 768, &empty_brief(), &mail, &files);
        assert_eq!(n, 2);
        let t = home_targets(1024, 768, &peek(), &empty_brief(), &mail, &files);
        assert_eq!(
            t.hit(rects[0].x + 8, rects[0].y + 8),
            Some(HomeHit::File(0))
        );
        assert_eq!(
            t.hit(rects[1].x + 8, rects[1].y + 8),
            Some(HomeHit::File(1))
        );
        // Mail above files: both hit, files sit below mail.
        let mut mail2 = MailPeek::empty(BridgeStatus::Online);
        copy_ascii(&mut mail2.rows[0].id, "0123456789abcdef");
        copy_ascii(&mut mail2.rows[0].from, "ada@x.com");
        copy_ascii(&mut mail2.rows[0].subj, "Hello");
        mail2.count = 1;
        let t2 = home_targets(1024, 768, &peek(), &empty_brief(), &mail2, &files);
        assert_eq!(t2.mail_n, 1);
        assert_eq!(t2.files_n, 2);
        assert!(t2.files[0].y > t2.mail[0].y);
        assert_eq!(
            t2.hit(t2.files[0].x + 8, t2.files[0].y + 8),
            Some(HomeHit::File(0))
        );
    }

    fn copy_ascii(dst: &mut [u8], s: &str) {
        dst.fill(0);
        let n = s.len().min(dst.len());
        dst[..n].copy_from_slice(&s.as_bytes()[..n]);
    }

    #[test]
    fn chill_rule_blend_stays_between_rule_and_accent() {
        // Peak breath should not reach full accent — ambient, not flashing.
        let peak = crate::anim::breath(90);
        let amt = (peak as i64 * (ONE_CHILL as i64) / (crate::anim::ONE as i64)) as i32;
        assert!(amt > 0 && amt < crate::anim::ONE / 2, "amt={amt}");
        let c = crate::anim::lerp_color(theme::RULE, theme::ACCENT, amt);
        assert_ne!(c, theme::RULE);
        assert_ne!(c, theme::ACCENT);
    }

    #[test]
    fn drawing_the_home_screen_does_not_panic() {
        // Exercises the offline branch and the count formatting together.
        let mail = MailPeek::empty(BridgeStatus::Offline);
        let files = FilePeek::empty(BridgeStatus::Offline, false);
        let _ = home_targets(1024, 768, &peek(), &empty_brief(), &mail, &files);
        assert_eq!(mail.count, 0);
    }

    #[test]
    fn mail_url_builds_email_scheme() {
        let mut mail = MailPeek::empty(BridgeStatus::Online);
        copy_ascii(&mut mail.rows[0].id, "0123456789abcdef");
        mail.count = 1;
        let mut buf = [0u8; 40];
        assert_eq!(mail.url_at(0, &mut buf), Some("email://0123456789abcdef"));
        let empty = MailPeek::empty(BridgeStatus::Online);
        assert_eq!(empty.url_at(0, &mut buf), None);
    }
}
