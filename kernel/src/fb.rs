//! Framebuffer drawing primitives (32-bit XRGB).

/// Classic public-domain 8×8 glyphs for ASCII 0x20..=0x7E (index = ch - 0x20).
const FONT: [[u8; 8]; 95] = include!("font8x8_basic.in");

fn glyph(ch: u8) -> [u8; 8] {
    if (0x20..=0x7E).contains(&ch) {
        FONT[(ch - 0x20) as usize]
    } else {
        FONT[(b'?' - 0x20) as usize]
    }
}

/// Live framebuffer surface.
pub struct Surface {
    addr: *mut u8,
    width: usize,
    height: usize,
    pitch: usize,
}

impl Surface {
    /// # Safety
    /// `addr` must be a valid writable framebuffer for the given geometry.
    pub unsafe fn new(addr: *mut u8, width: u64, height: u64, pitch: u64, bpp: u16) -> Option<Self> {
        if bpp != 32 || width == 0 || height == 0 || pitch < width * 4 {
            return None;
        }
        Some(Self {
            addr,
            width: width as usize,
            height: height as usize,
            pitch: pitch as usize,
        })
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
        let x1 = ((x + w) as usize).min(self.width);
        let y1 = ((y + h) as usize).min(self.height);
        for py in y0..y1 {
            for px in x0..x1 {
                unsafe { self.pixel(px, py).write_volatile(color) };
            }
        }
    }

    /// Soft rounded rectangle (pill-friendly).
    pub fn fill_round_rect(&self, x: i32, y: i32, w: i32, h: i32, radius: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let r = radius.max(0).min(w / 2).min(h / 2);
        let x0 = x;
        let y0 = y;
        let x1 = x + w - 1;
        let y1 = y + h - 1;

        // Center slab
        self.fill_rect(x0 + r, y0, w - 2 * r, h, color);
        // Side slabs
        self.fill_rect(x0, y0 + r, r, h - 2 * r, color);
        self.fill_rect(x1 - r + 1, y0 + r, r, h - 2 * r, color);

        // Corners
        let corners = [
            (x0 + r, y0 + r),
            (x1 - r, y0 + r),
            (x0 + r, y1 - r),
            (x1 - r, y1 - r),
        ];
        for (cx, cy) in corners {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        let px = cx + dx;
                        let py = cy + dy;
                        if px >= 0 && py >= 0 && (px as usize) < self.width && (py as usize) < self.height
                        {
                            unsafe { self.pixel(px as usize, py as usize).write_volatile(color) };
                        }
                    }
                }
            }
        }
    }

    /// Measure text width in pixels for a given scale.
    pub fn text_width(text: &str, scale: usize) -> i32 {
        (text.chars().count() * 8 * scale) as i32
    }

    pub fn text_height(scale: usize) -> i32 {
        (8 * scale) as i32
    }

    pub fn draw_text(&self, mut x: i32, y: i32, text: &str, scale: usize, color: u32) {
        let scale = scale.max(1);
        for ch in text.bytes() {
            if ch == b'\n' {
                continue;
            }
            let g = glyph(ch);
            for (row_i, bits) in g.iter().enumerate() {
                for col in 0..8 {
                    if bits & (1 << col) != 0 {
                        let px = x + (col * scale) as i32;
                        let py = y + (row_i * scale) as i32;
                        self.fill_rect(px, py, scale as i32, scale as i32, color);
                    }
                }
            }
            x += (8 * scale) as i32;
        }
    }

    pub fn draw_text_centered(&self, cx: i32, y: i32, text: &str, scale: usize, color: u32) {
        let w = Self::text_width(text, scale);
        self.draw_text(cx - w / 2, y, text, scale, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_space_blank() {
        assert_eq!(glyph(b' '), [0u8; 8]);
    }

    #[test]
    fn text_width_scales() {
        assert_eq!(Surface::text_width("os", 1), 16);
        assert_eq!(Surface::text_width("os", 4), 64);
    }
}
