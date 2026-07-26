//! Motion — easing curves and frame pacing.
//!
//! Screens used to snap between states. macOS reads as smooth because things
//! *move* and because the motion decelerates: a linear slide looks mechanical,
//! an ease-out looks physical.
//!
//! Everything is fixed point. The kernel never enables the FPU, so a cubic
//! curve is evaluated in Q16 integers rather than floats.

use crate::serial::{rdtsc, ASSUMED_HZ};

/// Fixed-point one.
pub const ONE: i32 = 1 << 16;

/// Cubic ease-out: fast departure, gentle arrival.
///
/// `t` and the result are Q16 in 0..=ONE. This is the curve most system UIs
/// use for entrances — the deceleration is what makes it read as physical
/// rather than mechanical.
fn ease_out_cubic(t: i32) -> i32 {
    let t = t.clamp(0, ONE);
    // 1 - (1-t)^3, all in Q16.
    let inv = (ONE - t) as i64;
    let cube = inv * inv / ONE as i64 * inv / ONE as i64;
    (ONE as i64 - cube) as i32
}

/// Interpolate `a`..`b` by Q16 `t`.
fn lerp(a: i32, b: i32, t: i32) -> i32 {
    a + (((b - a) as i64 * t.clamp(0, ONE) as i64) / ONE as i64) as i32
}

/// Busy-wait until `us` microseconds after `since`, returning the new mark.
///
/// Capped so a stuck or unavailable counter cannot hang an animation.
pub fn pace(since: u64, us: u32) -> u64 {
    let target = since.wrapping_add(ASSUMED_HZ / 1_000_000 * us as u64);
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

/// Default entrance. Short and small — the point is to soften the cut, not to
/// make the user wait for the interface.
pub const SLIDE_IN: Entrance = Entrance {
    frames: 10,
    travel_px: 18,
    frame_us: 12_000,
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
        assert!(ms <= 200, "entrance takes {ms}ms — too slow to feel responsive");
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
}
