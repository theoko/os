//! Early serial transport for debug + MCP bridge.
//!
//! x86 guests use the classic PC UARTs. ARM64 guests use QEMU/armvirt's
//! PL011-compatible debug UART for COM1; COM2 stays deliberately disabled
//! until the virtual-machine configuration supplies a second serial device.

#[cfg(target_arch = "aarch64")]
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_arch = "aarch64")]
static ACTIVE_PL011: AtomicUsize = AtomicUsize::new(0);

#[cfg(target_arch = "aarch64")]
const VBOX_PL011: usize = 0xFFDD_E000;

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

/// Which PL011 base this machine actually has.
///
/// Defaults to QEMU's so behaviour is unchanged until `detect_pl011` runs.
#[cfg(target_arch = "aarch64")]
static PL011_BASE: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(Serial::PL011_COM1);

#[cfg(target_arch = "aarch64")]
pub fn pl011_base() -> usize {
    PL011_BASE.load(core::sync::atomic::Ordering::Relaxed)
}

/// Does a PL011 answer at `base`?
///
/// Identified by the PrimeCell peripheral ID registers, which are constant for
/// the part. Reading a plausible-looking value out of unmapped space is how a
/// probe talks itself into the wrong address, so check all four.
#[cfg(target_arch = "aarch64")]
fn is_pl011(base: usize) -> bool {
    const PERIPH_ID: [(usize, u8); 4] = [(0xFE0, 0x11), (0xFE4, 0x10), (0xFE8, 0x14), (0xFEC, 0x00)];
    PERIPH_ID.iter().all(|(off, want)| {
        // SAFETY: device memory the firmware has already mapped; a read of a
        // wrong-but-mapped address returns a value that fails this check.
        let v = unsafe { core::ptr::read_volatile((base + off) as *const u32) };
        (v & 0xFF) as u8 == *want
    })
}

/// Find the UART before anything tries to log through it.
///
/// QEMU's `virt` and VirtualBox's `armv8virtual` put their PL011 at different
/// addresses, and the kernel hardcoded QEMU's. On VirtualBox every log line
/// went into unmapped space, so a guest that booted, rendered its home screen
/// and drove its USB controller still produced a serial log containing nothing
/// but firmware output - and its crashes had to be guessed at from
/// screenshots.
#[cfg(target_arch = "aarch64")]
pub fn detect_pl011(hhdm: u64) -> Option<usize> {
    // Try through the higher-half direct map first, then raw.
    //
    // Limine hands the kernel an MMU that maps physical memory at an offset,
    // so a raw physical address is not a valid pointer. Writing to one is not
    // a crash - it simply goes nowhere, which is why the ARM guest produced no
    // serial output under either QEMU or VirtualBox while appearing to boot
    // normally.
    for base in Serial::PL011_CANDIDATES {
        for candidate in [hhdm as usize + base, base] {
            if is_pl011(candidate) {
                PL011_BASE.store(candidate, core::sync::atomic::Ordering::Relaxed);
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(not(target_arch = "aarch64"))]
pub fn detect_pl011(_hhdm: u64) -> Option<usize> {
    None
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
    /// Where QEMU's ARM `virt` machine puts its PL011.
    #[cfg(target_arch = "aarch64")]
    pub const PL011_COM1: usize = 0x0900_0000;

    /// Where VirtualBox's `armv8virtual` machine puts its PL011.
    ///
    /// Taken from its own device map: `MMIO arm-pl011` at ffdde000. Hardcoding
    /// QEMU's address meant the kernel wrote every log line into unmapped
    /// space on VirtualBox, so an ARM guest that booted, rendered and drove its
    /// USB controller still produced a serial log containing nothing but UEFI
    /// firmware output - and every crash there had to be diagnosed from
    /// screenshots.
    #[cfg(target_arch = "aarch64")]
    pub const PL011_VBOX: usize = 0xFFDD_E000;

    /// PL011 bases to try, in order.
    #[cfg(target_arch = "aarch64")]
    pub const PL011_CANDIDATES: [usize; 2] = [Self::PL011_COM1, Self::PL011_VBOX];

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
        return Self::new(Some(pl011_base()));
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

    /// Whether this serial object names a device on this architecture.
    ///
    /// ARM COM2 is intentionally absent until a second UART is discovered.
    /// Callers can use this to avoid a synthetic timeout loop when there is
    /// no transport to wait for in the first place.
    pub const fn available(&self) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            true
        }
        #[cfg(target_arch = "aarch64")]
        {
            self.base.is_some()
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            false
        }
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
        if let Some(preferred) = self.base {
            // Do not infer a UART from an address alone. Both QEMU and
            // VirtualBox expose standard PrimeCell IDs, but at different
            // addresses. Remember the device that actually answers so every
            // byte after this is a single MMIO path with no probing.
            let base = [preferred, VBOX_PL011]
                .into_iter()
                .find(|base| pl011_present(*base))
                .unwrap_or(0);
            ACTIVE_PL011.store(base, Ordering::Release);
            if base == 0 {
                return;
            }
            // QEMU virt and VirtualBox armvirt use a 24 MHz PL011, 115200 8N1.
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
        if self.base.is_some() {
            let base = ACTIVE_PL011.load(Ordering::Acquire);
            if base == 0 {
                return;
            }
            // VirtualBox's ARM PL011 can leave TX full indefinitely after
            // firmware hands it off. Even inspecting the full flag then makes
            // boot timing nondeterministic, so the framebuffer is the sole
            // diagnostic surface on that platform. QEMU retains serial logs.
            if base == VBOX_PL011 {
                return;
            }
            unsafe {
                // Debug output is never allowed to pace the guest. VirtualBox
                // can leave the PL011 FIFO full after firmware hands it off;
                // even a short spin loop then becomes thousands of emulated
                // MMIO reads per message and stalls boot before USB starts.
                // Send when there is room and otherwise drop this diagnostic
                // byte. The framebuffer remains the authoritative UI.
                if core::ptr::read_volatile((base + 0x18) as *const u32) & (1 << 5) == 0 {
                    core::ptr::write_volatile(base as *mut u32, byte as u32);
                }
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
        if self.base.is_some() {
            let base = ACTIVE_PL011.load(Ordering::Acquire);
            if base == 0 {
                return None;
            }
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

#[cfg(target_arch = "aarch64")]
fn pl011_present(base: usize) -> bool {
    unsafe {
        core::ptr::read_volatile((base + 0xFE0) as *const u32) & 0xFF == 0x11
            && core::ptr::read_volatile((base + 0xFE4) as *const u32) & 0xFF == 0x10
            && core::ptr::read_volatile((base + 0xFF0) as *const u32) & 0xFF == 0x0D
            && core::ptr::read_volatile((base + 0xFF4) as *const u32) & 0xFF == 0xF0
    }
}

/// True after COM1 initialization identified VirtualBox's ARM device map.
pub fn is_virtualbox_arm() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        ACTIVE_PL011.load(Ordering::Acquire) == VBOX_PL011
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        false
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

/// Read the architecture's monotonic performance counter. Used to measure
/// frame cost honestly rather than asserting a frame rate.
#[cfg(target_arch = "x86_64")]
pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack));
    }
    ((hi as u64) << 32) | lo as u64
}

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub fn rdtsc() -> u64 {
    let ticks: u64;
    unsafe {
        core::arch::asm!("isb", "mrs {}, cntvct_el0", out(reg) ticks, options(nomem, nostack));
    }
    ticks
}

#[cfg(not(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", target_os = "none")
)))]
pub fn rdtsc() -> u64 {
    0
}

/// Frequency of [`rdtsc`] in ticks per second.
///
/// x86 keeps the historical 1 GHz pacing assumption because deriving the TSC
/// frequency portably needs CPUID/ACPI calibration. ARM exposes CNTFRQ_EL0,
/// so animation timing there is exact rather than guessed.
#[cfg(target_arch = "x86_64")]
pub fn counter_hz() -> u64 {
    1_000_000_000
}

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub fn counter_hz() -> u64 {
    let hz: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) hz, options(nomem, nostack));
    }
    hz
}

#[cfg(not(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", target_os = "none")
)))]
pub fn counter_hz() -> u64 {
    0
}
