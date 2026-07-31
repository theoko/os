//! PS/2 mouse (relative) + software cursor for the framebuffer UI.
//!
//! UTM often sets `QEMU.PS2Controller = false` and hides the host pointer when
//! the guest window is focused — without a large guest-drawn cursor it looks
//! mouseless. We always paint a chunky arrow; PS/2 moves it when the controller
//! is enabled (`make utm` turns PS/2 on).

use crate::fb::Surface;

#[allow(dead_code)]
const DATA: u16 = 0x60;
#[allow(dead_code)]
const STATUS: u16 = 0x64;
#[allow(dead_code)]
const CMD: u16 = 0x64;

/// Keep PS/2 probes short so a missing controller can't stall the UI.
#[allow(dead_code)]
const SPIN: u32 = 20_000;

#[cfg(target_arch = "x86_64")]
mod port {
    use core::arch::asm;

    #[inline]
    pub unsafe fn outb(port: u16, val: u8) {
        unsafe {
            asm!("out dx, al", in("dx") port, in("al") val, options(nostack, preserves_flags));
        }
    }

    #[inline]
    pub unsafe fn inb(port: u16) -> u8 {
        let val: u8;
        unsafe {
            asm!("in al, dx", out("al") val, in("dx") port, options(nostack, preserves_flags));
        }
        val
    }
}

#[allow(dead_code)]
fn wait_ibf_clear(spins: u32) -> bool {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        for _ in 0..spins {
            if port::inb(STATUS) & 0x02 == 0 {
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn write_cmd(cmd: u8) -> bool {
    if !wait_ibf_clear(SPIN) {
        return false;
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outb(CMD, cmd);
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = cmd;
    true
}

#[allow(dead_code)]
fn write_data(data: u8) -> bool {
    if !wait_ibf_clear(SPIN) {
        return false;
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outb(DATA, data);
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = data;
    true
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn write_mouse(byte: u8) -> bool {
    write_cmd(0xD4) && write_data(byte)
}

#[allow(dead_code)]
fn mouse_expect_ack() -> bool {
    matches!(read_data(SPIN), Some(0xFA))
}

/// Relative mouse state (screen pixels).
pub struct Mouse {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub present: bool,
    #[allow(dead_code)]
    packet: [u8; 3],
    #[allow(dead_code)]
    packet_i: usize,
    /// The controller has produced at least one byte with the AUX flag set —
    /// from then on we can trust the flag and reject keyboard bytes.
    #[allow(dead_code)]
    aux_seen: bool,
}

impl Mouse {
    pub const fn new(w: i32, h: i32) -> Self {
        Self {
            x: w / 2,
            y: h / 2,
            buttons: 0,
            present: false,
            packet: [0; 3],
            packet_i: 0,
            aux_seen: false,
        }
    }

    /// Ask the i8042 to enable the mouse port and streaming mode.
    ///
    /// Returns true when the controller and mouse both ACKed their commands.
    pub fn init(&mut self) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            if !write_cmd(0xA8) {
                return false;
            }
            if !write_cmd(0x20) {
                return false;
            }
            let mut comp = match read_data(SPIN) {
                Some(b) => b,
                None => return false,
            };
            comp |= 0x02;
            comp &= !0x20;
            if !write_cmd(0x60) || !write_data(comp) {
                return false;
            }
            if !write_mouse(0xF6) || !mouse_expect_ack() {
                return false;
            }
            if !write_mouse(0xF4) || !mouse_expect_ack() {
                return false;
            }
            self.present = true;
            true
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Drain available bytes; update position. `w`/`h` clamp.
    pub fn poll(&mut self, w: i32, h: i32) -> bool {
        #[cfg(target_arch = "x86_64")]
        #[allow(unused_mut)]
        let mut moved = false;
        #[cfg(not(target_arch = "x86_64"))]
        let moved = false;
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
                // When we've enabled the aux device, accept bytes even if the
                // controller forgets to set the AUX flag (common under TCG) —
                // but once the AUX flag has ever worked, trust it, so keyboard
                // scancodes are not parsed as mouse packets.
                if !is_mouse && (self.aux_seen || (!self.present && self.packet_i == 0)) {
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
                self.x = (self.x + dx).clamp(0, w.saturating_sub(1));
                self.y = (self.y - dy).clamp(0, h.saturating_sub(1));
                self.buttons = flags & 0x07;
                self.present = true;
                moved = true;
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            let _ = (w, h);
        }
        moved
    }
}

/// A small fixed-point spring between raw pointer input and the drawn cursor.
///
/// Input remains exact for hit-testing.  Only the visual pointer eases toward
/// it, at most once per 60 Hz frame, so a slow hand feels fluid without making
/// a fast user wait for the cursor to catch up.
pub struct CursorMotion {
    x: i32,
    y: i32,
    target_x: i32,
    target_y: i32,
    follow: i32,
}

const MOTION_ONE: i32 = 1 << 16;
/// Precise motion settles over two or three frames: enough filtering to hide
/// report jitter without making the pointer feel detached from the hand.
const GENTLE_FOLLOW: i32 = 42_000;
/// A flick should land in the next frame, like a desktop pointer.
const FAST_FOLLOW: i32 = MOTION_ONE;
const FAST_INPUT_PX: i32 = 48;

impl CursorMotion {
    pub const fn new(x: i32, y: i32) -> Self {
        Self {
            x: x << 16,
            y: y << 16,
            target_x: x << 16,
            target_y: y << 16,
            follow: GENTLE_FOLLOW,
        }
    }

    /// Aim for an input position. Swift, expert-like movements get a stronger
    /// follow factor; precise movement retains a softer game-like glide.
    pub fn set_target(&mut self, x: i32, y: i32) {
        let tx = x << 16;
        let ty = y << 16;
        let distance = ((tx - self.target_x).abs() + (ty - self.target_y).abs()) >> 16;
        self.target_x = tx;
        self.target_y = ty;
        self.follow = if distance >= FAST_INPUT_PX {
            FAST_FOLLOW
        } else {
            GENTLE_FOLLOW
        };
    }

    /// Interactions snap, so the visible pointer and the clicked target agree.
    pub fn snap(&mut self, x: i32, y: i32) {
        self.x = x << 16;
        self.y = y << 16;
        self.target_x = self.x;
        self.target_y = self.y;
    }

    pub fn active(&self) -> bool {
        self.x != self.target_x || self.y != self.target_y
    }

    /// Advance one 60 Hz frame. Returns a new whole-pixel cursor position only
    /// when one needs painting.
    pub fn step(&mut self) -> Option<(i32, i32)> {
        if !self.active() {
            return None;
        }
        let old_x = self.x >> 16;
        let old_y = self.y >> 16;
        self.x = follow(self.x, self.target_x, self.follow);
        self.y = follow(self.y, self.target_y, self.follow);
        let x = self.x >> 16;
        let y = self.y >> 16;
        (x != old_x || y != old_y).then_some((x, y))
    }
}

fn follow(current: i32, target: i32, amount: i32) -> i32 {
    let delta = target - current;
    if delta.abs() <= 512 {
        return target;
    }
    current + ((delta as i64 * amount as i64) / MOTION_ONE as i64) as i32
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
    /// `Surface::draw_count` when `saved` was captured.
    ///
    /// The saved pixels only describe the surface while nothing else draws.
    /// A repaint under a visible cursor invalidates them, and painting them
    /// back then punches a cursor-shaped hole in the new frame — the nav
    /// "os" losing its 's', a switch knob smearing, letters vanishing from a
    /// heading the pointer happened to cross.
    saved_at: u32,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            saved: [0; SAVE_LEN],
            has_saved: false,
            saved_at: 0,
        }
    }

    pub fn hide(&mut self, fb: &Surface) {
        if !self.has_saved {
            return;
        }
        self.has_saved = false;
        // Someone drew since the copy was taken, so it describes a frame that
        // no longer exists. Whatever they drew is already correct underneath —
        // dropping the copy is right, restoring it would corrupt them.
        if fb.draw_count() != self.saved_at {
            return;
        }
        restore(fb, self.x, self.y, &self.saved);
    }

    pub fn show_at(&mut self, fb: &Surface, x: i32, y: i32) {
        self.hide(fb);
        self.x = x;
        self.y = y;
        save(fb, x, y, &mut self.saved);
        self.has_saved = true;
        draw_arrow(fb, x, y);
        // Stamped after the arrow: drawing it is the last legitimate change to
        // this footprint, so anything counted beyond here came from a repaint
        // and means the copy is stale.
        self.saved_at = fb.draw_count();
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
    // Mark where the pixels are actually written, not where the hotspot is:
    // the box starts one pixel up and left of (x, y) to cover the keyline.
    // Marking (x, y) left that column and row repaired in the back buffer but
    // never blitted, so the old keyline stayed on screen as a 1px trail.
    fb.mark_dirty(x - 1, y - 1, SAVE_W as i32, SAVE_H as i32);
    for row in 0..SAVE_H {
        for col in 0..SAVE_W {
            fb.put_pixel(
                x - 1 + col as i32,
                y - 1 + row as i32,
                saved[row * SAVE_W + col],
            );
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

        rasterise(
            &[(-8, 0), (8, 0), (0, -8), (0, 8)],
            &mut *(&raw mut MASK_KEY),
        );
        rasterise(&[(0, 0)], &mut *(&raw mut MASK_INK));
        MASK_READY = true;
    }
}

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

/// Paint a one-shot pointer (no save buffer) — safe during first UI frame.
pub fn paint_pointer(fb: &Surface, x: i32, y: i32) {
    draw_arrow(fb, x, y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_tip_is_at_origin() {
        assert_eq!(ARROW[0], (0, 0), "hotspot must be the polygon tip");
    }

    #[test]
    fn arrow_fits_its_save_buffer() {
        // The save/restore box must cover the silhouette plus the 1px keyline.
        let max_x = ARROW.iter().map(|p| p.0).max().unwrap();
        let max_y = ARROW.iter().map(|p| p.1).max().unwrap();
        assert!(
            (max_x / 8 + 2) as usize <= DRAW_W,
            "arrow wider than save box"
        );
        assert!(
            (max_y / 8 + 2) as usize <= DRAW_H,
            "arrow taller than save box"
        );
    }

    #[test]
    fn arrow_is_pointer_shaped() {
        // Taller than wide, like every desktop pointer.
        let w = ARROW.iter().map(|p| p.0).max().unwrap();
        let h = ARROW.iter().map(|p| p.1).max().unwrap();
        assert!(h > w, "arrow should be taller than it is wide");
    }

    #[test]
    fn cursor_motion_glides_then_lands_exactly() {
        let mut motion = CursorMotion::new(10, 10);
        motion.set_target(30, 10);
        let first = motion.step().expect("first frame should move");
        assert!(first.0 > 10 && first.0 < 30, "a precise move should ease");
        for _ in 0..32 {
            motion.step();
            if !motion.active() {
                break;
            }
        }
        assert!(
            !motion.active(),
            "motion should settle rather than drift forever"
        );
    }

    #[test]
    fn cursor_motion_flick_lands_in_the_next_frame() {
        let mut motion = CursorMotion::new(10, 10);
        motion.set_target(110, 10);
        assert_eq!(motion.step(), Some((110, 10)));
        assert!(
            !motion.active(),
            "a fast move must not leave the pointer behind"
        );
    }

    #[test]
    fn cursor_motion_click_snap_has_no_visual_lag() {
        let mut motion = CursorMotion::new(0, 0);
        motion.set_target(100, 100);
        motion.snap(100, 100);
        assert!(!motion.active());
        assert_eq!(motion.step(), None);
    }

    /// A repaint under a visible cursor must not be damaged when it moves.
    ///
    /// Observed as the pointer eating holes in whatever it crossed: the nav
    /// "os" losing its 's', letters disappearing from a heading. The cursor
    /// had copied the pixels beneath it, the screen repainted underneath, and
    /// moving away stamped that stale copy back over the new frame.
    #[test]
    fn moving_off_a_repainted_screen_does_not_erase_it() {
        const W: usize = 96;
        const H: usize = 64;
        let mut buf = vec![0x00FF_FFFFu32; W * H];
        let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), W, H) };

        let mut cursor = Cursor::new();
        cursor.show_at(&fb, 20, 20);

        // Something repaints the whole screen while the cursor is visible —
        // exactly what a screen transition or a live toggle does.
        const INK: u32 = 0x001D_1D1F;
        fb.fill_rect(0, 0, W as i32, H as i32, INK);

        // Now the pointer moves away.
        cursor.show_at(&fb, 60, 40);

        // Every pixel the old cursor covered must still be the repaint, not
        // the page colour it copied beforehand.
        for row in 0..SAVE_H {
            for col in 0..SAVE_W {
                let (x, y) = (20 - 1 + col as i32, 20 - 1 + row as i32);
                assert_eq!(
                    fb.get_pixel(x, y),
                    INK,
                    "({x},{y}) was restored from a stale copy — the cursor ate the repaint"
                );
            }
        }
    }

    /// The restored footprint has to be advertised where it is actually
    /// written, or `present` never blits the edge and a 1px trail survives.
    #[test]
    fn restoring_marks_the_pixels_it_writes() {
        const W: usize = 96;
        const H: usize = 64;
        let mut buf = vec![0x00FF_FFFFu32; W * H];
        let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), W, H) };

        let mut cursor = Cursor::new();
        cursor.show_at(&fb, 30, 30);
        fb.clear_dirty();
        cursor.hide(&fb);

        let (x0, y0, x1, y1) = fb.dirty_rect().expect("hiding the cursor dirties the screen");
        assert!(x0 <= 29 && y0 <= 29, "dirty rect starts at ({x0},{y0}), misses the keyline");
        assert!(
            x1 >= 30 + SAVE_W as i32 - 1 && y1 >= 30 + SAVE_H as i32 - 1,
            "dirty rect ends at ({x1},{y1}), short of the restored box"
        );
    }

    #[test]
    fn mouse_new_initialises_at_centre() {
        let m = Mouse::new(1024, 768);
        assert_eq!((m.x, m.y), (512, 384));
        assert!(!m.present);
        assert_eq!(m.buttons, 0);
    }

    #[test]
    fn cursor_hide_when_not_visible_is_noop() {
        const W: usize = 96;
        const H: usize = 64;
        let mut buf = vec![0x00FF_FFFFu32; W * H];
        let fb = unsafe { Surface::in_memory(buf.as_mut_ptr(), W, H) };

        let mut cursor = Cursor::new();
        assert!(!cursor.has_saved);
        cursor.hide(&fb);
        assert!(fb.dirty_rect().is_none());
    }
}

