//! Minimal UHCI + QEMU usb-tablet (absolute HID) for UTM/SPICE.
//!
//! UTM always attaches `-device usb-tablet`, which overrides PS/2. Without this
//! driver the guest pointer never moves under Spice.

use crate::pci;

const USBCMD: u16 = 0x00;
const USBSTS: u16 = 0x02;
const FRNUM: u16 = 0x06;
const FLBASEADD: u16 = 0x08;
const SOFMOD: u16 = 0x0C;
const PORTSC1: u16 = 0x10;

const TD_ACTIVE: u32 = 1 << 23;
const TD_IOC: u32 = 1 << 24;
const TD_SPD: u32 = 1 << 29;

const TOKEN_SETUP: u32 = 0x2D;
const TOKEN_IN: u32 = 0x69;
const TOKEN_OUT: u32 = 0xE1;

#[repr(C, align(16))]
struct Td {
    link: u32,
    status: u32,
    token: u32,
    buffer: u32,
}

#[repr(C, align(16))]
struct Qh {
    head_link: u32,
    element: u32,
}

/// DMA arena: frame list (4KiB) + scratch (4KiB).
struct Dma {
    fl_phys: u32,
    fl: *mut u32,
    scratch_phys: u32,
    scratch: *mut u8,
}

pub struct UsbTablet {
    io: u16,
    dma: Dma,
    addr: u8,
    data_toggle: bool,
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    ready: bool,
}

fn delay(spins: u32) {
    for _ in 0..spins {
        core::hint::spin_loop();
    }
}

impl UsbTablet {
    /// Probe UHCI, reset port, configure tablet @ addr 1, return driver.
    pub unsafe fn init(hhdm: u64, phys_page0: u64, phys_page1: u64) -> Option<Self> {
        let (_b, _s, _f, io) = pci::find_uhci()?;

        let fl = (phys_page0 + hhdm) as *mut u32;
        let scratch = (phys_page1 + hhdm) as *mut u8;
        // Clear
        unsafe {
            for i in 0..1024 {
                fl.add(i).write_volatile(1); // terminate
            }
            for i in 0..4096 {
                scratch.add(i).write_volatile(0);
            }
        }

        let mut me = Self {
            io,
            dma: Dma {
                fl_phys: phys_page0 as u32,
                fl,
                scratch_phys: phys_page1 as u32,
                scratch,
            },
            addr: 0,
            data_toggle: false,
            x: 0,
            y: 0,
            buttons: 0,
            ready: false,
        };

        me.hc_reset()?;
        me.port_enable()?;
        // Device starts at address 0
        me.addr = 0;
        me.set_address(1)?;
        me.addr = 1;
        me.set_configuration(1)?;
        me.ready = true;
        Some(me)
    }

    fn outw(&self, off: u16, val: u16) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            pci::outw(self.io + off, val);
        }
        #[cfg(not(target_arch = "x86_64"))]
        let _ = (off, val);
    }

    fn inw(&self, off: u16) -> u16 {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            pci::inw(self.io + off)
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            let _ = off;
            0
        }
    }

    fn outl_flbase(&self, val: u32) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            // FLBASEADD is 32-bit at io+8 — use two outw or outl via pci helper
            let port = self.io + FLBASEADD;
            core::arch::asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack, preserves_flags));
        }
        #[cfg(not(target_arch = "x86_64"))]
        let _ = val;
    }

    fn hc_reset(&mut self) -> Option<()> {
        // Host controller reset
        self.outw(USBCMD, 0x0002);
        for _ in 0..100_000 {
            if self.inw(USBCMD) & 0x0002 == 0 {
                break;
            }
            delay(50);
        }
        self.outw(USBSTS, 0xFFFF); // clear status
        self.outw(SOFMOD, 64);
        self.outl_flbase(self.dma.fl_phys);
        self.outw(FRNUM, 0);
        // Run + max packet 64
        self.outw(USBCMD, 0x0001 | 0x0080);
        delay(10_000);
        Some(())
    }

    fn port_enable(&mut self) -> Option<()> {
        // Try ports 0 and 1 (PORTSC at +0x10 and +0x12)
        for port in 0..2u16 {
            let off = PORTSC1 + port * 2;
            let mut sc = self.inw(off);
            if sc & 1 == 0 {
                continue; // no device
            }
            // Port reset
            self.outw(off, sc | 0x0200);
            delay(200_000);
            sc = self.inw(off);
            self.outw(off, sc & !0x0200);
            delay(50_000);
            sc = self.inw(off);
            // Enable port
            self.outw(off, (sc & !0x000A) | 0x0004);
            delay(50_000);
            if self.inw(off) & 0x0004 != 0 {
                return Some(());
            }
        }
        None
    }

    fn scratch_offset(&self, off: usize) -> (*mut u8, u32) {
        unsafe {
            (
                self.dma.scratch.add(off),
                self.dma.scratch_phys + off as u32,
            )
        }
    }

    fn wait_td(&self, td: *mut Td, spins: u32) -> bool {
        for _ in 0..spins {
            let st = unsafe { core::ptr::addr_of!((*td).status).read_volatile() };
            if st & TD_ACTIVE == 0 {
                // bit 22 = stalled, 21 = buffer error, etc.
                return st & (1 << 22) == 0;
            }
            delay(20);
        }
        false
    }

    fn control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
    ) -> Option<()> {
        // Layout in scratch:
        // 0x00: setup packet (8)
        // 0x10: qh
        // 0x20: td_setup
        // 0x30: td_data (optional)
        // 0x40: td_status
        // 0x50: data buffer
        let (setup_v, setup_p) = self.scratch_offset(0x00);
        let (qh_v, qh_p) = self.scratch_offset(0x10);
        let (td0_v, td0_p) = self.scratch_offset(0x20);
        let (td1_v, td1_p) = self.scratch_offset(0x30);
        let (td2_v, td2_p) = self.scratch_offset(0x40);
        let (buf_v, buf_p) = self.scratch_offset(0x50);

        unsafe {
            setup_v.add(0).write_volatile(request_type);
            setup_v.add(1).write_volatile(request);
            setup_v.add(2).write_volatile((value & 0xFF) as u8);
            setup_v.add(3).write_volatile((value >> 8) as u8);
            setup_v.add(4).write_volatile((index & 0xFF) as u8);
            setup_v.add(5).write_volatile((index >> 8) as u8);
            let n = data.len() as u16;
            setup_v.add(6).write_volatile((n & 0xFF) as u8);
            setup_v.add(7).write_volatile((n >> 8) as u8);

            for (i, b) in data.iter().enumerate() {
                buf_v.add(i).write_volatile(*b);
            }
        }

        let addr = self.addr as u32;
        let setup_token = (7 << 21) | (addr << 8) | TOKEN_SETUP; // 8 bytes - 1 = 7
        // DATA1 for status/data toggles: bit 19

        let td0 = td0_v as *mut Td;
        let td1 = td1_v as *mut Td;
        let td2 = td2_v as *mut Td;
        let qh = qh_v as *mut Qh;

        let has_data = !data.is_empty();
        let data_is_in = request_type & 0x80 != 0;

        unsafe {
            // SETUP TD
            td0.write(Td {
                link: if has_data { td1_p | 0x4 } else { td2_p | 0x4 }, // depth first
                status: TD_ACTIVE | (3 << 27), // 3 errors
                token: setup_token,
                buffer: setup_p,
            });

            if has_data {
                let len = data.len() as u32;
                let tok = if data_is_in { TOKEN_IN } else { TOKEN_OUT };
                let token = ((len - 1) << 21) | (1 << 19) | (addr << 8) | tok; // DATA1
                td1.write(Td {
                    link: td2_p | 0x4,
                    status: TD_ACTIVE | (3 << 27) | if data_is_in { TD_SPD } else { 0 },
                    token,
                    buffer: buf_p,
                });
            }

            // STATUS TD: opposite direction, DATA1, 0 length -> maxlen 0x7FF encoding is length-1 with 0 bytes => 0x7FF?
            // UHCI: Maximum Length field is length-1; 0-byte packet uses 0x7FF
            let stok = if data_is_in || !has_data {
                TOKEN_OUT
            } else {
                TOKEN_IN
            };
            // For no-data control (SET_ADDRESS): status is IN
            let stok = if !has_data { TOKEN_IN } else { stok };
            let status_token = (0x7FF << 21) | (1 << 19) | (addr << 8) | stok;
            td2.write(Td {
                link: 1, // terminate
                status: TD_ACTIVE | TD_IOC | (3 << 27),
                token: status_token,
                buffer: 0,
            });

            qh.write(Qh {
                head_link: 1, // terminate horizontal
                element: td0_p,
            });

            // Point all frames at this QH (select execute)
            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(qh_p | 0x2); // QH bit
            }
        }

        let wait_td = if has_data { td2 } else { td2 };
        if !self.wait_td(wait_td, 500_000) {
            // also check setup
            if !self.wait_td(td0, 10_000) {
                return None;
            }
        }

        if has_data && data_is_in {
            unsafe {
                for i in 0..data.len() {
                    data[i] = buf_v.add(i).read_volatile();
                }
            }
        }

        // Detach schedule
        unsafe {
            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(1);
            }
        }
        Some(())
    }

    fn set_address(&mut self, new_addr: u8) -> Option<()> {
        let mut empty: [u8; 0] = [];
        self.control(0x00, 0x05, new_addr as u16, 0, &mut empty)
    }

    fn set_configuration(&mut self, cfg: u8) -> Option<()> {
        let mut empty: [u8; 0] = [];
        self.control(0x00, 0x09, cfg as u16, 0, &mut empty)
    }

    /// Poll interrupt IN on endpoint 1; update absolute x/y (0..32767).
    pub fn poll(&mut self, screen_w: i32, screen_h: i32) -> bool {
        if !self.ready {
            return false;
        }

        let (qh_v, qh_p) = self.scratch_offset(0x100);
        let (td_v, td_p) = self.scratch_offset(0x120);
        let (buf_v, buf_p) = self.scratch_offset(0x140);

        let addr = self.addr as u32;
        let toggle = if self.data_toggle { 1u32 << 19 } else { 0 };
        // EP1, max 8 bytes -> length-1 = 7
        let token = (7 << 21) | toggle | (1 << 15) | (addr << 8) | TOKEN_IN;

        unsafe {
            let td = td_v as *mut Td;
            let qh = qh_v as *mut Qh;
            td.write(Td {
                link: 1,
                status: TD_ACTIVE | (3 << 27) | TD_SPD,
                token,
                buffer: buf_p,
            });
            qh.write(Qh {
                head_link: 1,
                element: td_p,
            });
            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(qh_p | 0x2);
            }
        }

        let td = td_v as *mut Td;
        if !self.wait_td(td, 50_000) {
            unsafe {
                for i in 0..1024 {
                    self.dma.fl.add(i).write_volatile(1);
                }
            }
            return false;
        }

        let st = unsafe { core::ptr::addr_of!((*td).status).read_volatile() };
        unsafe {
            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(1);
            }
        }
        // Still active / stalled / NAK / timeout → no update
        if st & TD_ACTIVE != 0
            || st & (1 << 22) != 0
            || st & (1 << 19) != 0
            || st & (1 << 18) != 0
        {
            return false;
        }
        self.data_toggle = !self.data_toggle;

        // ActLen is bits 0-10; value is (length - 1), or 0x7FF if zero length.
        let act = (st & 0x7FF) as usize;
        let n = if act == 0x7FF { 0 } else { act + 1 };
        if n < 6 {
            return false;
        }

        let mut report = [0u8; 8];
        unsafe {
            for i in 0..n.min(8) {
                report[i] = buf_v.add(i).read_volatile();
            }
        }

        let buttons = report[0] & 0x07;
        let ax = u16::from_le_bytes([report[1], report[2]]) as i32;
        let ay = u16::from_le_bytes([report[3], report[4]]) as i32;
        let nx = (ax * (screen_w - 1)) / 32767;
        let ny = (ay * (screen_h - 1)) / 32767;
        let moved = nx != self.x || ny != self.y || buttons != self.buttons;
        self.x = nx.clamp(0, screen_w.saturating_sub(1));
        self.y = ny.clamp(0, screen_h.saturating_sub(1));
        self.buttons = buttons;
        moved
    }
}

/// Pick two usable 4KiB pages below 4GiB from the Limine memory map.
pub fn alloc_dma_pages(mmap: &limine::response::MemoryMapResponse) -> Option<(u64, u64)> {
    let mut pages = [0u64; 2];
    let mut n = 0;
    for entry in mmap.entries() {
        if entry.entry_type != limine::memory_map::EntryType::USABLE {
            continue;
        }
        let mut base = (entry.base + 0xFFF) & !0xFFF;
        let end = entry.base + entry.length;
        while base + 4096 <= end && n < 2 {
            // Skip page 0; stay under 4GiB for UHCI 32-bit pointers.
            if base >= 0x10000 && base < 0x1_0000_0000 {
                pages[n] = base;
                n += 1;
            }
            base += 4096;
        }
        if n == 2 {
            return Some((pages[0], pages[1]));
        }
    }
    None
}
