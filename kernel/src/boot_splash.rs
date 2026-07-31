//! Boot splash — animated connection sequence.
//!
//! Plays once at startup while the kernel probes the host bridge over COM2.
//! All math is fixed-point integer only (no FPU). The hub-and-spoke layout
//! draws "teddy OS" at the centre with four service nodes orbiting it:
//! Search, Knowledge, Skills, and Mail. Beams grow outward as each service
//! connects, driven by `anim::ease_out_cubic`.
//!
//! # Phases  (total ≈ 110 frames @ 60 Hz ≈ 1.8 s)
//! 0..18    Hub fades and scales in.
//! 12..60   Four beams grow outward, staggered by 8 frames each.
//! 40..80   Node labels pop in, each offset by 8 frames.
//! 70..90   Whole diagram pulses once (lerp_color ACCENT→ONLINE).
//! 90..110  Smooth fade to black, then the caller takes over.

use crate::anim::{ease_out_cubic, lerp, lerp_color, ONE};
use crate::fb::Surface;
use crate::font::{BRAND_FACE, SMALL_FACE};
use crate::ui::theme;

pub const TOTAL_FRAMES: u32 = 110;

/// Integer square-root (Newton iterations, no FPU).
fn isqrt(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let n64 = n as u64;
    let mut x = n64;
    loop {
        let next = (x + n64 / x) / 2;
        if next >= x {
            break;
        }
        x = next;
    }
    x as i32
}

/// Q16 progress of `frame` through the window `[start, start+dur)`, clamped 0..ONE.
fn window(frame: u32, start: u32, dur: u32) -> i32 {
    if frame < start {
        return 0;
    }
    let elapsed = (frame - start).min(dur) as i64;
    ((elapsed * ONE as i64) / dur as i64) as i32
}

// Colours
const BLACK: u32 = 0x0000_0000;
const NODE_LABEL_OFFSET: i32 = 30; // px below/beside a node circle

struct Node {
    dx: i32, // offset from centre (px)
    dy: i32,
    label: &'static str,
    beam_start: u32, // frame at which beam begins growing
    label_start: u32,
}

const NODES: [Node; 4] = [
    Node { dx: 0, dy: -155, label: "Search",    beam_start: 12, label_start: 42 },
    Node { dx: 210, dy: 0,  label: "Knowledge", beam_start: 20, label_start: 50 },
    Node { dx: 0, dy: 155,  label: "Skills",    beam_start: 28, label_start: 58 },
    Node { dx: -210, dy: 0, label: "Mail",      beam_start: 36, label_start: 66 },
];

/// Draw one frame of the boot splash.  Returns `true` while still animating.
pub fn draw_frame(fb: &Surface, frame: u32) -> bool {
    let w = fb.width() as i32;
    let h = fb.height() as i32;
    let cx = w / 2;
    let cy = h / 2;

    // ── fade-out at end ─────────────────────────────────────────────────────
    // Erase the screen each frame with a colour that transitions to black.
    let fade_t = ease_out_cubic(window(frame, 90, 20));
    let bg = lerp_color(theme::BG, BLACK, fade_t);
    fb.fill(bg);

    if frame >= TOTAL_FRAMES {
        return false;
    }

    let fade_in_alpha = ease_out_cubic(window(frame, 0, 18).min(ONE)); // 0..ONE

    // ── pulse colour (frames 70-90) ──────────────────────────────────────────
    let pulse_t = {
        let p = window(frame, 70, 20);
        // triangle: up 0..10, back 10..20
        if p < ONE / 2 {
            ease_out_cubic(p * 2)
        } else {
            ease_out_cubic(ONE - (p - ONE / 2) * 2)
        }
    };
    let accent = lerp_color(theme::ACCENT, theme::ONLINE, pulse_t);

    // ── hub circle ──────────────────────────────────────────────────────────
    let hub_r = lerp(6, 44, ease_out_cubic(window(frame, 0, 14)));
    let hub_alpha = (fade_in_alpha as u32 * 255 / ONE as u32).min(255);
    // Glow ring (slightly larger, transparent-ish via alpha blend)
    if hub_alpha > 30 {
        let glow_r = hub_r + 6;
        let glow_c = lerp_color(bg, accent, (fade_in_alpha / 4).min(ONE / 3));
        fb.fill_round_rect(cx - glow_r, cy - glow_r, glow_r * 2, glow_r * 2, glow_r, glow_c);
    }
    // Main hub
    let hub_c = lerp_color(bg, accent, fade_in_alpha);
    fb.fill_round_rect(cx - hub_r, cy - hub_r, hub_r * 2, hub_r * 2, hub_r, hub_c);

    // "os" wordmark inside hub
    if hub_r >= 30 && hub_alpha > 80 {
        let label_c = lerp_color(hub_c, theme::BG, fade_in_alpha);
        fb.draw_text_centered(
            cx,
            cy + BRAND_FACE.baseline() - BRAND_FACE.px / 2,
            "os",
            &BRAND_FACE,
            0,
            label_c,
        );
    }

    // ── beams & nodes ────────────────────────────────────────────────────────
    for node in &NODES {
        let nx = cx + node.dx;
        let ny = cy + node.dy;

        // Beam growth
        let beam_t = ease_out_cubic(window(frame, node.beam_start, 24));
        if beam_t > 0 {
            let dx = node.dx;
            let dy = node.dy;
            let full_len = isqrt(dx * dx + dy * dy);
            if full_len > 0 {
                let grown = lerp(0, full_len - hub_r - 14, beam_t); // stop short of node
                let beam_alpha = lerp_color(bg, accent, fade_in_alpha);

                // Draw beam as a 2px-wide line via thin rect.
                // For cardinal directions this is perfect; we use the dominant axis.
                let beam_x = lerp(cx, nx, beam_t);
                let beam_y = lerp(cy, ny, beam_t);
                if dx.abs() > dy.abs() {
                    // horizontal beam
                    let bx = cx + if dx > 0 { hub_r } else { -grown - 2 };
                    fb.fill_rect(bx, cy - 1, grown, 2, beam_alpha);
                } else {
                    // vertical beam
                    let by_ = cy + if dy > 0 { hub_r } else { -grown - 2 };
                    fb.fill_rect(cx - 1, by_, 2, grown, beam_alpha);
                }
                let _ = (beam_x, beam_y);
            }
        }

        // Node circle (pops in when beam has grown far enough)
        if beam_t >= ONE * 3 / 4 {
            let node_t = ease_out_cubic(((beam_t - ONE * 3 / 4) * 4).min(ONE));
            let node_r = lerp(0, 14, node_t);
            if node_r > 0 {
                let node_c = lerp_color(bg, accent, node_t);
                fb.fill_round_rect(nx - node_r, ny - node_r, node_r * 2, node_r * 2, node_r, node_c);
                // Hollow inner (ring effect)
                let inner_r = (node_r - 3).max(0);
                if inner_r > 0 {
                    let inner_c = lerp_color(node_c, bg, ONE * 2 / 3);
                    fb.fill_round_rect(
                        nx - inner_r,
                        ny - inner_r,
                        inner_r * 2,
                        inner_r * 2,
                        inner_r,
                        inner_c,
                    );
                }
            }
        }

        // Label
        let label_t = ease_out_cubic(window(frame, node.label_start, 14));
        if label_t > 0 {
            let lc = lerp_color(bg, theme::INK, label_t);
            let lx = nx;
            // Place label on the far side of the node from the hub
            let ly = if node.dy < 0 {
                ny - 14 - NODE_LABEL_OFFSET
            } else if node.dy > 0 {
                ny + 14 + 8
            } else if node.dx > 0 {
                ny + SMALL_FACE.px / 2
            } else {
                ny + SMALL_FACE.px / 2
            };
            fb.draw_text_centered(lx, ly + SMALL_FACE.baseline(), node.label, &SMALL_FACE, 0, lc);
        }
    }

    // ── tagline ─────────────────────────────────────────────────────────────
    let tag_t = ease_out_cubic(window(frame, 55, 18));
    if tag_t > 0 {
        let tag_c = lerp_color(bg, theme::MUTED, tag_t);
        fb.draw_text_centered(
            cx,
            cy + 110 + SMALL_FACE.baseline(),
            "connecting knowledge",
            &SMALL_FACE,
            0,
            tag_c,
        );
    }

    // ── bottom: "starting..." dots ─────────────────────────────────────────
    let dot_t = ease_out_cubic(window(frame, 18, 10));
    if dot_t > 0 {
        let dc = lerp_color(bg, theme::RULE, dot_t);
        let dots = ((frame - 18) / 8 % 4) as usize; // 0..3 cycling dots
        let dot_labels = ["", ".", "..", "..."];
        let label = dot_labels[dots];
        let status = if frame >= 70 { "connected" } else { "connecting" };
        let mut buf = [0u8; 32];
        let sb = status.as_bytes();
        let lb = label.as_bytes();
        let n = sb.len().min(buf.len());
        buf[..n].copy_from_slice(&sb[..n]);
        let m = (n + lb.len()).min(buf.len());
        buf[n..m].copy_from_slice(&lb[..m - n]);
        let s = core::str::from_utf8(&buf[..m]).unwrap_or("connecting");
        fb.draw_text_centered(cx, h - 60 + SMALL_FACE.baseline(), s, &SMALL_FACE, 0, dc);
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isqrt_basic() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(4), 2);
        assert_eq!(isqrt(9), 3);
        assert_eq!(isqrt(100), 10);
        assert_eq!(isqrt(155 * 155), 155);
    }

    #[test]
    fn window_clamps() {
        assert_eq!(window(0, 10, 20), 0);
        assert_eq!(window(10, 10, 20), 0);
        assert_eq!(window(20, 10, 20), ONE / 2);
        assert_eq!(window(30, 10, 20), ONE);
        assert_eq!(window(100, 10, 20), ONE);
    }

    #[test]
    fn total_frames_terminates() {
        // draw_frame must return false at or before TOTAL_FRAMES.
        let mut buf = vec![0u32; 1280 * 800];
        let fb = unsafe { crate::fb::Surface::in_memory(buf.as_mut_ptr(), 1280, 800) };
        // Spot-check a few key frames.
        assert!(draw_frame(&fb, 0));
        assert!(draw_frame(&fb, TOTAL_FRAMES - 1));
        assert!(!draw_frame(&fb, TOTAL_FRAMES));
    }

    #[test]
    fn animation_does_not_panic_on_small_screen() {
        let mut buf = vec![0u32; 800 * 600];
        let fb = unsafe { crate::fb::Surface::in_memory(buf.as_mut_ptr(), 800, 600) };
        for f in 0..TOTAL_FRAMES {
            draw_frame(&fb, f);
        }
    }

    #[test]
    fn nodes_are_symmetric_about_centre() {
        // Top and bottom dx==0; Left and Right dy==0.
        assert_eq!(NODES[0].dx, 0, "Search must be above centre");
        assert_eq!(NODES[2].dx, 0, "Skills must be below centre");
        assert_eq!(NODES[1].dy, 0, "Knowledge must be right");
        assert_eq!(NODES[3].dy, 0, "Mail must be left");
        // Beams start in the right order.
        for i in 1..NODES.len() {
            assert!(
                NODES[i].beam_start > NODES[i - 1].beam_start,
                "node {i} beam must start after node {}",
                i - 1
            );
        }
    }

    #[test]
    fn isqrt_edge_cases() {
        assert_eq!(isqrt(-5), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(2), 1);
        assert_eq!(isqrt(3), 1);
        assert_eq!(isqrt(65536), 256);
        assert_eq!(isqrt(2147483647), 46340);
    }

    #[test]
    fn window_edge_cases() {
        assert_eq!(window(0, 0, 10), 0);
        assert_eq!(window(5, 0, 10), ONE / 2);
        assert_eq!(window(10, 0, 10), ONE);
        assert_eq!(window(50, 0, 10), ONE);
    }

    #[test]
    fn draw_frame_all_phases_rendering() {
        let mut buf = vec![0u32; 1024 * 768];
        let fb = unsafe { crate::fb::Surface::in_memory(buf.as_mut_ptr(), 1024, 768) };
        for frame in [0, 15, 30, 45, 60, 75, 90, 105, 110] {
            let active = draw_frame(&fb, frame);
            if frame < TOTAL_FRAMES {
                assert!(active, "frame {frame} should be active");
            } else {
                assert!(!active, "frame {frame} should be finished");
            }
        }
    }

    #[test]
    fn ease_out_cubic_bounds_and_monotonicity() {
        assert_eq!(ease_out_cubic(0), 0);
        assert_eq!(ease_out_cubic(ONE), ONE);
        let e1 = ease_out_cubic(ONE / 4);
        let e2 = ease_out_cubic(ONE / 2);
        let e3 = ease_out_cubic(3 * ONE / 4);
        assert!(e1 > 0 && e1 < e2);
        assert!(e2 < e3 && e3 < ONE);
    }
}


