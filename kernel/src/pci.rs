//! Tiny PCI config-space helpers (port I/O).

use crate::port;

const CONFIG_ADDR: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

fn cfg_addr(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

fn read32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
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

/// Read the PCI command register (config offset 0x04).
pub(crate) fn read_cmd(bus: u8, slot: u8, func: u8) -> u16 {
    (read32(bus, slot, func, 0x04) & 0xFFFF) as u16
}

/// Write the PCI command register. The adjacent status half (0x06) is RW1C —
/// writing back the 1s we just read would clear them — so the high word is
/// always written as 0 (a no-op for RW1C bits).
pub(crate) fn write_cmd(bus: u8, slot: u8, func: u8, val: u16) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        port::outl(CONFIG_ADDR, cfg_addr(bus, slot, func, 0x04));
        port::outl(CONFIG_DATA, val as u32);
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = (bus, slot, func, val);
}

/// Walk present PCI functions (buses 0..4).
fn for_each_fn(mut visit: impl FnMut(u8, u8, u8)) {
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
                visit(bus, slot, func);
                let header = (read32(bus, slot, func, 0x0C) >> 16) as u8;
                if func == 0 && header & 0x80 == 0 {
                    break;
                }
            }
        }
    }
}

/// USB serial-bus controllers: class 0x0C / subclass 0x03 → prog-if.
fn usb_prog_if(bus: u8, slot: u8, func: u8) -> Option<u8> {
    let class = read32(bus, slot, func, 0x08);
    if (class >> 24) & 0xFF == 0x0C && (class >> 16) & 0xFF == 0x03 {
        Some(((class >> 8) & 0xFF) as u8)
    } else {
        None
    }
}

/// UHCI = class 0x0C, subclass 0x03, prog-if 0x00. Returns I/O bases only.
pub(crate) fn find_all_uhci() -> heapless_vec::UhciList {
    let mut out = heapless_vec::UhciList::new();
    for_each_fn(|bus, slot, func| {
        if usb_prog_if(bus, slot, func) != Some(0x00) {
            return;
        }
        let bar4 = read32(bus, slot, func, 0x20);
        if bar4 & 1 == 1 {
            let io = (bar4 & 0xFFE0) as u16;
            let cmd = read_cmd(bus, slot, func);
            write_cmd(bus, slot, func, cmd | 0x05);
            out.push(io);
        }
    });
    out
}

/// EHCI = class 0x0C, subclass 0x03, prog-if 0x20.
///
/// q35 with `-usb` builds an ICH9 set at 00:1d.x, and UTM adds a *second*
/// explicit `ich9-usb-ehci1`. Disabling only the first leaves the other still
/// owning its ports, so its UHCI companions see nothing.
pub(crate) fn for_each_ehci(mut f: impl FnMut(u8, u8, u8)) {
    for_each_fn(|bus, slot, func| {
        if usb_prog_if(bus, slot, func) == Some(0x20) {
            f(bus, slot, func);
        }
    });
}

/// Tiny fixed vec so we don't need alloc — max 8 UHCI I/O bases.
pub(crate) mod heapless_vec {
    pub struct UhciList {
        data: [u16; 8],
        len: usize,
    }
    impl UhciList {
        pub const fn new() -> Self {
            Self {
                data: [0; 8],
                len: 0,
            }
        }
        pub fn push(&mut self, io: u16) {
            if self.len < self.data.len() {
                self.data[self.len] = io;
                self.len += 1;
            }
        }
        pub fn is_empty(&self) -> bool {
            self.len == 0
        }
        pub fn iter(&self) -> impl Iterator<Item = &u16> {
            self.data[..self.len].iter()
        }
    }
}

