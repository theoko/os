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
    write32(bus, slot, func, aligned, v);
}

/// UHCI = class 0x0C, subclass 0x03, prog-if 0x00.
pub fn find_uhci() -> Option<(u8, u8, u8, u16)> {
    for bus in 0..=0u8 {
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
                        // Enable I/O + bus master
                        let cmd = read16(bus, slot, func, 0x04);
                        write16(bus, slot, func, 0x04, cmd | 0x05);
                        return Some((bus, slot, func, io));
                    }
                }
                let header = (read32(bus, slot, func, 0x0C) >> 16) as u8;
                if func == 0 && header & 0x80 == 0 {
                    break;
                }
            }
        }
    }
    // Also scan bus 0 more carefully already done; try buses 0..3 for q35
    for bus in 1..4u8 {
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
                        return Some((bus, slot, func, io));
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_arch = "x86_64")]
pub use port::{inb, inw, outb, outw};
