//! Early serial transport for debug + MCP bridge.
//!
//! x86 guests use the classic PC UARTs. ARM64 guests use QEMU/armvirt's
//! PL011-compatible debug UART for COM1; COM2 stays deliberately disabled
//! until the virtual-machine configuration supplies a second serial device.

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

/// Virtual address of the PL011, once someone has told us where physical
/// memory is mapped.
///
/// Limine hands the kernel over with the MMU **on** and the kernel living in
/// the higher half, so the PL011's physical address is not a valid pointer:
/// writing to it takes a synchronous data abort, and with no vector table
/// installed the CPU lands in unmapped memory at VBAR+0x200 and stops. That
/// is a completely silent death — it happened after Limine printed "Loading
/// executable" and before the kernel could say anything at all.
///
/// Zero means "not yet located"; `pl011_base()` then falls back to the
/// physical address, which is correct on any machine that identity-maps it
/// and no worse than the old behaviour anywhere else.
#[cfg(target_arch = "aarch64")]
static PL011_VIRT: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

/// Physical base of the PL011 on QEMU `virt` / VirtualBox `armv8virtual`.
#[cfg(target_arch = "aarch64")]
pub const PL011_PHYS: usize = 0x0900_0000;

/// Point the early console at `hhdm_offset + PL011_PHYS`.
///
/// Call this before the first write, from `kmain`, using Limine's HHDM
/// response. Idempotent.
#[cfg(target_arch = "aarch64")]
pub fn locate_pl011(hhdm_offset: u64) {
    if hhdm_offset != 0 {
        PL011_VIRT.store(
            (hhdm_offset as usize).wrapping_add(PL011_PHYS),
            core::sync::atomic::Ordering::SeqCst,
        );
    }
}

#[cfg(target_arch = "aarch64")]
fn pl011_base() -> Option<usize> {
    match PL011_VIRT.load(core::sync::atomic::Ordering::SeqCst) {
        0 => None,
        v => Some(v),
    }
}

/// Early debug or bridge serial device.
pub struct Serial {
    #[cfg(target_arch = "x86_64")]
    base: u16,
    #[cfg(target_arch = "aarch64")]
    base: Option<usize>,
}

impl Serial {
    #[cfg(target_arch = "x86_64")]
    pub const COM1: u16 = 0x3F8;
    #[cfg(target_arch = "x86_64")]
    pub const COM2: u16 = 0x2F8;
    #[cfg(target_arch = "aarch64")]
    pub const PL011_COM1: usize = PL011_PHYS;

    #[cfg(target_arch = "x86_64")]
    pub const fn new(base: u16) -> Self {
        Self { base }
    }

    #[cfg(target_arch = "aarch64")]
    pub const fn new(base: Option<usize>) -> Self {
        Self { base }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    pub const fn new() -> Self {
        Self {}
    }

    pub fn com1() -> Self {
        #[cfg(target_arch = "x86_64")]
        return Self::new(Self::COM1);
        #[cfg(target_arch = "aarch64")]
        return Self::new(pl011_base());
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        Self::new()
    }

    pub const fn com2() -> Self {
        #[cfg(target_arch = "x86_64")]
        return Self::new(Self::COM2);
        #[cfg(target_arch = "aarch64")]
        return Self::new(None);
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        Self::new()
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
        #[cfg(target_arch = "aarch64")]
        if let Some(base) = self.base {
            // QEMU virt / VirtualBox armvirt PL011 at 24 MHz, 115200 8N1.
            unsafe {
                core::ptr::write_volatile((base + 0x30) as *mut u32, 0);
                core::ptr::write_volatile((base + 0x44) as *mut u32, 0x7ff);
                core::ptr::write_volatile((base + 0x24) as *mut u32, 13);
                core::ptr::write_volatile((base + 0x28) as *mut u32, 1);
                core::ptr::write_volatile((base + 0x2c) as *mut u32, 0x70);
                core::ptr::write_volatile((base + 0x30) as *mut u32, 0x301);
            }
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
        #[cfg(target_arch = "aarch64")]
        if let Some(base) = self.base {
            let mut spins = 0u32;
            unsafe {
                while core::ptr::read_volatile((base + 0x18) as *const u32) & (1 << 5) != 0 {
                    spins += 1;
                    if spins > 1_000_000 {
                        break;
                    }
                    core::hint::spin_loop();
                }
                core::ptr::write_volatile(base as *mut u32, byte as u32);
            }
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
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
        #[cfg(target_arch = "aarch64")]
        if let Some(base) = self.base {
            unsafe {
                if core::ptr::read_volatile((base + 0x18) as *const u32) & (1 << 4) == 0 {
                    Some(core::ptr::read_volatile(base as *const u32) as u8)
                } else {
                    None
                }
            }
        } else {
            None
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
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
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!("wfe", options(nomem, nostack));
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
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
