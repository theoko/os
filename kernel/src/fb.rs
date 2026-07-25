//! Framebuffer drawing primitives (32-bit XRGB).
//!
//! Everything user-visible is anti-aliased: glyphs blend 8-bit coverage from
//! the build-time atlas (`font.rs`), and shapes are supersampled 4x4. Coverage
//! is computed with integer math only — the kernel never enables the FPU, so
//! float/SSE paths would be a #UD waiting to happen.

use crate::font::Face;

/// Live framebuffer surface.
pub struct Surface {
    addr: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
}

/// Blend `src` over `dst` by `a` (0..=255).
#[inline]
fn blend(dst: u32, src: u32, a: u32) -> u32 {
    if a == 0 {
        return dst;
    }
    if a >= 255 {
        return src;
    }
    let ia = 255 - a;
    let r = (((src >> 16) & 0xff) * a + ((dst >> 16) & 0xff) * ia + 127) / 255;
    let g = (((src >> 8) & 0xff) * a + ((dst >> 8) & 0xff) * ia + 127) / 255;
    let b = ((src & 0xff) * a + (dst & 0xff) * ia + 127) / 255;
    (r << 16) | (g << 8) | b
}

impl Surface {
    /// # Safety
    /// `addr` must be a valid writable framebuffer for the given geometry.
    ///
    /// All drawing here composes pixels as XRGB (red at bit 16, green at 8,
    /// blue at 0). `mask_shifts` = Limine's `(red, green, blue)` mask shifts;
    /// a framebuffer with any other channel order is rejected rather than
    /// silently rendering with swapped colours.
    pub unsafe fn new(
        addr: *mut u8,
        width: u64,
        height: u64,
        pitch: u64,
        bpp: u16,
        mask_shifts: (u8, u8, u8),
    ) -> Option<Self> {
        if bpp != 32 || width == 0 || height == 0 || pitch < width * 4 {
            return None;
        }
        if mask_shifts != (16, 8, 0) {
            return None;
        }
        Some(Self {
            addr,
            width: width as usize,
            height: height as usize,
            pitch: pitch as usize,
        })
    }

    /// A surface over caller-owned RAM, for off-screen rasterisation.
    ///
    /// # Safety
    /// `addr` must point to at least `width * height` u32s.
    pub unsafe fn in_memory(addr: *mut u32, width: usize, height: usize) -> Self {
        Self { addr: addr.cast::<u8>(), width, height, pitch: width * 4 }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get_pixel(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || (x as usize) >= self.width || (y as usize) >= self.height {
            return 0;
        }
        unsafe { self.pixel(x as usize, y as usize).read_volatile() }
    }

    pub fn put_pixel(&self, x: i32, y: i32, color: u32) {
        if x < 0 || y < 0 || (x as usize) >= self.width || (y as usize) >= self.height {
            return;
        }
        unsafe { self.pixel(x as usize, y as usize).write_volatile(color) };
    }

    /// Blend `color` at `a` (0..=255) over whatever is already there.
    #[inline]
    pub fn blend_pixel(&self, x: i32, y: i32, color: u32, a: u32) {
        if a == 0 || x < 0 || y < 0 || (x as usize) >= self.width || (y as usize) >= self.height {
            return;
        }
        let p = unsafe { self.pixel(x as usize, y as usize) };
        let dst = unsafe { p.read_volatile() };
        unsafe { p.write_volatile(blend(dst, color, a)) };
    }

    #[inline]
    unsafe fn pixel(&self, x: usize, y: usize) -> *mut u32 {
        unsafe { self.addr.add(y * self.pitch + x * 4).cast::<u32>() }
    }

    pub fn fill(&self, color: u32) {
        for y in 0..self.height {
            for x in 0..self.width {
                unsafe { self.pixel(x, y).write_volatile(color) };
            }
        }
    }

    pub fn fill_rect(&self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w).max(0) as usize).min(self.width);
        let y1 = ((y + h).max(0) as usize).min(self.height);
        for py in y0..y1 {
            for px in x0..x1 {
                unsafe { self.pixel(px, py).write_volatile(color) };
            }
        }
    }

    /// Anti-aliased rounded rectangle. `radius >= h/2` gives a pill.
    ///
    /// Coverage comes from a 4x4 integer supersample in 1/8-px units, so there
    /// are no floats and no `sqrt` — corners test squared distance directly.
    pub fn fill_round_rect(&self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.max(0).min(w / 2).min(h / 2);
        if r == 0 {
            self.fill_rect(x, y, w, h, color);
            return;
        }

        // Corner centres and radius, in 1/8-px units.
        let r8 = r * 8;
        let rr = r8 * r8;
        let (l8, t8) = (x * 8, y * 8);
        let (rt8, b8) = ((x + w) * 8, (y + h) * 8);
        let cx_l = l8 + r8;
        let cx_r = rt8 - r8;
        let cy_t = t8 + r8;
        let cy_b = b8 - r8;

        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.width as i32);
        let y1 = (y + h).min(self.height as i32);

        for py in y0..y1 {
            for px in x0..x1 {
                let mut hits = 0u32;
                for sy in 0..4 {
                    // Sample centres at 1/8, 3/8, 5/8, 7/8 of the pixel.
                    let s_y = py * 8 + sy * 2 + 1;
                    for sx in 0..4 {
                        let s_x = px * 8 + sx * 2 + 1;
                        if s_x < l8 || s_x >= rt8 || s_y < t8 || s_y >= b8 {
                            continue;
                        }
                        // Only the four corner squares need a radius test.
                        let cx = if s_x < cx_l {
                            cx_l
                        } else if s_x > cx_r {
                            cx_r
                        } else {
                            s_x
                        };
                        let cy = if s_y < cy_t {
                            cy_t
                        } else if s_y > cy_b {
                            cy_b
                        } else {
                            s_y
                        };
                        let dx = s_x - cx;
                        let dy = s_y - cy;
                        if dx * dx + dy * dy <= rr {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    self.blend_pixel(px, py, color, hits * 255 / 16);
                }
            }
        }
    }

    /// Anti-aliased convex/concave polygon fill (even-odd rule).
    ///
    /// Points are in 1/8-px units so callers can place sub-pixel vertices.
    /// Same 4x4 integer supersample as `fill_round_rect` — no floats.
    pub fn fill_polygon(&self, pts8: &[(i32, i32)], color: u32) {
        if pts8.len() < 3 {
            return;
        }
        let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
        let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
        for &(px, py) in pts8 {
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
        }
        let x0 = (min_x >> 3).max(0);
        let y0 = (min_y >> 3).max(0);
        let x1 = ((max_x >> 3) + 1).min(self.width as i32);
        let y1 = ((max_y >> 3) + 1).min(self.height as i32);

        for py in y0..y1 {
            for px in x0..x1 {
                let mut hits = 0u32;
                for sy in 0..4 {
                    let s_y = py * 8 + sy * 2 + 1;
                    for sx in 0..4 {
                        let s_x = px * 8 + sx * 2 + 1;
                        // Even-odd crossing test against every edge.
                        let mut inside = false;
                        let mut j = pts8.len() - 1;
                        for i in 0..pts8.len() {
                            let (xi, yi) = pts8[i];
                            let (xj, yj) = pts8[j];
                            if (yi > s_y) != (yj > s_y) {
                                // Compare against the edge's x at s_y without
                                // dividing: cross-multiply, flipping for sign.
                                let dy = yj - yi;
                                let t = (s_y - yi) as i64 * (xj - xi) as i64;
                                let lhs = t + (xi as i64) * dy as i64;
                                let rhs = (s_x as i64) * dy as i64;
                                if (dy > 0 && lhs > rhs) || (dy < 0 && lhs < rhs) {
                                    inside = !inside;
                                }
                            }
                            j = i;
                        }
                        if inside {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    self.blend_pixel(px, py, color, hits * 255 / 16);
                }
            }
        }
    }

    /// Draw `text` with its baseline at `baseline_y`, pen starting at `x`.
    ///
    /// `tracking64` is inter-glyph spacing in 1/64 px (negative tightens).
    pub fn draw_text(
        &self,
        x: i32,
        baseline_y: i32,
        text: &str,
        face: &Face,
        tracking64: i32,
        color: u32,
    ) {
        // Pen runs in 1/64 px so fractional advances don't accumulate error.
        let mut pen64 = x * 64;
        let mut first = true;
        for ch in text.bytes() {
            if !first {
                pen64 += tracking64;
            }
            first = false;
            let g = face.glyph_for(ch);
            let pen_px = (pen64 + 32) >> 6;
            if g.w > 0 && g.h > 0 {
                let gx = pen_px + g.bx;
                let gy = baseline_y + g.by;
                for row in 0..g.h {
                    let src = g.off + row * g.w;
                    for col in 0..g.w {
                        let a = face.bitmap[src + col] as u32;
                        if a != 0 {
                            self.blend_pixel(gx + col as i32, gy + row as i32, color, a);
                        }
                    }
                }
            }
            pen64 += g.adv64;
        }
    }

    /// Draw `text` horizontally centred on `cx`.
    pub fn draw_text_centered(
        &self,
        cx: i32,
        baseline_y: i32,
        text: &str,
        face: &Face,
        tracking64: i32,
        color: u32,
    ) {
        let w = face.width(text, tracking64);
        self.draw_text(cx - w / 2, baseline_y, text, face, tracking64, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{BODY_FACE, HERO_FACE};

    /// Off-screen surface so drawing can be asserted on the host.
    struct Canvas {
        buf: Vec<u32>,
        w: usize,
        h: usize,
    }

    impl Canvas {
        fn new(w: usize, h: usize) -> Self {
            Self { buf: vec![0x00FF_FFFF; w * h], w, h }
        }
        fn surface(&mut self) -> Surface {
            Surface {
                addr: self.buf.as_mut_ptr().cast::<u8>(),
                width: self.w,
                height: self.h,
                pitch: self.w * 4,
            }
        }
        fn get(&self, x: usize, y: usize) -> u32 {
            self.buf[y * self.w + x]
        }
        fn ink_count(&self) -> usize {
            self.buf.iter().filter(|&&p| p != 0x00FF_FFFF).count()
        }
    }

    #[test]
    fn blend_endpoints_are_exact() {
        assert_eq!(blend(0x00FF_FFFF, 0x0000_0000, 0), 0x00FF_FFFF);
        assert_eq!(blend(0x00FF_FFFF, 0x0000_0000, 255), 0x0000_0000);
    }

    #[test]
    fn blend_midpoint_is_grey() {
        let m = blend(0x00FF_FFFF, 0x0000_0000, 128);
        assert_eq!(m & 0xff, (m >> 8) & 0xff, "channels should stay neutral");
        assert!((0x76..=0x80).contains(&(m & 0xff)), "got {:06X}", m);
    }

    #[test]
    fn text_puts_ink_on_canvas() {
        let mut c = Canvas::new(300, 80);
        {
            let s = c.surface();
            s.draw_text(10, 50, "Hello", &BODY_FACE, 0, 0x0000_0000);
        }
        assert!(c.ink_count() > 40, "expected glyph ink, got {}", c.ink_count());
    }

    #[test]
    fn text_is_antialiased_on_canvas() {
        let mut c = Canvas::new(400, 120);
        {
            let s = c.surface();
            s.draw_text(10, 80, "oa", &HERO_FACE, 0, 0x0000_0000);
        }
        // Genuine AA means greys between the ink and the page.
        let greys = c
            .buf
            .iter()
            .filter(|&&p| p != 0x00FF_FFFF && p != 0x0000_0000)
            .count();
        assert!(greys > 20, "expected AA edge pixels, got {greys}");
    }

    #[test]
    fn centred_text_is_actually_centred() {
        let mut c = Canvas::new(400, 60);
        {
            let s = c.surface();
            s.draw_text_centered(200, 40, "Ready", &BODY_FACE, 0, 0x0000_0000);
        }
        let (mut lo, mut hi) = (usize::MAX, 0);
        for y in 0..c.h {
            for x in 0..c.w {
                if c.get(x, y) != 0x00FF_FFFF {
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
            }
        }
        let mid = (lo + hi) / 2;
        assert!(mid.abs_diff(200) <= 3, "text centre {mid} drifted from 200");
    }

    #[test]
    fn negative_tracking_tightens() {
        let tight = BODY_FACE.width("Agents with", -64);
        let loose = BODY_FACE.width("Agents with", 0);
        assert!(tight < loose, "negative tracking must narrow the run");
    }

    #[test]
    fn round_rect_corners_are_soft() {
        let mut c = Canvas::new(120, 60);
        {
            let s = c.surface();
            s.fill_round_rect(10, 10, 100, 40, 20, 0x0000_71E3);
        }
        // Dead centre is solid; the extreme corner is untouched page.
        assert_eq!(c.get(60, 30), 0x0000_71E3);
        assert_eq!(c.get(10, 10), 0x00FF_FFFF, "square corner — no rounding");
        // And somewhere on the arc there must be a partial blend.
        let partial = c
            .buf
            .iter()
            .filter(|&&p| p != 0x00FF_FFFF && p != 0x0000_71E3)
            .count();
        assert!(partial > 10, "corner arc is not anti-aliased ({partial})");
    }

    #[test]
    fn zero_radius_is_a_plain_rect() {
        let mut c = Canvas::new(40, 40);
        {
            let s = c.surface();
            s.fill_round_rect(5, 5, 10, 10, 0, 0x0000_0000);
        }
        assert_eq!(c.get(5, 5), 0x0000_0000);
        assert_eq!(c.ink_count(), 100);
    }

    #[test]
    fn drawing_clips_to_surface() {
        let mut c = Canvas::new(40, 40);
        {
            let s = c.surface();
            // Straddling every edge must not panic or corrupt memory.
            s.fill_round_rect(-20, -20, 30, 30, 8, 0x0000_0000);
            s.fill_round_rect(30, 30, 40, 40, 8, 0x0000_0000);
            s.draw_text(-50, 10, "clipped", &BODY_FACE, 0, 0x0000_0000);
            s.draw_text(38, 20, "clipped", &BODY_FACE, 0, 0x0000_0000);
        }
        assert_eq!(c.buf.len(), 40 * 40);
    }
}

/// Largest framebuffer we can double-buffer. Lives in `.bss`, so it costs
/// nothing in the ISO — Limine zeroes it at load.
pub const MAX_W: usize = 1920;
pub const MAX_H: usize = 1200;

static mut BACK: [u32; MAX_W * MAX_H] = [0; MAX_W * MAX_H];

/// Back buffer plus the framebuffer it presents to.
///
/// Drawing straight into video memory is why repaints flashed and crawled: a
/// full-screen `fill` is ~786k *uncached* MMIO writes, and every anti-aliased
/// pixel costs an MMIO read-modify-write on top. Compositing in cached RAM and
/// blitting once removes the flash and makes blending roughly free.
pub struct Screen {
    back: Surface,
    fb: *mut u8,
    fb_pitch: usize,
    w: usize,
    h: usize,
    /// False when the mode is bigger than the back buffer and we draw straight
    /// into video memory instead. Flickers, but a large display must still boot.
    buffered: bool,
}

impl Screen {
    /// # Safety
    /// Same contract as [`Surface::new`].
    ///
    /// Returns `None` when the mode is unsupported *or* larger than the back
    /// buffer; callers should fall back to drawing directly.
    pub unsafe fn new(
        addr: *mut u8,
        width: u64,
        height: u64,
        pitch: u64,
        bpp: u16,
        mask_shifts: (u8, u8, u8),
    ) -> Option<Self> {
        if bpp != 32 || width == 0 || height == 0 || pitch < width * 4 {
            return None;
        }
        if mask_shifts != (16, 8, 0) {
            return None;
        }
        let (w, h) = (width as usize, height as usize);
        // Too large to double-buffer: fall back to drawing directly rather
        // than refusing the mode, which would leave the machine with no UI.
        let buffered = w <= MAX_W && h <= MAX_H;
        let back = if buffered {
            Surface {
                addr: (&raw mut BACK).cast::<u8>(),
                width: w,
                height: h,
                pitch: w * 4,
            }
        } else {
            Surface { addr, width: w, height: h, pitch: pitch as usize }
        };
        Some(Self { back, fb: addr, fb_pitch: pitch as usize, w, h, buffered })
    }

    /// The surface to draw on. Nothing is visible until [`Self::present`].
    pub fn surface(&self) -> &Surface {
        &self.back
    }

    /// Whether composition is off-screen. False means direct-to-video fallback.
    pub fn is_buffered(&self) -> bool {
        self.buffered
    }

    /// Blit one region. Used for cursor motion — blitting the whole screen
    /// per mouse event would make tracking crawl over uncached MMIO.
    pub fn present_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        if !self.buffered {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w).max(0) as usize).min(self.w);
        let y1 = ((y + h).max(0) as usize).min(self.h);
        for py in y0..y1 {
            let src = unsafe { self.back.addr.add(py * self.back.pitch).cast::<u32>() };
            let dst = unsafe { self.fb.add(py * self.fb_pitch).cast::<u32>() };
            for px in x0..x1 {
                unsafe { dst.add(px).write_volatile(src.add(px).read()) };
            }
        }
    }

    /// Blit the whole back buffer to the framebuffer.
    pub fn present(&self) {
        if !self.buffered {
            return;
        }
        for y in 0..self.h {
            let src = unsafe { self.back.addr.add(y * self.back.pitch).cast::<u32>() };
            let dst = unsafe { self.fb.add(y * self.fb_pitch).cast::<u32>() };
            for x in 0..self.w {
                unsafe { dst.add(x).write_volatile(src.add(x).read()) };
            }
        }
    }
}
