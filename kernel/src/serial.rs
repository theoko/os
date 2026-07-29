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

/// VirtualBox ARM maps its second serial port (16550A, not PL011) through the
/// PIO bridge window at 0xFCD20000. ISA port 0x2F8 (COM2) sits directly at
/// pio_base + 0x2F8. Registers are byte-wide u32 slots (one register per u32).
#[cfg(target_arch = "aarch64")]
const VBOX_COM2_BASE: usize = 0xFCD2_02F8;

/// Resolved COM2 MMIO base on VirtualBox ARM. 0 = not present.
#[cfg(target_arch = "aarch64")]
static COM2_BASE: AtomicUsize = AtomicUsize::new(0);

/// Emit the kernel log on VirtualBox's ARM PL011.
///
/// Off by default: see `write_byte`. Turn on to diagnose something on that
/// platform that the screen cannot show — driver bring-up, in particular,
/// where "nothing happened" and "it happened and was dropped" look identical
/// from in front of the machine.
#[cfg(target_arch = "aarch64")]
const VBOX_SERIAL: bool = false;

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

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
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

/// Hosted unit tests run as a Linux/macOS process: `in`/`out` to COM ports
/// SIGSEGV. Freestanding guests (`target_os = "none"`) keep real PIO above.
#[cfg(all(target_arch = "x86_64", not(target_os = "none")))]
mod port {
    #[inline]
    pub unsafe fn outb(_port: u16, _val: u8) {}

    #[inline]
    pub unsafe fn inb(_port: u16) -> u8 {
        0
    }
}

/// Which PL011 base this machine actually has.
///
/// Defaults to QEMU's so behaviour is unchanged until `detect_pl011` runs.
/// Nothing writes through this address without `pl011_present` agreeing that a
/// PL011 is really there, so an undetected machine stays silent instead of
/// aborting.
#[cfg(target_arch = "aarch64")]
static PL011_BASE: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(Serial::PL011_COM1);

#[cfg(target_arch = "aarch64")]
pub fn pl011_base() -> usize {
    PL011_BASE.load(core::sync::atomic::Ordering::Relaxed)
}

/// Can this virtual address be dereferenced at all?
///
/// `AT S1E1R` runs the translation-table walk the MMU would run, without
/// performing the access, and reports the outcome in PAR_EL1. An unmapped
/// address therefore *reports* a fault instead of *taking* one.
///
/// Every probe below goes through this, because a UART probe that can fault is
/// worse than no probe. Limine hands the kernel over with the MMU on and its
/// higher-half direct map covering RAM, not device MMIO: on QEMU `virt` the
/// PL011's raw physical address is not a pointer, and touching it takes a
/// synchronous data abort. There is no vector table yet, so the CPU lands at
/// VBAR+0x200 in unmapped memory and stops — a completely silent death, after
/// Limine has printed "Loading executable" and before the kernel can say
/// anything at all. (An earlier comment here claimed such an access "simply
/// goes nowhere". Measured on QEMU, it does not: PC=0x200, X09=0x09000030.)
#[cfg(all(target_arch = "aarch64", target_os = "none"))]
fn va_mapped(va: usize) -> bool {
    let par: u64;
    unsafe {
        core::arch::asm!(
            "at s1e1r, {va}",
            "isb",
            "mrs {par}, par_el1",
            va = in(reg) va as u64,
            par = out(reg) par,
            options(nostack),
        );
    }
    // PAR_EL1.F: set means the walk faulted, i.e. nothing is mapped there.
    par & 1 == 0
}

/// Hosted builds (the unit-test target) have no EL1 and no device memory, so
/// no candidate address is ever probeable.
#[cfg(all(target_arch = "aarch64", not(target_os = "none")))]
fn va_mapped(_va: usize) -> bool {
    false
}

/// Does a PL011 answer at `base`?
///
/// Identified by the PrimeCell peripheral ID registers, which are constant for
/// the part. Reading a plausible-looking value out of unmapped space is how a
/// probe talks itself into the wrong address, so check all four — and refuse
/// to read at all unless the page is mapped.
#[cfg(target_arch = "aarch64")]
fn is_pl011(base: usize) -> bool {
    const PERIPH_ID: [(usize, u8); 4] =
        [(0xFE0, 0x11), (0xFE4, 0x10), (0xFE8, 0x14), (0xFEC, 0x00)];
    // Ask the MMU to translate before touching the device. Reading an
    // unmapped candidate is a synchronous abort into vectors that do not
    // exist yet, which is a silent death, not a failed probe.
    if !pl011_page_mapped(base) {
        return false;
    }
    PERIPH_ID.iter().all(|(off, want)| {
        // SAFETY: `pl011_page_mapped` just proved this page translates; a read
        // of a wrong-but-mapped address returns a value that fails this check.
        let v = unsafe { core::ptr::read_volatile((base + off) as *const u32) };
        (v & 0xFF) as u8 == *want
    })
}

/// Is the whole PL011 register window at `base` reachable?
///
/// The ID registers live at the top of the device's 4 KiB page; checking the
/// first and last address any probe touches covers every read below even if a
/// candidate base is not page-aligned.
#[cfg(target_arch = "aarch64")]
fn pl011_page_mapped(base: usize) -> bool {
    va_mapped(base) && va_mapped(base + 0xFF4)
}

/// Does a 16550A-compatible UART answer at `base` (VirtualBox ARM PIO-mapped)?
///
/// We use the scratch register (offset +7) loopback: write a canary, read it
/// back. On an empty PIO window the read returns 0xFF or 0x00 regardless of
/// the write; a real 16550A echo confirms the device. The page must be mapped
/// first — writing to an unmapped PIO window is a synchronous data abort.
#[cfg(target_arch = "aarch64")]
fn is_16550a(base: usize) -> bool {
    // Each 16550A register is one u32 slot in the PIO window (stride = 4 bytes
    // on VirtualBox ARM). Scratch = offset 7 → 0x1C bytes from base.
    const SCR: usize = 7 * 4;
    if !va_mapped(base) || !va_mapped(base + SCR) {
        return false;
    }
    unsafe {
        let ptr = (base + SCR) as *mut u32;
        core::ptr::write_volatile(ptr, 0xA5);
        let v = core::ptr::read_volatile(ptr as *const u32);
        v & 0xFF == 0xA5
    }
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
pub fn detect_pl011(_hhdm: u64) -> Option<usize> {
    // `kmain` installs an identity-mapped Device window before calling here.
    // Never bias MMIO through Limine's HHDM: that map describes RAM, and an
    // HHDM alias of a device address can synchronously abort on aarch64 before
    // the kernel has installed its exception vectors.
    for base in Serial::PL011_CANDIDATES {
        if is_pl011(base) {
            PL011_BASE.store(base, core::sync::atomic::Ordering::Relaxed);
            return Some(base);
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
        return Self::new(None); // base resolved at init time via COM2_BASE
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        Self::new()
    }

    /// Whether this serial object names a device on this architecture.
    ///
    /// For COM1, ARM always has a PL011. For COM2, ARM needs the VirtualBox
    /// PIO-mapped 16550A to have been detected by `init`.
    pub fn available(&self) -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            true
        }
        #[cfg(target_arch = "aarch64")]
        {
            // COM1: base is Some(). COM2: base is None but COM2_BASE may be set.
            self.base.is_some() || COM2_BASE.load(Ordering::Relaxed) != 0
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
            // COM1 path: PL011 detection and initialization.
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
        } else {
            // COM2 path: probe VirtualBox's PIO-mapped 16550A.
            // Register stride is 4 bytes (one u32 slot per ISA register).
            if is_16550a(VBOX_COM2_BASE) {
                COM2_BASE.store(VBOX_COM2_BASE, Ordering::Release);
                // 115200 8N1: divisor = 1 at 1.8432 MHz base clock.
                unsafe {
                    let b = VBOX_COM2_BASE;
                    // DLAB=1: enable divisor latch
                    core::ptr::write_volatile((b + 3 * 4) as *mut u32, 0x80);
                    core::ptr::write_volatile((b + 0 * 4) as *mut u32, 0x01); // DLL
                    core::ptr::write_volatile((b + 1 * 4) as *mut u32, 0x00); // DLH
                    // DLAB=0, 8 data bits, no parity, 1 stop
                    core::ptr::write_volatile((b + 3 * 4) as *mut u32, 0x03);
                    // FIFO enable, clear, 14-byte threshold
                    core::ptr::write_volatile((b + 2 * 4) as *mut u32, 0xC7);
                    // DTR + RTS (modem control)
                    core::ptr::write_volatile((b + 4 * 4) as *mut u32, 0x0B);
                }
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
        {
            // COM1: write through the PL011 when self.base is Some.
            if self.base.is_some() {
                let base = ACTIVE_PL011.load(Ordering::Acquire);
                if base == 0 {
                    return;
                }
                if base == VBOX_PL011 && !VBOX_SERIAL {
                    return;
                }
                unsafe {
                    if core::ptr::read_volatile((base + 0x18) as *const u32) & (1 << 5) == 0 {
                        core::ptr::write_volatile(base as *mut u32, byte as u32);
                    }
                }
            } else {
                // COM2: write through the VirtualBox PIO-mapped 16550A.
                let base = COM2_BASE.load(Ordering::Acquire);
                if base == 0 {
                    return;
                }
                unsafe {
                    // Spin briefly on THR Empty (LSR bit 5) at register offset 5*4.
                    let mut spins = 0u32;
                    while core::ptr::read_volatile((base + 5 * 4) as *const u32) & 0x20 == 0 {
                        spins += 1;
                        if spins > 100_000 { break; }
                    }
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
        {
            if self.base.is_some() {
                // COM1: PL011 data register (RXE empty = flag bit 4 in UARTFR).
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
                // COM2: 16550A LSR (offset 5*4) bit 0 = Data Ready.
                let base = COM2_BASE.load(Ordering::Acquire);
                if base == 0 {
                    return None;
                }
                unsafe {
                    if core::ptr::read_volatile((base + 5 * 4) as *const u32) & 0x01 != 0 {
                        Some(core::ptr::read_volatile(base as *const u32) as u8)
                    } else {
                        None
                    }
                }
            }
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

/// PrimeCell identity check used by `init`, covering the PCellID registers as
/// well as the peripheral IDs.
///
/// Like `is_pl011` this refuses to read an address the MMU cannot translate.
/// `com1()` hands `init` whichever base detection settled on — including the
/// unverified QEMU default when detection found nothing — so this is the last
/// gate before the kernel writes to something that may not be there.
#[cfg(target_arch = "aarch64")]
fn pl011_present(base: usize) -> bool {
    if !pl011_page_mapped(base) {
        return false;
    }
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
