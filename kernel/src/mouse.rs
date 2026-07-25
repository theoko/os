//! PS/2 mouse (relative) + software cursor for the framebuffer UI.
//!
//! UTM often sets `QEMU.PS2Controller = false` and hides the host pointer when
//! the guest window is focused — without a large guest-drawn cursor it looks
//! mouseless. We always paint a chunky arrow; PS/2 moves it when the controller
//! is enabled (`make utm` turns PS/2 on).

use crate::fb::Surface;

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;
const CMD: u16 = 0x64;

/// Keep PS/2 probes short so a missing controller can't stall the UI.
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
    write_cmd(0xD4) && write_data(byte)
}

fn mouse_expect_ack() -> bool {
    matches!(read_data(SPIN), Some(0xFA))
}

/// Relative mouse state (screen pixels).
pub struct Mouse {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub present: bool,
    packet: [u8; 3],
    packet_i: usize,
}

impl Mouse {
    pub fn new(screen_w: i32, screen_h: i32) -> Self {
        Self {
            x: screen_w / 2,
            y: screen_h / 2,
            buttons: 0,
            present: false,
            packet: [0; 3],
            packet_i: 0,
        }
    }

    /// Best-effort PS/2 enable. Returns whether streaming was enabled.
    pub fn init(&mut self) -> bool {
        if !write_cmd(0xA8) {
            return false;
        }
        if !write_cmd(0x20) {
            return false;
        }
        let mut status = match read_data(SPIN) {
            Some(s) => s,
            None => return false,
        };
        status |= 0x02;
        status &= !0x20;
        if !write_cmd(0x60) || !write_data(status) {
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

    /// Drain available bytes; update position. `w`/`h` clamp.
    pub fn poll(&mut self, w: i32, h: i32) -> bool {
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
                // When we've enabled the aux device, accept bytes even if the
                // controller forgets to set the AUX flag (common under TCG).
                if !is_mouse && !self.present && self.packet_i == 0 {
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
                let dx = self.packet[1] as i8 as i32;
                let dy = self.packet[2] as i8 as i32;
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

/// Classic arrow pattern (1 = outline/black, 2 = fill). Drawn at SCALE pixels per cell.
const CURSOR: [[u8; 11]; 17] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0],
    [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0],
    [1, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1],
    [1, 2, 2, 1, 2, 2, 1, 0, 0, 0, 0],
    [1, 2, 1, 0, 1, 2, 2, 1, 0, 0, 0],
    [1, 1, 0, 0, 1, 2, 2, 1, 0, 0, 0],
    [1, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 1, 2, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0],
];

/// 4× scale → ~44×68 — visible on virtio-vga / Retina-scaled UTM windows.
const SCALE: i32 = 4;
const CURSOR_W: usize = 11;
const CURSOR_H: usize = 17;
const DRAW_W: usize = CURSOR_W * SCALE as usize;
const DRAW_H: usize = CURSOR_H * SCALE as usize;
const SAVE_LEN: usize = DRAW_W * DRAW_H;

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
    for row in 0..DRAW_H {
        for col in 0..DRAW_W {
            out[row * DRAW_W + col] = fb.get_pixel(x + col as i32, y + row as i32);
        }
    }
}

fn restore(fb: &Surface, x: i32, y: i32, saved: &[u32; SAVE_LEN]) {
    for row in 0..DRAW_H {
        for col in 0..DRAW_W {
            fb.put_pixel(x + col as i32, y + row as i32, saved[row * DRAW_W + col]);
        }
    }
}

fn draw_arrow(fb: &Surface, x: i32, y: i32) {
    // Blue fill + black outline — matches UI accent, obvious on white.
    const OUTLINE: u32 = 0x0000_0000;
    const FILL: u32 = 0x0000_71E3;
    for row in 0..CURSOR_H {
        for col in 0..CURSOR_W {
            let v = CURSOR[row][col];
            if v == 0 {
                continue;
            }
            let color = if v == 1 { OUTLINE } else { FILL };
            fb.fill_rect(
                x + (col as i32) * SCALE,
                y + (row as i32) * SCALE,
                SCALE,
                SCALE,
                color,
            );
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
    fn cursor_glyph_tip() {
        assert_eq!(CURSOR[0][0], 1);
        assert_eq!(CURSOR[1][1], 1);
    }

    #[test]
    fn scaled_size_roomy() {
        assert!(DRAW_W >= 40);
        assert!(DRAW_H >= 60);
    }
}
