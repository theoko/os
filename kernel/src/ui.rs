//! Home screen, modelled on superintelmarkets.com.
//!
//! White page, one dominant display line at tight tracking, muted supporting
//! copy, a pill CTA pair, then a bordered card row. All type is anti-aliased
//! proportional (see `font.rs`); all copy is ASCII because the atlas covers
//! 0x20..=0x7E only.

use crate::fb::Surface;
use crate::font::{self, BODY_FACE, BRAND_FACE, BTN_FACE, H2_FACE, HERO_FACE, SMALL_FACE};
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
const CARD_GUTTER: i32 = 20;
const CARD_H: i32 = 140;
const CTA_H: i32 = 46;
const CONTENT_MAX: i32 = 920;

const HERO_TO_SUB: i32 = 34;
const SUB_GAP: i32 = 30;
const SUB_TO_CTA: i32 = 40;
const CTA_TO_H2: i32 = 84;
const H2_TO_CARDS: i32 = 26;

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
}

/// Hit targets for the CTA pair, computed with the same layout as `draw_home`.
#[derive(Clone, Copy, Debug)]
pub struct CtaTargets {
    pub ready: Rect,
    pub skills: Rect,
}

impl CtaTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<CtaId> {
        if self.ready.contains(px, py) {
            Some(CtaId::Ready)
        } else if self.skills.contains(px, py) {
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
}

impl HomeTargets {
    pub fn hit(self, px: i32, py: i32) -> Option<HomeHit> {
        if let Some(c) = self.ctas.hit(px, py) {
            return Some(HomeHit::Cta(c));
        }
        self.cards.hit(px, py).map(HomeHit::Card)
    }
}

/// Card copy — what the OS actually wires up.
const CARDS: [(&str, &str, &str); 3] = [
    (
        "Connectors",
        "Email, search and skills reach",
        "the host bridge over COM2.",
    ),
    (
        "Capabilities",
        "Tools sit behind explicit caps.",
        "No ambient root, ever.",
    ),
    (
        "Skills",
        "Markdown playbooks the agent",
        "loads - not privileged code.",
    ),
];

/// Draw the home composition on `fb`.
///
/// `status` is a short footer line (ASCII); empty falls back to the version bar.
pub fn draw_home(fb: &Surface, mail: &MailPeek, skills: &SkillPeek, status: &str) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;

    fb.fill(theme::BG);
    draw_nav(fb, w, mail);

    // Content column, capped so the hero never sprawls on wide framebuffers.
    let content_w = (w - PAD_X * 2).min(CONTENT_MAX);
    let x0 = (w - content_w) / 2;

    let hero_track = font::tracking_pct(HERO_FACE.px, -30); // -3%, as on the site
    let h2_track = font::tracking_pct(H2_FACE.px, -15);

    let block_h = content_block_h();

    // Centre the stack in the space between the nav rule and the footer, with
    // a slight upward bias. Dividing by 3 (as a first cut did) dumps ~140px of
    // dead air under the cards.
    let avail = h - NAV_H - 70;
    let mut y = NAV_H + (((avail - block_h) * 9) / 20).max(24);

    // --- hero ---
    y += HERO_FACE.baseline();
    fb.draw_text_centered(
        w / 2,
        y,
        "Agents with explicit authority.",
        &HERO_FACE,
        hero_track,
        theme::INK,
    );

    y += HERO_TO_SUB + BODY_FACE.baseline();
    fb.draw_text_centered(
        w / 2,
        y,
        "Capability-scoped agents, host connectors on one port,",
        &BODY_FACE,
        0,
        theme::MUTED,
    );
    y += SUB_GAP;
    fb.draw_text_centered(
        w / 2,
        y,
        "and skills that stay out of the kernel.",
        &BODY_FACE,
        0,
        theme::MUTED,
    );

    // --- CTA pair ---
    y += SUB_TO_CTA;
    draw_ctas(fb, w / 2, y, skills);
    y += CTA_H;

    // --- section ---
    y += CTA_TO_H2 + H2_FACE.baseline();
    fb.draw_text(x0, y, "What's wired", &H2_FACE, h2_track, theme::INK);

    y += H2_TO_CARDS;
    let card_w = (content_w - CARD_GUTTER * 2) / 3;
    for (i, (title, l1, l2)) in CARDS.iter().enumerate() {
        let cx = x0 + (card_w + CARD_GUTTER) * i as i32;
        draw_card(fb, cx, y, card_w, CARD_H, title, l1, l2);
    }

    // --- footer ---
    let foot = if status.is_empty() {
        "os 0.7.6  |  limine  |  x86_64"
    } else {
        status
    };
    fb.draw_text_centered(w / 2, h - 30, foot, &SMALL_FACE, 0, theme::MUTED);
}

fn content_block_h() -> i32 {
    HERO_FACE.px
        + HERO_TO_SUB
        + BODY_FACE.px
        + SUB_GAP
        + SUB_TO_CTA
        + CTA_H
        + CTA_TO_H2
        + H2_FACE.px
        + H2_TO_CARDS
        + CARD_H
}

/// CTA top edge for a framebuffer of height `h` — shared by draw + hit-test.
fn cta_top(h: i32) -> i32 {
    let avail = h - NAV_H - 70;
    let y0 = NAV_H + (((avail - content_block_h()) * 9) / 20).max(24);
    y0 + HERO_FACE.baseline()
        + HERO_TO_SUB
        + BODY_FACE.baseline()
        + SUB_GAP
        + SUB_TO_CTA
}

/// Top edge of the card row — same arithmetic as `draw_home`.
fn cards_top(h: i32) -> i32 {
    cta_top(h) + CTA_H + CTA_TO_H2 + H2_FACE.baseline() + H2_TO_CARDS
}

fn skills_label(skills: &SkillPeek) -> &'static str {
    match skills.count {
        0 => "No skills",
        1 => "1 skill",
        _ => "Skills loaded",
    }
}

/// Hit regions for the Ready / Skills pills on a `w`×`h` framebuffer.
pub fn cta_targets(w: i32, h: i32, skills: &SkillPeek) -> CtaTargets {
    let y = cta_top(h);
    let primary = "Ready";
    let secondary = skills_label(skills);
    let pad = 30;
    let pw = BTN_FACE.width(primary, 0) + pad * 2;
    let sw = BTN_FACE.width(secondary, 0) + pad * 2;
    let gap = 12;
    let total = pw + gap + sw;
    let x = w / 2 - total / 2;
    CtaTargets {
        ready: Rect {
            x,
            y,
            w: pw,
            h: CTA_H,
        },
        skills: Rect {
            x: x + pw + gap,
            y,
            w: sw,
            h: CTA_H,
        },
    }
}

/// Hit regions for the three feature cards.
pub fn card_targets(w: i32, h: i32) -> CardTargets {
    let content_w = (w - PAD_X * 2).min(CONTENT_MAX);
    let x0 = (w - content_w) / 2;
    let card_w = (content_w - CARD_GUTTER * 2) / 3;
    let y = cards_top(h);
    CardTargets {
        connectors: Rect {
            x: x0,
            y,
            w: card_w,
            h: CARD_H,
        },
        capabilities: Rect {
            x: x0 + card_w + CARD_GUTTER,
            y,
            w: card_w,
            h: CARD_H,
        },
        skills: Rect {
            x: x0 + (card_w + CARD_GUTTER) * 2,
            y,
            w: card_w,
            h: CARD_H,
        },
    }
}

/// CTA + card hit regions for the finished home screen.
pub fn home_targets(w: i32, h: i32, skills: &SkillPeek) -> HomeTargets {
    HomeTargets {
        ctas: cta_targets(w, h, skills),
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

/// Primary pill + tinted secondary pill, centred as a unit.
fn draw_ctas(fb: &Surface, cx: i32, y: i32, skills: &SkillPeek) {
    let primary = "Ready";
    let secondary = skills_label(skills);

    let pad = 30;
    let pw = BTN_FACE.width(primary, 0) + pad * 2;
    let sw = BTN_FACE.width(secondary, 0) + pad * 2;
    let gap = 12;
    let total = pw + gap + sw;
    let x = cx - total / 2;
    let r = CTA_H / 2;
    let base = y + (CTA_H - BTN_FACE.px) / 2 + BTN_FACE.baseline() - 2;

    // Primary: solid accent, white label.
    fb.fill_round_rect(x, y, pw, CTA_H, r, theme::ACCENT);
    fb.draw_text_centered(x + pw / 2, base, primary, &BTN_FACE, 0, theme::BG);

    // Secondary: 6% accent fill inside a 20% accent hairline.
    let sx = x + pw + gap;
    fb.fill_round_rect(sx, y, sw, CTA_H, r, theme::TINT_BORDER);
    fb.fill_round_rect(sx + 1, y + 1, sw - 2, CTA_H - 2, r - 1, theme::TINT_BG);
    fb.draw_text_centered(sx + sw / 2, base, secondary, &BTN_FACE, 0, theme::ACCENT);
}

/// Bordered card: hairline rounded rect, then the page colour inset by 1px.
fn draw_card(fb: &Surface, x: i32, y: i32, w: i32, h: i32, title: &str, l1: &str, l2: &str) {
    const R: i32 = 12;
    fb.fill_round_rect(x, y, w, h, R, theme::CARD_BORDER);
    fb.fill_round_rect(x + 1, y + 1, w - 2, h - 2, R - 1, theme::BG);

    let pad = 22;
    let mut ty = y + pad + BRAND_FACE.baseline();
    fb.draw_text(x + pad, ty, title, &BRAND_FACE, 0, theme::INK);

    ty += 34;
    fb.draw_text(x + pad, ty, l1, &SMALL_FACE, 0, theme::MUTED);
    ty += 22;
    fb.draw_text(x + pad, ty, l2, &SMALL_FACE, 0, theme::MUTED);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_copy() -> Vec<&'static str> {
        let mut all = vec![
            "Agents with explicit authority.",
            "Capability-scoped agents, host connectors on one port,",
            "and skills that stay out of the kernel.",
            "What's wired",
            "os 0.7.6  |  limine  |  x86_64",
            "bridge connected",
            "bridge offline",
            "Ready",
            "Skills loaded",
            "No skills",
            "1 skill",
            "os",
        ];
        for (t, a, b) in CARDS {
            all.push(t);
            all.push(a);
            all.push(b);
        }
        all
    }

    #[test]
    fn copy_is_ascii_only() {
        // The atlas covers 0x20..=0x7E; anything else silently renders as '?'.
        for s in all_copy() {
            assert!(
                s.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                "non-ASCII copy would render as '?': {s:?}"
            );
        }
    }

    #[test]
    fn hero_fits_a_1024_framebuffer() {
        let track = font::tracking_pct(HERO_FACE.px, -30);
        let w = HERO_FACE.width("Agents with explicit authority.", track);
        assert!(w < 1024 - PAD_X * 2, "hero overflows 1024px: {w}");
    }

    #[test]
    fn sub_copy_fits() {
        for s in [
            "Capability-scoped agents, host connectors on one port,",
            "and skills that stay out of the kernel.",
        ] {
            let w = BODY_FACE.width(s, 0);
            assert!(w < 1024 - PAD_X * 2, "sub-copy overflows: {s:?} = {w}");
        }
    }

    #[test]
    fn card_copy_fits_its_column() {
        let content_w = (1024 - PAD_X * 2).min(CONTENT_MAX);
        let card_w = (content_w - CARD_GUTTER * 2) / 3;
        for (t, a, b) in CARDS {
            for s in [t, a, b] {
                let w = SMALL_FACE.width(s, 0);
                assert!(
                    w < card_w - 44,
                    "card text {s:?} overflows {card_w}px column: {w}"
                );
            }
        }
    }

    #[test]
    fn layout_block_fits_768_tall() {
        // Guards against the stack running off the bottom on the UTM default.
        let block_h = content_block_h();
        assert!(block_h < 768 - NAV_H - 56, "content stack too tall: {block_h}");
    }

    #[test]
    fn hero_tracking_is_negative() {
        // The 8x8 era rendered display type with huge positive tracking; the
        // reference design is tight. Guard against regressing to that.
        assert!(font::tracking_pct(HERO_FACE.px, -30) < 0);
    }

    #[test]
    fn cta_hit_regions_match_draw_layout() {
        let skills = SkillPeek {
            count: 3,
            names: [[0; 28]; 8],
        };
        let t = cta_targets(1024, 768, &skills);
        assert!(t.ready.w > 40 && t.ready.h == CTA_H);
        assert!(t.skills.w > 40 && t.skills.h == CTA_H);
        assert_eq!(t.ready.y, t.skills.y);
        assert!(t.ready.x + t.ready.w <= t.skills.x);
        // Centres of each pill hit the matching id.
        assert_eq!(
            t.hit(t.ready.x + t.ready.w / 2, t.ready.y + CTA_H / 2),
            Some(CtaId::Ready)
        );
        assert_eq!(
            t.hit(t.skills.x + t.skills.w / 2, t.skills.y + CTA_H / 2),
            Some(CtaId::Skills)
        );
        assert_eq!(t.hit(0, 0), None);
    }

    #[test]
    fn card_hit_regions_are_three_columns() {
        let skills = SkillPeek {
            count: 3,
            names: [[0; 28]; 8],
        };
        let home = home_targets(1024, 768, &skills);
        let c = home.cards;
        assert_eq!(c.connectors.y, c.capabilities.y);
        assert_eq!(c.capabilities.y, c.skills.y);
        assert!(c.connectors.x + c.connectors.w <= c.capabilities.x);
        assert!(c.capabilities.x + c.capabilities.w <= c.skills.x);
        assert_eq!(
            home.hit(
                c.connectors.x + c.connectors.w / 2,
                c.connectors.y + CARD_H / 2
            ),
            Some(HomeHit::Card(CardId::Connectors))
        );
        assert_eq!(
            home.hit(
                c.skills.x + c.skills.w / 2,
                c.skills.y + CARD_H / 2
            ),
            Some(HomeHit::Card(CardId::Skills))
        );
        // CTA still wins when aimed at Ready.
        let ready = home.ctas.ready;
        assert_eq!(
            home.hit(ready.x + ready.w / 2, ready.y + CTA_H / 2),
            Some(HomeHit::Cta(CtaId::Ready))
        );
    }
}
