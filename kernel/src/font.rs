//! Anti-aliased proportional UI type.
//!
//! The atlas is rasterized at build time from a real outline font (SF Pro when
//! present) — see `build.rs`. The kernel only blends 8-bit coverage, so there is
//! no rasterizer, no allocator and no float transcendentals here.

/// One glyph's placement and its slice of the face's alpha bitmap.
pub struct Glyph {
    /// Bitmap width in px (0 for blanks like space).
    pub w: usize,
    /// Bitmap height in px.
    pub h: usize,
    /// Left side bearing, relative to the pen.
    pub bx: i32,
    /// Top bearing, relative to the *baseline* (negative = above it).
    pub by: i32,
    /// Horizontal advance in 1/64 px, so fractional widths don't drift.
    pub adv64: i32,
    /// Byte offset into the face bitmap.
    pub off: usize,
}

/// A single size+weight cut.
pub struct Face {
    pub glyphs: &'static [Glyph],
    pub bitmap: &'static [u8],
    pub ascent: i32,
    pub descent: i32,
    /// Default line height (ascent - descent + gap).
    pub line: i32,
    pub px: i32,
}

/// Codepoint range covered by every face.
const FIRST: u8 = 0x20;
const LAST: u8 = 0x7E;

include!(concat!(env!("OUT_DIR"), "/font_atlas.rs"));

impl Face {
    /// Glyph for an ASCII byte; anything outside the atlas renders as `?`.
    pub fn glyph_for(&self, ch: u8) -> &Glyph {
        let idx = if (FIRST..=LAST).contains(&ch) {
            (ch - FIRST) as usize
        } else {
            (b'?' - FIRST) as usize
        };
        &self.glyphs[idx]
    }

    /// Width of `text` in 1/64 px, including `tracking64` between glyphs.
    ///
    /// Tracking is applied *between* glyphs only — a trailing gap would make
    /// centred text sit visibly left of true centre at negative tracking.
    pub fn width64(&self, text: &str, tracking64: i32) -> i32 {
        let mut total = 0;
        let mut n = 0;
        for ch in text.bytes() {
            total += self.glyph_for(ch).adv64;
            n += 1;
        }
        if n > 1 {
            total += tracking64 * (n - 1);
        }
        total
    }

    /// Width of `text` in whole px.
    pub fn width(&self, text: &str, tracking64: i32) -> i32 {
        (self.width64(text, tracking64) + 32) >> 6
    }

    /// Distance from the top of a line box to the baseline.
    pub fn baseline(&self) -> i32 {
        self.ascent
    }
}

/// Letter-spacing helper: `pct` of the em, in 1/64 px.
///
/// The reference design uses -3% on display type and -1.5% on section heads.
pub const fn tracking_pct(px: i32, pct_tenths: i32) -> i32 {
    // px * 64 * (pct_tenths / 1000)
    (px * 64 * pct_tenths) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_present() {
        // Deliberately not a fixed count — adding a cut shouldn't fail a test.
        assert!(!FACES.is_empty());
        for f in FACES {
            assert!(f.px > 0);
            assert_eq!(f.glyphs.len(), (LAST - FIRST + 1) as usize);
        }
    }

    #[test]
    fn hero_is_largest() {
        assert!(HERO_FACE.px > BODY_FACE.px);
        assert!(BODY_FACE.px > SMALL_FACE.px);
    }

    #[test]
    fn space_is_blank_but_advances() {
        let g = BODY_FACE.glyph_for(b' ');
        assert_eq!(g.w, 0, "space should have no bitmap");
        assert!(g.adv64 > 0, "space must still advance the pen");
    }

    #[test]
    fn glyphs_have_coverage() {
        // A solid letter must actually carry ink.
        let g = HERO_FACE.glyph_for(b'H');
        assert!(g.w > 0 && g.h > 0);
        let ink: u32 = HERO_FACE.bitmap[g.off..g.off + g.w * g.h]
            .iter()
            .map(|&a| a as u32)
            .sum();
        assert!(ink > 0, "glyph bitmap is empty");
    }

    #[test]
    fn antialiased_not_bilevel() {
        // The whole point of the atlas: partial coverage must exist.
        let g = HERO_FACE.glyph_for(b'o');
        let mid = HERO_FACE.bitmap[g.off..g.off + g.w * g.h]
            .iter()
            .filter(|&&a| a > 8 && a < 247)
            .count();
        assert!(mid > 0, "no partial coverage — atlas is 1-bit, not AA");
    }

    #[test]
    fn tracking_is_negative_for_display() {
        assert!(tracking_pct(44, -30) < 0);
    }

    #[test]
    fn width_excludes_trailing_tracking() {
        let one = BODY_FACE.width64("A", -64);
        assert_eq!(one, BODY_FACE.glyph_for(b'A').adv64, "single glyph gets no tracking");
    }

    #[test]
    fn width_grows_with_text() {
        assert!(BODY_FACE.width("Hello world", 0) > BODY_FACE.width("Hello", 0));
    }
}
