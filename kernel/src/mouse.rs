//! PS/2 mouse (relative) + software cursor for the framebuffer UI.
//!
//! UTM often sets `QEMU.PS2Controller = false` and hides the host pointer when
//! the guest window is focused — without a large guest-drawn cursor it looks
//! mouseless. We always paint a chunky arrow; PS/2 moves it when the controller
//! is enabled (`make utm` turns PS/2 on).

use crate::fb::Surface;
use crate::port;

const DATA: u16 = 0x60;
/// i8042 status (read) and command (write) share this port.
const STATUS: u16 = 0x64;

/// Keep PS/2 probes short so a missing controller can't stall the UI.
const SPIN: u32 = 20_000;


fn wait_ibf_clear() -> bool {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        for _ in 0..SPIN {
            if port::inb(STATUS) & 0x02 == 0 {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

fn wait_obf_set(spins: u32) -> bool {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        for _ in 0..spins {
            if port::inb(STATUS) & 0x01 != 0 {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = spins;
        false
    }
}

fn write_i8042(port: u16, byte: u8) -> bool {
    if !wait_ibf_clear() {
        return false;
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outb(port, byte);
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = (port, byte);
    true
}

fn read_data(spins: u32) -> Option<u8> {
    if !wait_obf_set(spins) {
        return None;
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        Some(port::inb(DATA))
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = spins;
        None
    }
}

fn write_mouse(byte: u8) -> bool {
    write_i8042(STATUS, 0xD4) && write_i8042(DATA, byte)
}

fn mouse_expect_ack() -> bool {
    matches!(read_data(SPIN), Some(0xFA))
}

/// Relative mouse state (screen pixels).
pub struct Mouse {
    pub x: i32,
    pub y: i32,
    buttons: u8,
    screen_w: i32,
    screen_h: i32,
    prev_buttons: u8,
    packet: [u8; 3],
    packet_i: usize,
    /// The controller has produced at least one byte with the AUX flag set —
    /// from then on we can trust the flag and reject keyboard bytes.
    aux_seen: bool,
}

impl Mouse {
    pub fn new(screen_w: i32, screen_h: i32) -> Self {
        Self {
            x: screen_w / 2,
            y: screen_h / 2,
            buttons: 0,
            screen_w,
            screen_h,
            prev_buttons: 0,
            packet: [0; 3],
            packet_i: 0,
            aux_seen: false,
        }
    }

    /// Rising edge on the primary button. Call once per frame.
    pub fn take_click_edge(&mut self) -> bool {
        let edge = self.buttons & 1 != 0 && self.prev_buttons & 1 == 0;
        self.prev_buttons = self.buttons;
        edge
    }

    /// Absolute tablet report in 0..=32767 → screen pixels on this mouse.
    pub(crate) fn apply_abs(&mut self, ax: i32, ay: i32, buttons: u8) -> bool {
        let ax = ax.clamp(0, 32767);
        let ay = ay.clamp(0, 32767);
        let nx = ((ax * (self.screen_w - 1)) / 32767).clamp(0, self.screen_w.saturating_sub(1));
        let ny = ((ay * (self.screen_h - 1)) / 32767).clamp(0, self.screen_h.saturating_sub(1));
        let moved = nx != self.x || ny != self.y || buttons != self.buttons;
        self.x = nx;
        self.y = ny;
        self.buttons = buttons;
        moved
    }

    /// Best-effort PS/2 enable. Returns whether streaming was enabled.
    pub fn init(&mut self) -> bool {
        if !write_i8042(STATUS, 0xA8) {
            return false;
        }
        // Drain any stale output (boot-time keyboard/self-test bytes) so the
        // 0x20 reply below is really the command byte and not leftovers.
        for _ in 0..16 {
            if read_data(1).is_none() {
                break;
            }
        }
        if !write_i8042(STATUS, 0x20) {
            return false;
        }
        let mut status = match read_data(SPIN) {
            Some(s) => s,
            None => return false,
        };
        status |= 0x02;
        status &= !0x20;
        if !write_i8042(STATUS, 0x60) || !write_i8042(DATA, status) {
            return false;
        }
        if !write_mouse(0xF6) || !mouse_expect_ack() {
            return false;
        }
        if !write_mouse(0xF4) || !mouse_expect_ack() {
            return false;
        }
        true
    }

    /// Drain available bytes; update position (clamped to the screen).
    pub fn poll(&mut self) -> bool {
        let mut moved = false;
        #[cfg(target_arch = "x86_64")]
        {
            loop {
                let st = unsafe { port::inb(STATUS) };
                if st & 0x01 == 0 {
                    break;
                }
                let is_mouse = st & 0x20 != 0;
                let b = unsafe { port::inb(DATA) };
                if is_mouse {
                    self.aux_seen = true;
                }
                // Accept bytes even if the controller forgets the AUX flag
                // (common under TCG) — but once the flag has ever worked, trust
                // it so keyboard scancodes are not parsed as mouse packets.
                if !is_mouse && self.aux_seen {
                    self.packet_i = 0;
                    continue;
                }
                if self.packet_i == 0 && (b & 0x08) == 0 {
                    continue;
                }
                self.packet[self.packet_i] = b;
                self.packet_i += 1;
                if self.packet_i < 3 {
                    continue;
                }
                self.packet_i = 0;
                let flags = self.packet[0];
                // Overflow bits: the deltas are invalid — drop the packet.
                if flags & 0xC0 != 0 {
                    continue;
                }
                // 9-bit deltas: bits 4/5 of flags are the sign (bit 8) of
                // dx/dy. `as i8` alone misreads deltas outside -128..127.
                let dx = self.packet[1] as i32 - (((flags as i32) << 4) & 0x100);
                let dy = self.packet[2] as i32 - (((flags as i32) << 3) & 0x100);
                self.x = (self.x + dx).clamp(0, self.screen_w.saturating_sub(1));
                self.y = (self.y - dy).clamp(0, self.screen_h.saturating_sub(1));
                self.buttons = flags & 0x07;
                moved = true;
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {}
        moved
    }
}

/// Arrow outline in 1/8-px units, tip at (0,0) — a real polygon so the cursor
/// is anti-aliased like the rest of the UI instead of a stair-stepped bitmap.
const ARROW: [(i32, i32); 7] = [
    (0, 0),
    (0, 136),
    (34, 104),
    (56, 152),
    (76, 144),
    (55, 97),
    (92, 96),
];

/// Bounding box in whole px (from ARROW, plus 1px for the white keyline).
const DRAW_W: usize = 14;
const DRAW_H: usize = 21;
/// The keyline is also stamped one pixel LEFT and ABOVE the hotspot, so the
/// save/restore box extends 1px past the silhouette box on those sides —
/// otherwise moving the cursor leaves a white AA trail behind.
const SAVE_W: usize = DRAW_W + 1;
const SAVE_H: usize = DRAW_H + 1;
const SAVE_LEN: usize = SAVE_W * SAVE_H;

/// Saves under-cursor pixels so we can move without full redraws.
pub struct Cursor {
    x: i32,
    y: i32,
    saved: [u32; SAVE_LEN],
    has_saved: bool,
}

impl Cursor {
    pub const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            saved: [0; SAVE_LEN],
            has_saved: false,
        }
    }

    pub fn hide(&mut self, fb: &Surface) {
        if self.has_saved {
            restore(fb, self.x, self.y, &self.saved);
            self.has_saved = false;
        }
    }

    pub fn show_at(&mut self, fb: &Surface, x: i32, y: i32) {
        self.hide(fb);
        self.x = x;
        self.y = y;
        save(fb, x, y, &mut self.saved);
        self.has_saved = true;
        draw_arrow(fb, x, y);
    }
}

fn save(fb: &Surface, x: i32, y: i32, out: &mut [u32; SAVE_LEN]) {
    for row in 0..SAVE_H {
        for col in 0..SAVE_W {
            out[row * SAVE_W + col] = fb.get_pixel(x - 1 + col as i32, y - 1 + row as i32);
        }
    }
}

fn restore(fb: &Surface, x: i32, y: i32, saved: &[u32; SAVE_LEN]) {
    fb.mark_dirty(x, y, SAVE_W as i32, SAVE_H as i32);
    for row in 0..SAVE_H {
        for col in 0..SAVE_W {
            fb.put_pixel(x - 1 + col as i32, y - 1 + row as i32, saved[row * SAVE_W + col]);
        }
    }
}

/// Pre-rendered cursor coverage, built once.
///
/// Rasterising the arrow per mouse move meant five supersampled polygon fills
/// — four keyline stamps plus the body — or roughly 185k edge tests per event,
/// which is what made pointer motion lag. Bake the coverage once and the move
/// path becomes a few hundred alpha blends.
static mut MASK_INK: [u8; SAVE_LEN] = [0; SAVE_LEN];
static mut MASK_KEY: [u8; SAVE_LEN] = [0; SAVE_LEN];
static mut MASK_READY: bool = false;

/// Rasterise `ARROW` into the two coverage masks. Idempotent.
fn ensure_mask() {
    // SAFETY: single-threaded kernel; this runs before any cursor is drawn and
    // is a no-op thereafter.
    unsafe {
        if MASK_READY {
            return;
        }
        let mut scratch = [0u32; SAVE_LEN];

        // White-on-black gives us coverage directly in the low byte.
        let mut rasterise = |offsets: &[(i32, i32)], out: &mut [u8; SAVE_LEN]| {
            for px in scratch.iter_mut() {
                *px = 0;
            }
            let surf = Surface::in_memory(scratch.as_mut_ptr(), SAVE_W, SAVE_H);
            let mut pts = [(0i32, 0i32); ARROW.len()];
            for (dx, dy) in offsets {
                for (i, (ax, ay)) in ARROW.iter().enumerate() {
                    // +1px so the keyline's left/top stamps stay in the box.
                    pts[i] = (ax + dx + 8, ay + dy + 8);
                }
                surf.fill_polygon(&pts, 0x00FF_FFFF);
            }
            for (i, px) in scratch.iter().enumerate() {
                out[i] = (*px & 0xFF) as u8;
            }
        };

        rasterise(&[(-8, 0), (8, 0), (0, -8), (0, 8)], &mut *(&raw mut MASK_KEY));
        rasterise(&[(0, 0)], &mut *(&raw mut MASK_INK));
        MASK_READY = true;
    }
}

/// Paint a pointer (no save buffer) — safe during the first UI frame.
fn draw_arrow(fb: &Surface, x: i32, y: i32) {
    // Ink body under a white keyline, so the pointer stays legible on both the
    // white page and the blue CTA.
    const INK: u32 = 0x001D_1D1F;
    const KEYLINE: u32 = 0x00FF_FFFF;

    ensure_mask();
    // SAFETY: masks are fully initialised by ensure_mask and never mutated after.
    let (key, ink) = unsafe { (&*(&raw const MASK_KEY), &*(&raw const MASK_INK)) };

    // Origin is shifted back by the 1px keyline margin baked into the mask.
    let ox = x - 1;
    let oy = y - 1;
    fb.mark_dirty(ox, oy, SAVE_W as i32, SAVE_H as i32);
    for row in 0..SAVE_H {
        for col in 0..SAVE_W {
            let i = row * SAVE_W + col;
            let (px, py) = (ox + col as i32, oy + row as i32);
            let k = key[i] as u32;
            if k != 0 {
                fb.blend_pixel(px, py, KEYLINE, k);
            }
            let a = ink[i] as u32;
            if a != 0 {
                fb.blend_pixel(px, py, INK, a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_edge_is_rising_primary_only() {
        let mut m = Mouse::new(100, 100);
        m.buttons = 1;
        assert!(m.take_click_edge());
        assert!(!m.take_click_edge(), "held must not retrigger");
        m.buttons = 0;
        assert!(!m.take_click_edge());
        m.buttons = 1;
        assert!(m.take_click_edge());
        m.buttons = 2;
        assert!(!m.take_click_edge(), "secondary alone is not a click");
    }

    #[test]
    fn arrow_fits_its_save_buffer() {
        // The save/restore box must cover the silhouette plus the 1px keyline.
        let max_x = ARROW.iter().map(|p| p.0).max().unwrap();
        let max_y = ARROW.iter().map(|p| p.1).max().unwrap();
        assert!((max_x / 8 + 2) as usize <= DRAW_W, "arrow wider than save box");
        assert!((max_y / 8 + 2) as usize <= DRAW_H, "arrow taller than save box");
    }

    #[test]
    fn arrow_is_pointer_shaped() {
        // Taller than wide, like every desktop pointer.
        let w = ARROW.iter().map(|p| p.0).max().unwrap();
        let h = ARROW.iter().map(|p| p.1).max().unwrap();
        assert!(h > w, "arrow should be taller than it is wide");
    }
}
