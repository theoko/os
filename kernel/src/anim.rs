//! Motion — easing curves and frame pacing.
//!
//! Screens used to snap between states. A chill game loop reads as smooth
//! because things *move*, decelerate, and keep a quiet pulse at 60 Hz even
//! when the pointer is still.
//!
//! Everything is fixed point. The kernel never enables the FPU, so a cubic
//! curve is evaluated in Q16 integers rather than floats.

use crate::serial::{counter_hz, rdtsc};

/// Fixed-point one.
pub const ONE: i32 = 1 << 16;

/// One frame at 60 Hz, in microseconds.
pub const FRAME_US_60: u32 = 1_000_000 / 60;

/// Cubic ease-out: fast departure, gentle arrival.
///
/// `t` and the result are Q16 in 0..=ONE. This is the curve most system UIs
/// use for entrances — the deceleration is what makes it read as physical
/// rather than mechanical.
pub fn ease_out_cubic(t: i32) -> i32 {
    let t = t.clamp(0, ONE);
    // 1 - (1-t)^3, all in Q16.
    let inv = (ONE - t) as i64;
    let cube = inv * inv / ONE as i64 * inv / ONE as i64;
    (ONE as i64 - cube) as i32
}

/// Cubic ease-in-out: soft start and soft landing (game-menu feel).
pub fn ease_in_out_cubic(t: i32) -> i32 {
    let t = t.clamp(0, ONE);
    if t < ONE / 2 {
        // 4 * t^3
        let t64 = t as i64;
        let t2 = t64 * t64 / ONE as i64;
        let t3 = t2 * t64 / ONE as i64;
        (4 * t3).clamp(0, ONE as i64) as i32
    } else {
        // 1 - (-2t + 2)^3 / 2
        let u = (2 * (ONE as i64 - t as i64)).clamp(0, 2 * ONE as i64);
        let u2 = u * u / ONE as i64;
        let u3 = u2 * u / ONE as i64;
        (ONE as i64 - u3 / 2).clamp(0, ONE as i64) as i32
    }
}

/// Soft 0..=ONE breath for ambient chrome. Advances once per 60 Hz frame.
///
/// Full cycle ~3 seconds — slow enough to feel chill, not twitchy.
pub fn breath(phase: u32) -> i32 {
    const PERIOD: u32 = 180;
    let p = phase % PERIOD;
    let half = PERIOD / 2;
    let rising = if p < half {
        (p as i64 * ONE as i64) / half as i64
    } else {
        ((PERIOD - p) as i64 * ONE as i64) / half as i64
    } as i32;
    ease_in_out_cubic(rising)
}

/// Interpolate `a`..`b` by Q16 `t`.
pub fn lerp(a: i32, b: i32, t: i32) -> i32 {
    a + (((b - a) as i64 * t.clamp(0, ONE) as i64) / ONE as i64) as i32
}

/// Blend two XRGB colours by Q16 `t`.
pub fn lerp_color(a: u32, b: u32, t: i32) -> u32 {
    let ch = |sh: u32| -> u32 {
        let ca = ((a >> sh) & 0xff) as i32;
        let cb = ((b >> sh) & 0xff) as i32;
        (lerp(ca, cb, t).clamp(0, 255) as u32) << sh
    };
    ch(16) | ch(8) | ch(0)
}

/// Busy-wait until `us` microseconds after `since`, returning the new mark.
///
/// Capped so a stuck or unavailable counter cannot hang an animation.
pub fn pace(since: u64, us: u32) -> u64 {
    let hz = counter_hz();
    if hz == 0 {
        return since;
    }
    let target = since.wrapping_add(hz / 1_000_000 * us as u64);
    let mut guard: u64 = 0;
    while rdtsc() < target {
        guard += 1;
        if guard > 50_000_000 {
            break;
        }
        core::hint::spin_loop();
    }
    rdtsc()
}

/// A screen entrance: how far it slides and over how many frames.
pub struct Entrance {
    pub frames: u32,
    pub travel_px: i32,
    pub frame_us: u32,
}

/// Default entrance — paced at 60 Hz, soft travel, still under a quarter second.
pub const SLIDE_IN: Entrance = Entrance {
    frames: 12,
    travel_px: 22,
    frame_us: FRAME_US_60,
};

impl Entrance {
    /// Offset and opacity for frame `i`, as (dy px, alpha Q16).
    pub fn at(&self, i: u32) -> (i32, i32) {
        let denom = self.frames.max(1) as i64;
        let t = ((i.min(self.frames) as i64 * ONE as i64) / denom) as i32;
        let e = ease_out_cubic(t);
        let dy = lerp(self.travel_px, 0, e);
        (dy, e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_hits_both_ends_exactly() {
        assert_eq!(ease_out_cubic(0), 0);
        assert_eq!(ease_out_cubic(ONE), ONE);
    }

    #[test]
    fn ease_is_monotonic() {
        let mut prev = -1;
        for i in 0..=32 {
            let v = ease_out_cubic(i * ONE / 32);
            assert!(v >= prev, "eased curve went backwards at {i}");
            prev = v;
        }
    }

    #[test]
    fn ease_decelerates() {
        // The defining property: more distance covered early than late.
        let first = ease_out_cubic(ONE / 4);
        let last = ONE - ease_out_cubic(3 * ONE / 4);
        assert!(first > last, "curve is not an ease-OUT: {first} vs {last}");
    }

    #[test]
    fn ease_clamps_out_of_range_input() {
        assert_eq!(ease_out_cubic(-ONE), 0);
        assert_eq!(ease_out_cubic(3 * ONE), ONE);
    }

    #[test]
    fn ease_never_overshoots() {
        for i in 0..=64 {
            let v = ease_out_cubic(i * ONE / 64);
            assert!((0..=ONE).contains(&v), "overshoot at {i}: {v}");
        }
    }

    #[test]
    fn lerp_endpoints_and_midpoint() {
        assert_eq!(lerp(10, 20, 0), 10);
        assert_eq!(lerp(10, 20, ONE), 20);
        assert_eq!(lerp(0, 100, ONE / 2), 50);
    }

    #[test]
    fn colour_blend_stays_in_gamut() {
        let c = lerp_color(0x00FF_FFFF, 0x0000_0000, ONE / 2);
        for sh in [16, 8, 0] {
            assert!((c >> sh) & 0xff <= 0xff);
        }
        assert_eq!(lerp_color(0x00FF_FFFF, 0x001D_1D1F, ONE), 0x001D_1D1F);
    }

    #[test]
    fn entrance_starts_offset_and_lands_flush() {
        let (dy0, a0) = SLIDE_IN.at(0);
        assert_eq!(dy0, SLIDE_IN.travel_px, "should start displaced");
        assert_eq!(a0, 0, "should start transparent");

        let (dy_end, a_end) = SLIDE_IN.at(SLIDE_IN.frames);
        assert_eq!(dy_end, 0, "must land exactly flush, not near it");
        assert_eq!(a_end, ONE, "must land fully opaque");
    }

    #[test]
    fn entrance_is_brief() {
        let ms = SLIDE_IN.frames * SLIDE_IN.frame_us / 1000;
        // Chill 60 Hz slide: soft, but still snappy enough for setup.
        assert!(
            ms <= 250,
            "entrance takes {ms}ms — too slow to feel responsive"
        );
        assert_eq!(
            SLIDE_IN.frame_us, FRAME_US_60,
            "entrances must pace at 60 Hz"
        );
    }

    #[test]
    fn ease_in_out_is_soft_at_both_ends() {
        let early = ease_in_out_cubic(ONE / 8);
        let late = ONE - ease_in_out_cubic(7 * ONE / 8);
        assert!(early < ONE / 4, "should ease in: {early}");
        assert!(late < ONE / 4, "should ease out: {late}");
        assert_eq!(ease_in_out_cubic(0), 0);
        assert_eq!(ease_in_out_cubic(ONE), ONE);
    }

    #[test]
    fn breath_is_periodic_and_bounded() {
        for i in 0..360 {
            let v = breath(i);
            assert!((0..=ONE).contains(&v), "breath out of range at {i}: {v}");
        }
        assert_eq!(breath(0), breath(180));
        assert!(breath(90) > breath(0), "mid-breath should rise");
    }

    #[test]
    fn entrance_never_moves_backwards() {
        let mut prev = i32::MAX;
        for i in 0..=SLIDE_IN.frames {
            let (dy, _) = SLIDE_IN.at(i);
            assert!(dy <= prev, "slid backwards at frame {i}");
            prev = dy;
        }
    }

    #[test]
    fn lerp_reversed_range() {
        assert_eq!(lerp(100, 0, 0), 100);
        assert_eq!(lerp(100, 0, ONE), 0);
        assert_eq!(lerp(100, 0, ONE / 2), 50);
    }

    #[test]
    fn lerp_color_black_and_white() {
        assert_eq!(lerp_color(0x00000000, 0x00FFFFFF, 0), 0x00000000);
        assert_eq!(lerp_color(0x00000000, 0x00FFFFFF, ONE), 0x00FFFFFF);
        assert_eq!(lerp_color(0x00000000, 0x00FFFFFF, ONE / 2), 0x007F7F7F);
    }
}

