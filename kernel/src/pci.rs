//! Tiny PCI config-space helpers (port I/O).

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

fn cfg_addr(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

pub fn read32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outl(CONFIG_ADDR, cfg_addr(bus, slot, func, offset));
        port::inl(CONFIG_DATA)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = (bus, slot, func, offset);
        0xFFFF_FFFF
    }
}

pub fn write32(bus: u8, slot: u8, func: u8, offset: u8, val: u32) {
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
    pub fn has_unsupported(&self) -> bool {
        self.xhci > 0 || self.ohci > 0
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
    for bus in 0..=255u16 {
        for slot in 0..32u8 {
            for func in 0..8u8 {
                let vendor = read16(bus as u8, slot, func, 0x00);
                if vendor == 0xFFFF {
                    continue;
                }
                let class = read32(bus as u8, slot, func, 0x08);
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
        let s = UsbSurvey { uhci: 1, ehci: 1, ..Default::default() };
        assert!(!s.has_unsupported(), "UHCI and EHCI are both handled");
    }

    #[test]
    fn an_empty_survey_is_not_mistaken_for_a_working_bus() {
        assert!(UsbSurvey::default().none_at_all());
    }
}
