//! Home screen — cool mist, brand-first, one CTA. ASCII-only (8x8 font).

use crate::fb::Surface;
use crate::mcp::{BridgeStatus, MailPeek};
use crate::skills::SkillPeek;

pub mod theme {
    pub const BG_TOP: u32 = 0x00E8_EEF4;
    pub const BG_BOT: u32 = 0x00FF_FFFF;
    pub const INK: u32 = 0x000E_1621;
    pub const INK_MUTED: u32 = 0x005A_6674;
    pub const INK_FAINT: u32 = 0x009A_A5B0;
    pub const SIGNAL: u32 = 0x0000_6ACC;
    pub const SIGNAL_DEEP: u32 = 0x0000_4F9E;
    pub const RULE: u32 = 0x00D5_DCE4;
    pub const ONLINE: u32 = 0x001D_9A6C;
    pub const OFFLINE: u32 = 0x00C4_3C3C;
}

/// Draw the first-viewport home composition on `fb`.
pub fn draw_home(fb: &Surface, mail: &MailPeek, skills: &SkillPeek) {
    let w = fb.width() as i32;
    let h = fb.height() as i32;
    let _ = skills;

    paint_atmosphere(fb, w, h);

    // Top chrome: quiet, not the brand.
    fb.fill_rect(0, 0, w, 1, theme::RULE);
    fb.draw_text(28, 20, "os", 1, theme::INK_FAINT);
    let status = match mail.status {
        BridgeStatus::Online => "bridge on",
        BridgeStatus::Offline => "bridge off",
    };
    let st_w = Surface::text_width(status, 1);
    let dot = match mail.status {
        BridgeStatus::Online => theme::ONLINE,
        BridgeStatus::Offline => theme::OFFLINE,
    };
    fb.fill_round_rect(w - 28 - st_w - 16, 22, 8, 8, 4, dot);
    fb.draw_text(w - 28 - st_w, 20, status, 1, theme::INK_FAINT);

    // Hero — only ASCII (bitmap font is 0x20..=0x7E).
    let brand_scale = brand_scale_for(w);
    let head = "Agents with explicit authority.";
    let line = "Caps, skills, host connectors. No ambient root.";
    let head_scale = if w >= 1000 { 2 } else { 1 };

    let brand_h = Surface::text_height(brand_scale);
    let head_h = Surface::text_height(head_scale);
    let line_h = Surface::text_height(1);
    let cta_h = 48i32;
    let gap_brand = 32;
    let gap_head = 18;
    let gap_cta = 36;
    let block = brand_h + gap_brand + head_h + gap_head + line_h + gap_cta + cta_h;
    let mut y = ((h - block) / 2 - 8).max(56);

    // Brand as type (clean) — geometric logo was reading broken at small FB sizes.
    fb.draw_text_centered(w / 2, y, "os", brand_scale, theme::INK);
    // Signal underline under brand
    let uw = Surface::text_width("os", brand_scale) * 2 / 3;
    fb.fill_rect(w / 2 - uw / 2, y + brand_h + 6, uw, 3, theme::SIGNAL);
    y += brand_h + gap_brand;

    fb.draw_text_centered(w / 2, y, head, head_scale, theme::INK);
    y += head_h + gap_head;

    fb.draw_text_centered(w / 2, y, line, 1, theme::INK_MUTED);
    y += line_h + gap_cta;

    let cta = "Ready";
    let cta_scale = 2;
    let cta_tw = Surface::text_width(cta, cta_scale);
    let pad_x = 36;
    let cta_w = cta_tw + pad_x * 2;
    let cta_x = w / 2 - cta_w / 2;
    fb.fill_round_rect(cta_x + 2, y + 3, cta_w, cta_h, cta_h / 2, theme::SIGNAL_DEEP);
    fb.fill_round_rect(cta_x, y, cta_w, cta_h, cta_h / 2, theme::SIGNAL);
    let ty = y + (cta_h - Surface::text_height(cta_scale)) / 2;
    fb.draw_text_centered(w / 2, ty, cta, cta_scale, theme::BG_BOT);

    fb.draw_text_centered(w / 2, h - 28, "v0.6.2  |  limine  |  x86_64", 1, theme::INK_FAINT);
}

fn brand_scale_for(w: i32) -> usize {
    if w >= 1400 {
        8
    } else if w >= 1000 {
        7
    } else if w >= 800 {
        6
    } else {
        5
    }
}

fn paint_atmosphere(fb: &Surface, w: i32, h: i32) {
    // Banded gradient (fast, no per-pixel rings) — cool mist into white.
    const BANDS: i32 = 32;
    let bh = (h + BANDS - 1) / BANDS;
    for i in 0..BANDS {
        let t = (i * 256) / (BANDS - 1).max(1);
        let c = lerp_rgb(theme::BG_TOP, theme::BG_BOT, t);
        fb.fill_rect(0, i * bh, w, bh + 1, c);
    }
}

fn lerp_rgb(a: u32, b: u32, t256: i32) -> u32 {
    let t = t256.clamp(0, 256);
    let lerp = |ca: u32, cb: u32| -> u32 {
        let ca = ca as i32;
        let cb = cb as i32;
        (ca + ((cb - ca) * t) / 256) as u32
    };
    let ar = (a >> 16) & 0xff;
    let ag = (a >> 8) & 0xff;
    let ab = a & 0xff;
    let br = (b >> 16) & 0xff;
    let bg = (b >> 8) & 0xff;
    let bb = b & 0xff;
    (lerp(ar, br) << 16) | (lerp(ag, bg) << 8) | lerp(ab, bb)
}
