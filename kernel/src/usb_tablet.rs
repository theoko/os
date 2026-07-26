//! Minimal UHCI + QEMU usb-tablet (absolute HID) for UTM/SPICE.
//!
//! On q35 the tablet lands on **EHCI** (`usb-bus.0`). Full-speed handoff to the
//! ICH9 UHCI companions only happens after EHCI releases the port — we halt
//! EHCI first, then talk UHCI.

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
    /// Interrupt IN endpoint number (low 4 bits of bEndpointAddress).
    ep: u8,
    /// Max packet size for the interrupt endpoint (minus one goes in the TD).
    max_packet: u8,
    data_toggle: bool,
    x: i32,
    y: i32,
    buttons: u8,
    ready: bool,
    screen_w: i32,
    screen_h: i32,
    /// Interrupt IN TD is armed in the frame list; poll only checks completion.
    outstanding: bool,
}

fn delay(spins: u32) {
    for _ in 0..spins {
        core::hint::spin_loop();
    }
}

/// Disable *every* EHCI via PCI command (no MMIO) so companion UHCIs own the
/// ports. There is more than one controller under UTM, and leaving either
/// enabled strands the devices behind it.
fn disable_ehci_pci() -> u32 {
    let mut n = 0;
    pci::for_each_ehci(|b, s, f| {
        let cmd = pci::read16(b, s, f, 0x04);
        pci::write16(b, s, f, 0x04, cmd & !0x06); // clear Mem Space + Bus Master
        n += 1;
    });
    n
}

/// Outcome of probing one (controller, port) pair.
enum Probe {
    /// A verified tablet, configured and ready.
    Bound(UsbTablet),
    /// Nothing connected on this port.
    Empty,
    /// A device enumerated but is not an absolute pointer (with reason).
    NotTablet(&'static str),
}

impl UsbTablet {
    /// Probe every UHCI controller and every port, binding only a device that
    /// identifies as an absolute tablet. `err` gets a short ASCII reason.
    pub unsafe fn init(hhdm: u64, phys_page0: u64, phys_page1: u64, err: &mut [u8]) -> Option<Self> {
        set_err(err, "start");
        let _ = disable_ehci_pci();

        let controllers = pci::find_all_uhci();
        if controllers.is_empty() {
            set_err(err, "no-uhci");
            return None;
        }

        let mut saw_device = false;
        let mut reason = "not-tablet";
        for &(_b, _s, _f, io) in controllers.iter() {
            for port in 0..2u16 {
                match unsafe { Self::init_on(io, port, hhdm, phys_page0, phys_page1) } {
                    Probe::Bound(t) => {
                        set_err(err, "ok");
                        return Some(t);
                    }
                    // Something enumerated but was a keyboard / redirect stub.
                    Probe::NotTablet(r) => {
                        saw_device = true;
                        reason = r;
                    }
                    Probe::Empty => {}
                }
            }
        }
        set_err(err, if saw_device { reason } else { "no-port" });
        None
    }

    unsafe fn init_on(
        io: u16,
        port: u16,
        hhdm: u64,
        phys_page0: u64,
        phys_page1: u64,
    ) -> Probe {
        let fl = (phys_page0 + hhdm) as *mut u32;
        let scratch = (phys_page1 + hhdm) as *mut u8;
        unsafe {
            for i in 0..1024 {
                fl.add(i).write_volatile(1);
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
            ep: 1,
            max_packet: 8,
            data_toggle: false,
            // -1 so the first absolute report always counts as motion.
            x: -1,
            y: -1,
            buttons: 0,
            ready: false,
            screen_w: 0,
            screen_h: 0,
            outstanding: false,
        };

        me.hc_reset();
        if !me.port_enable(port) {
            return Probe::Empty;
        }
        // Port reset returns the device to the default address.
        me.addr = 0;
        if me.set_address(1).is_none() {
            return Probe::NotTablet("set-addr");
        }
        me.addr = 1;
        if let Err(r) = me.identify() {
            return Probe::NotTablet(r);
        }
        if me.set_configuration(1).is_none() {
            return Probe::NotTablet("set-cfg");
        }
        // HID: prefer Report protocol; ignore failures (some firmwares NAK).
        let _ = me.hid_set_idle();
        let _ = me.hid_set_protocol(1);
        me.ready = true;
        Probe::Bound(me)
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
        unsafe {
            crate::port::outl(self.io + FLBASEADD, val);
        }
    }

    fn hc_reset(&mut self) {
        self.outw(USBCMD, 0x0002);
        for _ in 0..100_000 {
            if self.inw(USBCMD) & 0x0002 == 0 {
                break;
            }
            delay(50);
        }
        self.outw(USBINTR_ZERO, 0); // defined below as 0x04 — disable IRQs
        self.outw(USBSTS, 0xFFFF);
        self.outw(SOFMOD, 64);
        self.outl_flbase(self.dma.fl_phys);
        self.outw(FRNUM, 0);
        self.outw(USBCMD, 0x0001 | 0x0080); // RS | MaxPacket
        delay(20_000);
    }

    /// Reset and enable a single port. False if nothing is connected there.
    fn port_enable(&mut self, port: u16) -> bool {
        let off = PORTSC1 + port * 2;
        let mut sc = self.inw(off);
        // CSC clear, check CCS
        if sc & 0x02 != 0 {
            self.outw(off, sc | 0x02); // write-1-to-clear CSC
            sc = self.inw(off);
        }
        if sc & 1 == 0 {
            return false;
        }
        // Reset
        self.outw(off, (sc & !0x000A) | 0x0200);
        delay(500_000);
        sc = self.inw(off);
        self.outw(off, sc & !0x0200);
        delay(100_000);
        sc = self.inw(off);
        // Enable + clear status change bits
        self.outw(off, (sc & !0x000A) | 0x0004 | 0x000A);
        delay(100_000);
        self.inw(off) & 0x0004 != 0
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
                return st & (1 << 22) == 0; // not stalled
            }
            delay(20);
        }
        false
    }

    /// Wait for FRNUM to advance (≤1 ms) after unlinking the frame list, so
    /// the HC cannot still be executing a QH/TD we are about to rewrite.
    /// Bounded in case the controller is halted and FRNUM is frozen.
    fn wait_frame_tick(&self) {
        let start = self.inw(FRNUM);
        for _ in 0..200_000 {
            if self.inw(FRNUM) != start {
                return;
            }
            core::hint::spin_loop();
        }
    }

    fn control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
    ) -> Option<()> {
        // Scratch layout is fixed so the control and interrupt paths never
        // overlap: 0x100..0x160 belongs to poll_once().
        const DATA_TD_BASE: usize = 0x200;
        const DATA_TD_STRIDE: usize = 0x20;
        const DATA_BUF: usize = 0x600;
        // Control endpoint max packet for a full-speed QEMU HID device.
        const MPS: usize = 8;

        if data.len() > 256 {
            return None;
        }

        let (setup_v, setup_p) = self.scratch_offset(0x00);
        let (qh_v, qh_p) = self.scratch_offset(0x10);
        let (td0_v, td0_p) = self.scratch_offset(0x30);
        let (td2_v, td2_p) = self.scratch_offset(0x40);
        let (buf_v, buf_p) = self.scratch_offset(DATA_BUF);

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
        let setup_token = (7 << 21) | (addr << 8) | TOKEN_SETUP;
        let has_data = !data.is_empty();
        let data_is_in = request_type & 0x80 != 0;

        let td0 = td0_v as *mut Td;
        let td2 = td2_v as *mut Td;
        let qh = qh_v as *mut Qh;

        // Split the data stage into max-packet-sized TDs. A single TD carrying
        // more than bMaxPacketSize0 babbles — which is why an 18-byte device
        // descriptor could never be read before.
        let n_data = data.len().div_ceil(MPS);

        unsafe {
            let first_data = if has_data {
                (self.dma.scratch_phys + DATA_TD_BASE as u32) | 0x4
            } else {
                td2_p | 0x4
            };
            td0.write_volatile(Td {
                link: first_data,
                status: TD_ACTIVE | (3 << 27),
                token: setup_token,
                buffer: setup_p,
            });

            for i in 0..n_data {
                let (tdv, _tdp) = self.scratch_offset(DATA_TD_BASE + i * DATA_TD_STRIDE);
                let next = if i + 1 == n_data {
                    td2_p | 0x4
                } else {
                    (self.dma.scratch_phys + (DATA_TD_BASE + (i + 1) * DATA_TD_STRIDE) as u32) | 0x4
                };
                let off = i * MPS;
                let len = (data.len() - off).min(MPS) as u32;
                let tok = if data_is_in { TOKEN_IN } else { TOKEN_OUT };
                // DATA1 on the first data packet, alternating thereafter.
                let toggle = if i % 2 == 0 { 1u32 << 19 } else { 0 };
                let token = ((len - 1) << 21) | toggle | (addr << 8) | tok;
                (tdv as *mut Td).write_volatile(Td {
                    link: next,
                    status: TD_ACTIVE | (3 << 27),
                    token,
                    buffer: buf_p + off as u32,
                });
            }

            let stok = if !has_data {
                TOKEN_IN
            } else if data_is_in {
                TOKEN_OUT
            } else {
                TOKEN_IN
            };
            let status_token = (0x7FF << 21) | (1 << 19) | (addr << 8) | stok;
            td2.write_volatile(Td {
                link: 1,
                status: TD_ACTIVE | TD_IOC | (3 << 27),
                token: status_token,
                buffer: 0,
            });

            qh.write_volatile(Qh {
                head_link: 1,
                element: td0_p,
            });

            // The HC reads these structures via DMA: make sure every TD/QH
            // store above is complete before the frame list publishes them.
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);

            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(qh_p | 0x2);
            }
        }

        if !self.wait_td(td2, 1_000_000) {
            unsafe {
                for i in 0..1024 {
                    self.dma.fl.add(i).write_volatile(1);
                }
            }
            // The HC may have fetched the frame pointer just before the
            // unlink; let the current frame drain before scratch is reused.
            self.wait_frame_tick();
            return None;
        }

        if has_data && data_is_in {
            unsafe {
                for i in 0..data.len() {
                    data[i] = buf_v.add(i).read_volatile();
                }
            }
        }

        unsafe {
            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(1);
            }
        }
        self.wait_frame_tick();
        Some(())
    }

    /// GET_DESCRIPTOR(type, index) into `buf`, requesting exactly `buf.len()`.
    fn get_descriptor(&mut self, desc_type: u8, index: u8, buf: &mut [u8]) -> Option<()> {
        self.control(0x80, 0x06, ((desc_type as u16) << 8) | index as u16, 0, buf)
    }

    /// Is the device on this port an absolute pointer we can drive?
    ///
    /// UTM populates the bus with keyboards and usb-redir stubs, so binding the
    /// first device that enumerates picks up the wrong one and then reports
    /// "ready" while never producing motion. QEMU's HID cuts are distinguished
    /// by the interface descriptor:
    ///   usb-kbd     subclass 1 (boot), protocol 1 (keyboard)
    ///   usb-mouse   subclass 1 (boot), protocol 2 (relative mouse)
    ///   usb-tablet  subclass 0,        protocol 0  <- absolute, what we want
    fn identify(&mut self) -> Result<(), &'static str> {
        let mut dev = [0u8; 18];
        if self.get_descriptor(1, 0, &mut dev).is_none() {
            return Err("dev-desc");
        }
        // bDescriptorType must be DEVICE, and the device class must be 0 so the
        // interface descriptor is the one that carries the HID class.
        if dev[1] != 0x01 {
            return Err("bad-dev");
        }

        // Config descriptor header first, to learn wTotalLength.
        let mut head = [0u8; 9];
        if self.get_descriptor(2, 0, &mut head).is_none() || head[1] != 0x02 {
            return Err("cfg-hdr");
        }
        let total = u16::from_le_bytes([head[2], head[3]]) as usize;
        if total < 9 || total > 128 {
            return Err("cfg-len");
        }
        let mut cfg = [0u8; 128];
        if self.get_descriptor(2, 0, &mut cfg[..total]).is_none() {
            return Err("cfg-body");
        }

        // Walk looking for HID tablet INTERFACE, then its IN ENDPOINT.
        let mut i = 0usize;
        let mut found_iface = false;
        while i + 1 < total {
            let len = cfg[i] as usize;
            if len < 2 || i + len > total {
                break;
            }
            let dtype = cfg[i + 1];
            if dtype == 0x04 && len >= 9 {
                let class = cfg[i + 5];
                let subclass = cfg[i + 6];
                let protocol = cfg[i + 7];
                found_iface = class == 0x03 && subclass == 0x00 && protocol == 0x00;
            } else if found_iface && dtype == 0x05 && len >= 7 {
                let addr = cfg[i + 2];
                if addr & 0x80 != 0 {
                    self.ep = addr & 0x0F;
                    let mps = u16::from_le_bytes([cfg[i + 4], cfg[i + 5]]).min(64) as u8;
                    self.max_packet = if mps == 0 { 8 } else { mps };
                    return Ok(());
                }
            }
            i += len;
        }
        if found_iface {
            // Interface matched but no endpoint — still bind with defaults.
            return Ok(());
        }
        Err("not-hid")
    }

    fn set_address(&mut self, new_addr: u8) -> Option<()> {
        let mut empty: [u8; 0] = [];
        self.control(0x00, 0x05, new_addr as u16, 0, &mut empty)
    }

    fn set_configuration(&mut self, cfg: u8) -> Option<()> {
        let mut empty: [u8; 0] = [];
        self.control(0x00, 0x09, cfg as u16, 0, &mut empty)
    }

    fn hid_set_idle(&mut self) -> Option<()> {
        let mut empty: [u8; 0] = [];
        self.control(0x21, 0x0A, 0, 0, &mut empty)
    }

    fn hid_set_protocol(&mut self, protocol: u16) -> Option<()> {
        let mut empty: [u8; 0] = [];
        self.control(0x21, 0x0B, protocol, 0, &mut empty)
    }

    /// Record framebuffer size used to scale absolute reports.
    pub fn bind_screen(&mut self, w: i32, h: i32) {
        self.screen_w = w;
        self.screen_h = h;
    }

    pub fn poll(&mut self, mice: &mut crate::mouse::Mouse) -> bool {
        if !self.ready {
            return false;
        }
        // Non-blocking: arm an interrupt IN once, then only check Active.
        // Waiting out the full bInterval on every NAK froze the UI and also
        // tore down the schedule before the HC could retire the TD.
        if !self.outstanding {
            self.arm_interrupt_in();
            self.outstanding = true;
            return false;
        }

        let (td_v, _) = self.scratch_offset(0x120);
        let td = td_v as *mut Td;
        let st = unsafe { core::ptr::addr_of!((*td).status).read_volatile() };
        if st & TD_ACTIVE != 0 {
            return false;
        }

        // TD retired — unlink from the frame list before touching toggle/state.
        unsafe {
            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(1);
            }
        }
        // The HC may have fetched this frame's pointer pre-unlink; let the
        // frame drain before the next arm_interrupt_in rewrites the QH/TD.
        self.wait_frame_tick();
        self.outstanding = false;

        // Stalled / babble / CRC / buffer error → drop, do not flip toggle.
        // NAK keeps Active set on UHCI, so we never reach here for a NAK.
        if st & (1 << 22) != 0
            || st & (1 << 20) != 0
            || st & (1 << 18) != 0
            || st & (1 << 21) != 0
        {
            return false;
        }

        self.data_toggle = !self.data_toggle;

        let (buf_v, _) = self.scratch_offset(0x140);
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
        let ax = ax.clamp(0, 32767);
        let ay = ay.clamp(0, 32767);
        let nx = ((ax * (self.screen_w - 1)) / 32767)
            .clamp(0, self.screen_w.saturating_sub(1));
        let ny = ((ay * (self.screen_h - 1)) / 32767)
            .clamp(0, self.screen_h.saturating_sub(1));
        let moved = nx != self.x || ny != self.y || buttons != self.buttons;
        self.x = nx;
        self.y = ny;
        self.buttons = buttons;
        if moved {
            mice.x = nx;
            mice.y = ny;
            mice.buttons = buttons;
        }
        moved
    }

    fn arm_interrupt_in(&mut self) {
        let (qh_v, qh_p) = self.scratch_offset(0x100);
        let (td_v, td_p) = self.scratch_offset(0x120);
        let (buf_v, buf_p) = self.scratch_offset(0x140);

        let addr = self.addr as u32;
        let ep = self.ep as u32;
        let mps = self.max_packet.max(1) as u32;
        let toggle = if self.data_toggle { 1u32 << 19 } else { 0 };
        let token = ((mps - 1) << 21) | toggle | (ep << 15) | (addr << 8) | TOKEN_IN;

        unsafe {
            let td = td_v as *mut Td;
            let qh = qh_v as *mut Qh;
            for i in 0..8 {
                buf_v.add(i).write_volatile(0);
            }
            td.write_volatile(Td {
                link: 1,
                status: TD_ACTIVE | (3 << 27) | TD_SPD,
                token,
                buffer: buf_p,
            });
            qh.write_volatile(Qh {
                head_link: 1,
                element: td_p,
            });
            // TD/QH must be fully written before the frame list points at them.
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            for i in 0..1024 {
                self.dma.fl.add(i).write_volatile(qh_p | 0x2);
            }
        }
    }
}

const USBINTR_ZERO: u16 = 0x04;

fn set_err(buf: &mut [u8], msg: &str) {
    buf.fill(0);
    let b = msg.as_bytes();
    let n = b.len().min(buf.len().saturating_sub(1));
    buf[..n].copy_from_slice(&b[..n]);
}

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
