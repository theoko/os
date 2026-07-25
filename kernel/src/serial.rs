//! Early PC UART (COM1 / COM2) for debug + MCP bridge.

/// Line ending used after the hello banner.
pub const LINE_ENDING: &str = "\n";

/// Encode a string as bytes for the serial port (UTF-8 as-is).
pub fn encode_for_serial(s: &str) -> &[u8] {
    s.as_bytes()
}

/// Whether a byte is safe to emit raw on early UART (printable + common whitespace).
pub fn is_early_serial_byte(b: u8) -> bool {
    matches!(b, b'\n' | b'\r' | b'\t' | 0x20..=0x7e)
}

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

/// Classic PC UART.
pub struct Serial {
    #[allow(dead_code)] // read on x86_64 I/O paths
    base: u16,
}

impl Serial {
    pub const COM1: u16 = 0x3F8;
    pub const COM2: u16 = 0x2F8;

    pub const fn new(base: u16) -> Self {
        Self { base }
    }

    pub const fn com1() -> Self {
        Self::new(Self::COM1)
    }

    pub const fn com2() -> Self {
        Self::new(Self::COM2)
    }

    /// Initialize 115200 8N1. Best-effort; QEMU accepts this.
    pub fn init(&self) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            port::outb(self.base + 1, 0x00);
            port::outb(self.base + 3, 0x80);
            port::outb(self.base + 0, 0x01);
            port::outb(self.base + 1, 0x00);
            port::outb(self.base + 3, 0x03);
            port::outb(self.base + 2, 0xC7);
            port::outb(self.base + 4, 0x0B);
        }
    }

    pub fn write_byte(&self, byte: u8) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let mut spins = 0u32;
            while port::inb(self.base + 5) & 0x20 == 0 {
                spins += 1;
                if spins > 1_000_000 {
                    break;
                }
            }
            port::outb(self.base, byte);
        }
        #[cfg(not(target_arch = "x86_64"))]
        let _ = byte;
    }

    pub fn write_bytes(&self, bytes: &[u8]) {
        for &b in bytes {
            if b == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(b);
        }
    }

    pub fn write_str(&self, s: &str) {
        self.write_bytes(encode_for_serial(s));
    }

    /// Non-blocking read: `None` if no byte waiting.
    pub fn try_read_byte(&self) -> Option<u8> {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            if port::inb(self.base + 5) & 0x01 != 0 {
                Some(port::inb(self.base))
            } else {
                None
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        None
    }

    /// Read a line into `buf` (without trailing `\n`). Returns length or None on timeout.
    ///
    /// Framing rules for the COM2 protocol: a line longer than `buf` is
    /// truncated but drained through its terminator, so the tail is never
    /// replayed as the next "line"; a mid-line timeout returns None rather
    /// than presenting a partial accumulation as a complete line.
    ///
    /// If the UART keeps delivering bytes with no terminator (open COM2 with
    /// no peer, or firmware noise), give up after `MAX_OVERRUN` discarded
    /// bytes so we never spin forever — the old buffer-full early return was
    /// the previous escape hatch for that case.
    pub fn read_line(&self, buf: &mut [u8], timeout_spins: u32) -> Option<usize> {
        const MAX_OVERRUN: usize = 4096;
        let mut n = 0usize;
        let mut spins = 0u32;
        let mut overrun = 0usize;
        loop {
            if let Some(b) = self.try_read_byte() {
                spins = 0;
                if b == b'\n' || b == b'\r' {
                    if n > 0 {
                        return Some(n);
                    }
                    continue;
                }
                if n < buf.len() {
                    buf[n] = b;
                    n += 1;
                } else {
                    overrun += 1;
                    if overrun > MAX_OVERRUN {
                        return None;
                    }
                }
            } else {
                spins += 1;
                if spins > timeout_spins {
                    return None;
                }
                core::hint::spin_loop();
            }
        }
    }
}

/// Ask QEMU's `isa-debug-exit` to quit. No-op on UTM / hosts without that device;
/// caller may continue (e.g. interactive mouse loop).
pub fn request_qemu_exit(success: bool) {
    #[cfg(target_arch = "x86_64")]
    {
        let code: u8 = if success { 0x10 } else { 0x11 };
        unsafe {
            port::outb(0xf4, code);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = success;
}

/// QEMU `isa-debug-exit` device (iobase 0xf4): status = `(code << 1) | 1`.
pub fn exit_qemu(success: bool) -> ! {
    request_qemu_exit(success);
    halt()
}

pub fn halt() -> ! {
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
        #[cfg(not(target_arch = "x86_64"))]
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_preserves_utf8() {
        assert_eq!(encode_for_serial("os: hello"), b"os: hello");
    }

    #[test]
    fn early_bytes_accept_banner() {
        for &b in encode_for_serial(crate::HELLO_MESSAGE) {
            assert!(is_early_serial_byte(b), "unexpected byte {b}");
        }
    }
}

/// Read the cycle counter. Used to measure frame cost honestly rather than
/// asserting a frame rate.
#[cfg(target_arch = "x86_64")]
pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    ((hi as u64) << 32) | lo as u64
}

#[cfg(not(target_arch = "x86_64"))]
pub fn rdtsc() -> u64 {
    0
}
