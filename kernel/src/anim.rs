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
const ONE: i32 = 1 << 16;

/// Default entrance. Short and small — soften the cut, don't make the user wait.
const SLIDE_FRAMES: u32 = 10;
const SLIDE_TRAVEL_PX: i32 = 18;
const SLIDE_FRAME_US: u32 = 12_000;

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
pub(crate) fn pace(since: u64, us: u32) -> u64 {
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

/// Offset and opacity for entrance frame `i`, as (dy px, alpha Q16).
fn slide_at(i: u32) -> (i32, i32) {
    let denom = SLIDE_FRAMES as i64;
    let t = ((i.min(SLIDE_FRAMES) as i64 * ONE as i64) / denom) as i32;
    let e = ease_out_cubic(t);
    let dy = lerp(SLIDE_TRAVEL_PX, 0, e);
    (dy, e)
}

/// Play the default screen entrance, calling `frame(dy, alpha)` then pacing.
pub fn slide_in(mut frame: impl FnMut(i32, i32)) {
    let mut mark = rdtsc();
    for i in 0..=SLIDE_FRAMES {
        let (dy, a) = slide_at(i);
        frame(dy, a);
        mark = pace(mark, SLIDE_FRAME_US);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_is_monotonic() {
        let mut prev = -1;
        for i in 0..=64 {
            let v = ease_out_cubic(i * ONE / 64);
            assert!((0..=ONE).contains(&v), "overshoot at {i}: {v}");
            assert!(v >= prev, "eased curve went backwards at {i}");
            prev = v;
        }
        assert_eq!(ease_out_cubic(-ONE), 0);
        assert_eq!(ease_out_cubic(3 * ONE), ONE);
    }

    #[test]
    fn ease_decelerates() {
        // The defining property: more distance covered early than late.
        let first = ease_out_cubic(ONE / 4);
        let last = ONE - ease_out_cubic(3 * ONE / 4);
        assert!(first > last, "curve is not an ease-OUT: {first} vs {last}");
    }

    #[test]
    fn lerp_endpoints_and_midpoint() {
        assert_eq!(lerp(10, 20, 0), 10);
        assert_eq!(lerp(10, 20, ONE), 20);
        assert_eq!(lerp(0, 100, ONE / 2), 50);
    }

    #[test]
    fn entrance_starts_offset_and_lands_flush() {
        let (dy0, a0) = slide_at(0);
        assert_eq!(dy0, SLIDE_TRAVEL_PX, "should start displaced");
        assert_eq!(a0, 0, "should start transparent");

        let (dy_end, a_end) = slide_at(SLIDE_FRAMES);
        assert_eq!(dy_end, 0, "must land exactly flush, not near it");
        assert_eq!(a_end, ONE, "must land fully opaque");
    }

    #[test]
    fn entrance_never_moves_backwards() {
        let mut prev = i32::MAX;
        for i in 0..=SLIDE_FRAMES {
            let (dy, _) = slide_at(i);
            assert!(dy <= prev, "slid backwards at frame {i}");
            prev = dy;
        }
    }
}
