//! Tiny PCI config-space helpers.
//!
//! x86 uses the legacy configuration ports. ARM virtual machines expose the
//! standard PCI ECAM window instead; keeping both behind this module lets USB
//! discovery and the eventual xHCI input driver share one bus view.
//!
//! The two access paths meet inside `read32`/`write32`, so every scanner below
//! is written once and works on both machines.

use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_arch = "x86_64")]
mod port {
    use core::arch::asm;

    #[inline]
    pub unsafe fn outl(port: u16, val: u32) {
        unsafe {
            asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack, preserves_flags));
        }
    }

    #[inline]
    pub unsafe fn inl(port: u16) -> u32 {
        let val: u32;
        unsafe {
            asm!("in eax, dx", out("eax") val, in("dx") port, options(nostack, preserves_flags));
        }
        val
    }

    #[inline]
    pub unsafe fn outw(port: u16, val: u16) {
        unsafe {
            asm!("out dx, ax", in("dx") port, in("ax") val, options(nostack, preserves_flags));
        }
    }

    #[inline]
    pub unsafe fn inw(port: u16) -> u16 {
        let val: u16;
        unsafe {
            asm!("in ax, dx", out("ax") val, in("dx") port, options(nostack, preserves_flags));
        }
        val
    }

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

const CONFIG_ADDR: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

/// Where QEMU's `virt` machine has historically put its high ECAM window.
///
/// A fallback, never a first choice: the base moves with machine type, RAM
/// size and whether UEFI firmware is loaded, and VirtualBox's `armv8virtual`
/// is a different device model entirely. Read ACPI MCFG first and only fall
/// back here — `use_ecam` refuses a window that does not answer, so a wrong
/// guess degrades to "no PCI" instead of an abort.
pub const QEMU_VIRT_ECAM: u64 = 0x0000_0040_1000_0000;

/// Virtual base of the ECAM window (already direct-map biased), or 0 for
/// "there is no window; use the legacy ports".
///
/// Runtime state, not a `cfg`: `cargo test` runs on an arm64 Mac, so
/// `cfg!(target_arch)` describes the machine running the tests rather than the
/// machine being driven. `inputdiag.rs` records that trap being sprung once
/// already, and this is the layer where it would be silent.
static ECAM_BASE: AtomicUsize = AtomicUsize::new(0);

/// `start_bus | end_bus << 8 | 1 << 16`, so 0 still means "unset".
static ECAM_BUSES: AtomicUsize = AtomicUsize::new(0);

/// Does anything answer on the legacy configuration ports?
///
/// The question is asked at runtime so the x86 path stays exactly what it was:
/// where the ports work they keep being used, and ECAM is never installed.
pub fn port_io_works() -> bool {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outl(CONFIG_ADDR, cfg_addr(0, 0, 0, 0));
        port::inl(CONFIG_DATA) != 0xFFFF_FFFF
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// Install a config window. `virt_base` must already include the direct-map
/// offset, because `read32` is a free function with no way to reach it.
///
/// Returns false — leaving the ports selected — when nothing on the first bus
/// answers. A base read out of a bad table must degrade, not fault.
///
/// The probe sweeps the first bus rather than asking 00:00.0 alone: which slot
/// holds the host bridge is a platform decision, and an empty slot 0 would
/// otherwise condemn a window that works.
pub fn use_ecam(virt_base: usize, start_bus: u8, end_bus: u8) -> bool {
    if virt_base == 0 || end_bus < start_bus {
        return false;
    }
    ECAM_BASE.store(virt_base, Ordering::SeqCst);
    ECAM_BUSES.store(
        (start_bus as usize) | ((end_bus as usize) << 8) | (1 << 16),
        Ordering::SeqCst,
    );
    let answered = (0..32u8).any(|slot| read32(start_bus, slot, 0, 0x00) != 0xFFFF_FFFF);
    if !answered {
        clear_ecam();
    }
    answered
}

pub fn clear_ecam() {
    ECAM_BASE.store(0, Ordering::SeqCst);
    ECAM_BUSES.store(0, Ordering::SeqCst);
}

pub fn ecam_active() -> bool {
    ECAM_BASE.load(Ordering::SeqCst) != 0
}

pub fn ecam_bus_range() -> Option<(u8, u8)> {
    let packed = ECAM_BUSES.load(Ordering::SeqCst);
    (packed & (1 << 16) != 0).then(|| ((packed & 0xFF) as u8, ((packed >> 8) & 0xFF) as u8))
}

/// Byte offset of a config dword inside a window, or `None` when the bus falls
/// outside it.
///
/// Pure, so it can be tested without hardware — and it is the only thing
/// standing between `usb_survey`'s 0..=255 sweep and a data abort. A window is
/// as narrow as 16 buses; on x86 a read past the end floats harmlessly to
/// 0xFFFF, on aarch64 it is a synchronous external abort into a kernel with no
/// vector table.
pub const fn ecam_offset(
    bus: u8,
    slot: u8,
    func: u8,
    offset: u8,
    start_bus: u8,
    end_bus: u8,
) -> Option<usize> {
    if bus < start_bus || bus > end_bus {
        return None;
    }
    Some(
        (((bus - start_bus) as usize) << 20)
            | ((slot as usize) << 15)
            | ((func as usize) << 12)
            | ((offset as usize) & 0xFC),
    )
}

fn ecam_ptr(bus: u8, slot: u8, func: u8, offset: u8) -> Option<*mut u32> {
    let base = ECAM_BASE.load(Ordering::SeqCst);
    if base == 0 {
        return None;
    }
    let (lo, hi) = ecam_bus_range()?;
    let off = ecam_offset(bus, slot, func, offset, lo, hi)?;
    Some((base + off) as *mut u32)
}

fn cfg_addr(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

pub fn read32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    if let Some(p) = ecam_ptr(bus, slot, func, offset) {
        return unsafe { core::ptr::read_volatile(p) };
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outl(CONFIG_ADDR, cfg_addr(bus, slot, func, offset));
        port::inl(CONFIG_DATA)
    }
    // No window and no ports: an absent device, which is what every scanner
    // below already knows how to handle.
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (bus, slot, func, offset);
        0xFFFF_FFFF
    }
}

pub fn write32(bus: u8, slot: u8, func: u8, offset: u8, val: u32) {
    if let Some(p) = ecam_ptr(bus, slot, func, offset) {
        unsafe { core::ptr::write_volatile(p, val) };
        return;
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outl(CONFIG_ADDR, cfg_addr(bus, slot, func, offset));
        port::outl(CONFIG_DATA, val);
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = (bus, slot, func, offset, val);
}

pub fn read16(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    let v = read32(bus, slot, func, offset & 0xFC);
    ((v >> (8 * (offset as u32 & 2))) & 0xFFFF) as u16
}

pub fn write16(bus: u8, slot: u8, func: u8, offset: u8, val: u16) {
    let aligned = offset & 0xFC;
    let shift = 8 * (offset as u32 & 2);
    let mut v = read32(bus, slot, func, aligned);
    v &= !(0xFFFF << shift);
    v |= (val as u32) << shift;
    // The status register (0x06) is RW1C: writing back the 1s we just read
    // would clear them. When updating the command register, write 0s to the
    // status half instead (0s are a no-op for RW1C bits).
    if aligned == 0x04 && shift == 0 {
        v &= 0x0000_FFFF;
    }
    write32(bus, slot, func, aligned, v);
}

/// UHCI = class 0x0C, subclass 0x03, prog-if 0x00.
pub fn find_all_uhci() -> heapless_vec::UhciList {
    let mut out = heapless_vec::UhciList::new();
    for bus in 0..4u8 {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let id = read32(bus, slot, func, 0x00);
                if id == 0xFFFF_FFFF {
                    if func == 0 {
                        break;
                    }
                    continue;
                }
                let class = read32(bus, slot, func, 0x08);
                let base_class = (class >> 24) & 0xFF;
                let subclass = (class >> 16) & 0xFF;
                let prog_if = (class >> 8) & 0xFF;
                if base_class == 0x0C && subclass == 0x03 && prog_if == 0x00 {
                    let bar4 = read32(bus, slot, func, 0x20);
                    if bar4 & 1 == 1 {
                        let io = (bar4 & 0xFFE0) as u16;
                        let cmd = read16(bus, slot, func, 0x04);
                        write16(bus, slot, func, 0x04, cmd | 0x05);
                        out.push((bus, slot, func, io));
                    }
                }
                let header = (read32(bus, slot, func, 0x0C) >> 16) as u8;
                if func == 0 && header & 0x80 == 0 {
                    break;
                }
            }
        }
    }
    out
}

/// EHCI = class 0x0C, subclass 0x03, prog-if 0x20. Returns MMIO BAR phys.
/// Every EHCI controller on the bus.
///
/// q35 with `-usb` builds an ICH9 set at 00:1d.x, and UTM adds a *second*
/// explicit `ich9-usb-ehci1`. Disabling only the first leaves the other still
/// owning its ports, so its UHCI companions see nothing.
pub fn for_each_ehci(mut f: impl FnMut(u8, u8, u8)) {
    for bus in 0..4u8 {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let id = read32(bus, slot, func, 0x00);
                if id == 0xFFFF_FFFF {
                    if func == 0 {
                        break;
                    }
                    continue;
                }
                let class = read32(bus, slot, func, 0x08);
                if (class >> 24) & 0xFF == 0x0C
                    && (class >> 16) & 0xFF == 0x03
                    && (class >> 8) & 0xFF == 0x20
                {
                    f(bus, slot, func);
                }
                let header = (read32(bus, slot, func, 0x0C) >> 16) as u8;
                if func == 0 && header & 0x80 == 0 {
                    break;
                }
            }
        }
    }
}

pub fn find_ehci_mmio() -> Option<(u8, u8, u8, u64)> {
    for bus in 0..4u8 {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let id = read32(bus, slot, func, 0x00);
                if id == 0xFFFF_FFFF {
                    if func == 0 {
                        break;
                    }
                    continue;
                }
                let class = read32(bus, slot, func, 0x08);
                let base_class = (class >> 24) & 0xFF;
                let subclass = (class >> 16) & 0xFF;
                let prog_if = (class >> 8) & 0xFF;
                if base_class == 0x0C && subclass == 0x03 && prog_if == 0x20 {
                    let bar0 = read32(bus, slot, func, 0x10);
                    if bar0 & 1 == 0 {
                        let mem = (bar0 & 0xFFFF_FFF0) as u64;
                        let cmd = read16(bus, slot, func, 0x04);
                        write16(bus, slot, func, 0x04, cmd | 0x06); // mem + bus master
                        return Some((bus, slot, func, mem));
                    }
                }
                let header = (read32(bus, slot, func, 0x0C) >> 16) as u8;
                if func == 0 && header & 0x80 == 0 {
                    break;
                }
            }
        }
    }
    None
}

/// xHCI = class 0x0C, subclass 0x03, prog-if 0x30. Returns its MMIO BAR.
///
/// The controller is present on VirtualBox ARM when USB 3 is enabled. This
/// does not start it; the xHCI driver owns that once its command/event rings
/// have been installed.
pub fn find_xhci_mmio() -> Option<(u8, u8, u8, u64)> {
    for bus in 0..4u8 {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let id = read32(bus, slot, func, 0x00);
                if id == 0xFFFF_FFFF {
                    if func == 0 {
                        break;
                    }
                    continue;
                }
                let class = read32(bus, slot, func, 0x08);
                if (class >> 24) & 0xFF == 0x0C
                    && (class >> 16) & 0xFF == 0x03
                    && (class >> 8) & 0xFF == PROG_IF_XHCI
                {
                    let lo = read32(bus, slot, func, 0x10);
                    if lo & 1 == 0 {
                        let mut base = (lo & 0xFFFF_FFF0) as u64;
                        if (lo >> 1) & 0x3 == 0x2 {
                            base |= (read32(bus, slot, func, 0x14) as u64) << 32;
                        }
                        // xHCI will neither decode its BAR nor fetch DMA
                        // rings until these PCI command bits are set. Do this
                        // at discovery time, before the driver ever writes an
                        // operational register; `write16` preserves RW1C
                        // status bits in the other half of this dword.
                        let command = read16(bus, slot, func, 0x04);
                        write16(bus, slot, func, 0x04, command | 0x0006);
                        return Some((bus, slot, func, base));
                    }
                }
                let header = (read32(bus, slot, func, 0x0C) >> 16) as u8;
                if func == 0 && header & 0x80 == 0 {
                    break;
                }
            }
        }
    }
    None
}

/// Walk every function that answers, stopping as soon as `f` returns true.
///
/// One loop for every later scanner, rather than a second copy for the ECAM
/// path: the two access methods already meet inside `read32`. The bus range
/// comes from the installed window when there is one, because ARM firmware is
/// free to put a controller on a bus x86 never used.
pub fn for_each_function(mut f: impl FnMut(u8, u8, u8) -> bool) {
    let (lo, hi) = ecam_bus_range().unwrap_or((0, 3));
    for bus in lo..=hi {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                if read32(bus, slot, func, 0x00) == 0xFFFF_FFFF {
                    // Function 0 absent means the whole slot is absent.
                    if func == 0 {
                        break;
                    }
                    continue;
                }
                if f(bus, slot, func) {
                    return;
                }
                let header = (read32(bus, slot, func, 0x0C) >> 16) as u8;
                if func == 0 && header & 0x80 == 0 {
                    break;
                }
            }
        }
        if bus == hi {
            break;
        }
    }
}

/// First function matching a class triple.
pub fn find_class(base_class: u8, subclass: u8, prog_if: u8) -> Option<(u8, u8, u8)> {
    let mut hit = None;
    for_each_function(|b, s, f| {
        let class = read32(b, s, f, 0x08);
        if (class >> 24) as u8 == base_class
            && (class >> 16) as u8 == subclass
            && (class >> 8) as u8 == prog_if
        {
            hit = Some((b, s, f));
            return true;
        }
        false
    });
    hit
}

/// Tiny fixed vec so we don't need alloc — max 8 UHCI controllers.
pub mod heapless_vec {
    pub struct UhciList {
        data: [(u8, u8, u8, u16); 8],
        len: usize,
    }
    impl UhciList {
        pub const fn new() -> Self {
            Self {
                data: [(0, 0, 0, 0); 8],
                len: 0,
            }
        }
        pub fn push(&mut self, v: (u8, u8, u8, u16)) {
            if self.len < self.data.len() {
                self.data[self.len] = v;
                self.len += 1;
            }
        }
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }
        pub fn iter(&self) -> impl Iterator<Item = &(u8, u8, u8, u16)> {
            self.data[..self.len].iter()
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub use port::{inb, inw, outb, outw};


/// What USB host controllers this machine actually has.
///
/// The guest only drives UHCI, which is a QEMU-era controller. Real machines
/// built in the last fifteen years expose xHCI instead, so booting this on
/// hardware can leave the pointer dead with nothing on screen explaining why.
/// Counting the controllers lets the UI say "there is an xHCI here and I
/// cannot speak to it" rather than showing a cursor that never moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsbSurvey {
    pub uhci: u8,
    pub ohci: u8,
    pub ehci: u8,
    pub xhci: u8,
}

impl UsbSurvey {
    /// True when a controller exists that we have no driver for.
    ///
    /// OHCI came off this list when `ohci.rs` landed. Leaving it on would have
    /// the status line announce that the controller we just implemented cannot
    /// be spoken to — on the one machine where it is the only USB there is.
    pub fn has_unsupported(&self) -> bool {
        self.xhci > 0
    }

    /// Name the controller we found and cannot drive, for the status line.
    /// A function rather than a literal in `inputdiag`, so the next driver is
    /// a one-line edit here instead of a message that quietly goes stale.
    pub fn unsupported_name(&self) -> Option<&'static str> {
        (self.xhci > 0).then_some("xHCI")
    }

    pub fn none_at_all(&self) -> bool {
        *self == Self::default()
    }
}

/// USB controller `prog_if` values, from the PCI spec.
const PROG_IF_UHCI: u32 = 0x00;
const PROG_IF_OHCI: u32 = 0x10;
const PROG_IF_EHCI: u32 = 0x20;
const PROG_IF_XHCI: u32 = 0x30;

pub fn usb_survey() -> UsbSurvey {
    let mut out = UsbSurvey::default();
    // The sweep never breaks out early — an absent function is a `continue` —
    // so under ECAM it would walk straight off the end of the mapping. Clamp
    // to the buses the window actually covers; the port path keeps its old
    // 0..=255 range so x86 counts exactly what it counted before.
    let (lo, hi) = ecam_bus_range().unwrap_or((0, 255));
    for bus in lo..=hi {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let vendor = read16(bus, slot, func, 0x00);
                if vendor == 0xFFFF {
                    continue;
                }
                let class = read32(bus, slot, func, 0x08);
                if (class >> 24) & 0xFF != 0x0C || (class >> 16) & 0xFF != 0x03 {
                    continue;
                }
                let counter = match (class >> 8) & 0xFF {
                    PROG_IF_UHCI => &mut out.uhci,
                    PROG_IF_OHCI => &mut out.ohci,
                    PROG_IF_EHCI => &mut out.ehci,
                    PROG_IF_XHCI => &mut out.xhci,
                    _ => continue,
                };
                *counter = counter.saturating_add(1);
            }
        }
    }
    out
}

#[cfg(test)]
mod survey_tests {
    use super::*;

    #[test]
    fn a_machine_with_only_xhci_is_flagged_as_undrivable() {
        let s = UsbSurvey { xhci: 1, ..Default::default() };
        assert!(s.has_unsupported());
        assert!(!s.none_at_all());
    }

    #[test]
    fn the_controller_we_can_drive_is_not_flagged() {
        let s = UsbSurvey { uhci: 1, ohci: 1, ehci: 1, ..Default::default() };
        assert!(!s.has_unsupported(), "UHCI, OHCI and EHCI all have drivers");
        assert_eq!(s.unsupported_name(), None);
    }

    #[test]
    fn an_unsupported_controller_says_which_one_it_is() {
        let s = UsbSurvey { xhci: 1, ..Default::default() };
        assert_eq!(s.unsupported_name(), Some("xHCI"));
    }

    #[test]
    fn an_empty_survey_is_not_mistaken_for_a_working_bus() {
        assert!(UsbSurvey::default().none_at_all());
    }
}

#[cfg(test)]
mod ecam_tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    /// The window is global state, so the tests that install one take turns.
    static SERIAL: Mutex<()> = Mutex::new(());

    const BUSES: usize = 2;

    /// A config space we can point the real `read32` at.
    struct Fake {
        _guard: MutexGuard<'static, ()>,
        words: &'static mut [u32],
    }

    impl Fake {
        fn new() -> Self {
            let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
            clear_ecam();
            let words: &'static mut [u32] =
                std::vec![0xFFFF_FFFFu32; (BUSES << 20) / 4].leak();
            Self { _guard: guard, words }
        }

        fn base(&self) -> usize {
            self.words.as_ptr() as usize
        }

        /// Populate one function: vendor/device, class triple, and BAR0.
        fn device(&mut self, bus: u8, slot: u8, func: u8, class: u32, bar0: u32) {
            let off = ecam_offset(bus, slot, func, 0, 0, (BUSES - 1) as u8).expect("in window");
            self.words[off / 4] = 0x1234_5678;
            self.words[off / 4 + 1] = 0; // command | status
            self.words[off / 4 + 2] = class;
            self.words[off / 4 + 3] = 0; // header type 0, single function
            self.words[off / 4 + 4] = bar0;
        }

        fn install(&self) -> bool {
            use_ecam(self.base(), 0, (BUSES - 1) as u8)
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            clear_ecam();
        }
    }

    #[test]
    fn a_bus_outside_the_window_is_refused_before_it_is_dereferenced() {
        // usb_survey sweeps buses without ever breaking out. On aarch64 one
        // read past the mapping is an abort with no handler, so this bounds
        // check is the feature, not a guard.
        assert!(ecam_offset(0, 0, 0, 0, 0, 15).is_some());
        assert!(ecam_offset(15, 31, 7, 0xFC, 0, 15).is_some());
        assert_eq!(ecam_offset(16, 0, 0, 0, 0, 15), None);
        assert_eq!(ecam_offset(3, 0, 0, 0, 4, 15), None, "below the window too");
    }

    #[test]
    fn a_window_that_does_not_start_at_bus_zero_is_addressed_from_its_own_start() {
        // QEMU may hand out buses 0..15 and firmware a slice starting higher;
        // biasing from bus 0 there reads a megabyte past the end.
        assert_eq!(ecam_offset(4, 0, 0, 0, 4, 7), Some(0));
        assert_eq!(ecam_offset(5, 0, 0, 0, 4, 7), Some(1 << 20));
    }

    #[test]
    fn config_offsets_are_dword_aligned_so_a_byte_offset_cannot_fault() {
        // read16/write16 pass raw byte offsets straight through.
        assert_eq!(ecam_offset(0, 0, 0, 0x06, 0, 0), Some(0x04));
        assert_eq!(ecam_offset(0, 0, 0, 0x0F, 0, 0), Some(0x0C));
    }

    #[test]
    fn the_scanners_find_a_controller_through_a_config_window() {
        let mut fake = Fake::new();
        fake.device(0, 0, 0, 0x0600_0000, 0); // host bridge
        // OHCI: class 0x0C, subclass 0x03, prog-if 0x10, 32-bit memory BAR.
        fake.device(1, 4, 0, 0x0C03_1000, 0xFEBF_0000);
        assert!(fake.install(), "a window that answers must be accepted");

        assert!(ecam_active());
        assert_eq!(ecam_bus_range(), Some((0, (BUSES - 1) as u8)));
        assert_eq!(find_class(0x0C, 0x03, 0x10), Some((1, 4, 0)));
        assert_eq!(find_class(0x0C, 0x03, 0x30), None, "there is no xHCI here");
        assert_eq!(read16(1, 4, 0, 0x00), 0x5678, "vendor id, low half");
        assert_eq!(read16(1, 4, 0, 0x02), 0x1234, "device id, high half");
    }

    #[test]
    fn the_survey_counts_only_the_buses_the_window_covers() {
        let mut fake = Fake::new();
        fake.device(0, 1, 0, 0x0C03_1000, 0);
        fake.device(1, 2, 0, 0x0C03_3000, 0);
        fake.device(0, 3, 0, 0x0106_0000, 0); // SATA, not USB
        assert!(fake.install());

        let s = usb_survey();
        assert_eq!((s.ohci, s.xhci, s.uhci, s.ehci), (1, 1, 0, 0));
        assert!(s.has_unsupported(), "the xHCI is still undrivable");
    }

    #[test]
    fn enabling_bus_mastering_writes_through_and_leaves_status_alone() {
        let mut fake = Fake::new();
        fake.device(0, 1, 0, 0x0C03_1000, 0xFEBF_0000);
        assert!(fake.install());

        // Status bits are RW1C: writing back what we read would clear them.
        let idx = ecam_offset(0, 1, 0, 0x04, 0, 1).unwrap() / 4;
        fake.words[idx] = 0xFFFF_0000;
        let cmd = read16(0, 1, 0, 0x04);
        write16(0, 1, 0, 0x04, cmd | 0x0406);
        assert_eq!(read16(0, 1, 0, 0x04), 0x0406, "memory | bus master | intx off");
        assert_eq!(read16(0, 1, 0, 0x06), 0, "status must not be written back as 1s");
    }

    #[test]
    fn a_window_where_nothing_answers_is_rejected_rather_than_used() {
        let fake = Fake::new(); // every dword left at 0xFFFFFFFF
        assert!(!fake.install(), "a wrong base must degrade to no PCI");
        assert!(!ecam_active(), "and must not stay installed");
    }
}
