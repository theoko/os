//! Minimal polled OHCI host controller for ARM virtual machines.
//!
//! VirtualBox on Apple Silicon exposes an OHCI controller when USB 1.1 is
//! enabled. The UI already knows how to decode USB HID reports, but without a
//! host-controller driver no report ever reaches it. This module implements
//! the deliberately small subset needed by the VM:
//!
//! - reset and power the controller/root hub;
//! - enumerate one HID interface per root-port device;
//! - run boot keyboards, boot mice, and the VirtualBox/QEMU absolute tablet;
//! - poll completed periodic TDs without depending on a GIC driver.
//!
//! It is not a general USB stack. Hubs, isochronous transfers, hotplug, and
//! arbitrary HID report descriptors remain outside this first usable path.

use core::sync::atomic::{Ordering, compiler_fence};

use crate::{
    hid::{self, AbsLayout, HidIface, HidKind, Pointer, TabletFormat},
    keyboard::Key,
    pci, serial,
    time::{self, Timebase},
};

const REVISION: usize = 0x00;
const CONTROL: usize = 0x04;
const COMMAND_STATUS: usize = 0x08;
const INTERRUPT_STATUS: usize = 0x0C;
const INTERRUPT_DISABLE: usize = 0x14;
const HCCA_REG: usize = 0x18;
const CONTROL_HEAD_ED: usize = 0x20;
const BULK_HEAD_ED: usize = 0x28;
const FM_INTERVAL: usize = 0x34;
const PERIODIC_START: usize = 0x40;
const LS_THRESHOLD: usize = 0x44;
const RH_DESCRIPTOR_A: usize = 0x48;
const RH_STATUS: usize = 0x50;
const RH_PORT_STATUS: usize = 0x54;

const CONTROL_CLE: u32 = 1 << 4;
const CONTROL_PLE: u32 = 1 << 2;
const CONTROL_HCFS_MASK: u32 = 0b11 << 6;
const CONTROL_HCFS_OPERATIONAL: u32 = 0b10 << 6;
const CONTROL_IR: u32 = 1 << 8;

const CMD_HCR: u32 = 1 << 0;
const CMD_CLF: u32 = 1 << 1;
const CMD_OCR: u32 = 1 << 3;

const INTERRUPT_WDH: u32 = 1 << 1;

const RH_STATUS_LPSC: u32 = 1 << 16;
const PORT_CCS: u32 = 1 << 0;
const PORT_PES: u32 = 1 << 1;
const PORT_PRS: u32 = 1 << 4;
const PORT_PPS: u32 = 1 << 8;
const PORT_LSDA: u32 = 1 << 9;
const PORT_CHANGES: u32 = 0x001F_0000;

const ED_DIR_FROM_TD: u32 = 0 << 11;
const ED_DIR_IN: u32 = 2 << 11;
const ED_LOW_SPEED: u32 = 1 << 13;
const ED_SKIP: u32 = 1 << 14;

const TD_ROUNDING: u32 = 1 << 18;
const TD_DP_SETUP: u32 = 0 << 19;
const TD_DP_OUT: u32 = 1 << 19;
const TD_DP_IN: u32 = 2 << 19;
const TD_DI_NONE: u32 = 7 << 21;
const TD_T_DATA0: u32 = 2 << 24;
const TD_T_DATA1: u32 = 3 << 24;
const TD_CC_NOT_ACCESSED: u32 = 0xF << 28;
const TD_CC_NO_ERROR: u32 = 0;
const TD_CC_DATA_UNDERRUN: u32 = 9;

const HCCA_OFF: usize = 0x000;
const CONTROL_ED_OFF: usize = 0x100;
const CONTROL_TD0_OFF: usize = 0x120;
const CONTROL_TD1_OFF: usize = 0x130;
const CONTROL_TD2_OFF: usize = 0x140;
const CONTROL_DUMMY_OFF: usize = 0x150;
const SETUP_OFF: usize = 0x180;
const CONTROL_BUF_OFF: usize = 0x200;
const PERIODIC_ED_OFF: usize = 0x400;
const PERIODIC_TD_OFF: usize = 0x500;
const PERIODIC_DUMMY_OFF: usize = 0x580;
const REPORT_OFF: usize = 0x600;

const PAGE: usize = 4096;
const MAX_DEVICES: usize = 4;
const MAX_CONFIG: usize = 192;

#[repr(C, align(256))]
struct Hcca {
    interrupt_table: [u32; 32],
    frame_number: u16,
    pad: u16,
    done_head: u32,
    reserved: [u8; 116],
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Ed {
    control: u32,
    tail_p: u32,
    head_p: u32,
    next_ed: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct Td {
    control: u32,
    cbp: u32,
    next_td: u32,
    be: u32,
}

#[derive(Clone, Copy)]
struct Device {
    active: bool,
    kind: HidKind,
    address: u8,
    endpoint: u8,
    max_packet: u16,
    low_speed: bool,
    tablet_format: TabletFormat,
}

impl Device {
    const EMPTY: Self = Self {
        active: false,
        kind: HidKind::BootKeyboard,
        address: 0,
        endpoint: 0,
        max_packet: 8,
        low_speed: false,
        tablet_format: TabletFormat::Qemu,
    };
}

struct Dma {
    phys: u32,
    virt: *mut u8,
}

impl Dma {
    fn phys(&self, off: usize) -> u32 {
        self.phys + off as u32
    }

    unsafe fn ptr<T>(&self, off: usize) -> *mut T {
        unsafe { self.virt.add(off).cast() }
    }
}

pub struct OhciInput {
    regs: *mut u32,
    dma: Dma,
    clock: Timebase,
    devices: [Device; MAX_DEVICES],
    count: usize,
    keyboard: hid::Keyboard,
    pointer: Pointer,
    has_keyboard: bool,
    has_pointer: bool,
}

impl OhciInput {
    /// Bring up the first OHCI controller and enumerate its root ports.
    ///
    /// `dma_page` must be a page-aligned, below-4-GiB usable physical page.
    /// OHCI stores 32-bit DMA pointers even when the CPU is aarch64.
    pub unsafe fn init(hhdm: u64, dma_page: u64, err: &mut [u8]) -> Option<Self> {
        set_err(err, "no-ohci");
        let (_, _, _, bar) = pci::find_ohci_mmio()?;
        if bar == 0
            || bar > usize::MAX as u64
            || dma_page >= u32::MAX as u64
            || dma_page & 0xFFF != 0
        {
            set_err(err, "bad-address");
            return None;
        }

        // PCI device registers are identity-mapped as Device memory on
        // VirtualBox ARM. Keep the dedicated OHCI page on that same uncached
        // low mapping: this VirtualBox ARM target does not permit explicit
        // EL1 data-cache maintenance, while an uncached alias provides the
        // ownership semantics the controller needs.
        let vbox = serial::is_virtualbox_arm();
        let regs = if vbox {
            bar as usize as *mut u32
        } else {
            hhdm.checked_add(bar)? as usize as *mut u32
        };
        let virt = if vbox {
            dma_page as usize as *mut u8
        } else {
            hhdm.checked_add(dma_page)? as usize as *mut u8
        };
        unsafe {
            for i in 0..PAGE {
                virt.add(i).write_volatile(0);
            }
        }

        let mut me = Self {
            regs,
            dma: Dma {
                phys: dma_page as u32,
                virt,
            },
            clock: Timebase::probe(),
            devices: [Device::EMPTY; MAX_DEVICES],
            count: 0,
            keyboard: hid::Keyboard::new(),
            pointer: Pointer::default(),
            has_keyboard: false,
            has_pointer: false,
        };

        if !me.reset_controller() {
            set_err(err, "reset-timeout");
            return None;
        }

        let ports = (me.read(RH_DESCRIPTOR_A) & 0xFF).min(15) as usize;
        if ports == 0 {
            set_err(err, "no-ports");
            return None;
        }
        me.power_root_hub(ports);

        let mut next_address = 1u8;
        let mut last_error = "no-device";
        for port in 0..ports {
            if me.count == MAX_DEVICES {
                break;
            }
            match me.enumerate_port(port, next_address) {
                Ok(device) => {
                    me.devices[me.count] = device;
                    me.count += 1;
                    next_address = next_address.saturating_add(1);
                }
                Err("empty") => {}
                Err(reason) => last_error = reason,
            }
        }
        if me.count == 0 {
            set_err(err, last_error);
            return None;
        }

        me.install_periodic_schedule();
        me.has_keyboard = me.devices[..me.count]
            .iter()
            .any(|d| d.kind == HidKind::BootKeyboard);
        me.has_pointer = me.devices[..me.count].iter().any(|d| d.kind.is_pointer());
        set_err(err, "ok");
        Some(me)
    }

    pub const fn has_keyboard(&self) -> bool {
        self.has_keyboard
    }

    pub const fn has_pointer(&self) -> bool {
        self.has_pointer
    }

    pub fn next_key(&mut self) -> Option<Key> {
        self.keyboard.next_key()
    }

    pub const fn pointer(&self) -> Pointer {
        self.pointer
    }

    /// Retire every completed interrupt TD and immediately re-arm it.
    ///
    /// Returns true only when pointer state changed. Keyboard reports remain
    /// available through `next_key`, so typing never causes a full redraw by
    /// itself; the normal text-field path decides what became dirty.
    pub fn poll(&mut self, screen_w: i32, screen_h: i32) -> bool {
        // OHCI owns the TDs and report buffers between arms. HCCA.done_head is
        // the controller-defined ownership handoff; walking that queue avoids
        // spinning on a condition-code cache line that ARM may retain.
        dma_consume(&self.dma);
        let completed = self.take_done_mask();
        if completed == 0 {
            return false;
        }

        let mut pointer_changed = false;
        for i in 0..self.count {
            if completed & (1 << i) == 0 {
                continue;
            }
            let td = unsafe { self.periodic_td(i) };
            let control = unsafe { core::ptr::addr_of!((*td).control).read_volatile() };
            let cc = control >> 28;

            let device = self.devices[i];
            if cc == TD_CC_NO_ERROR || cc == TD_CC_DATA_UNDERRUN {
                let report_len = device.max_packet.min(64) as usize;
                let report_ptr = unsafe { self.report_ptr(i) };
                let cbp = unsafe { core::ptr::addr_of!((*td).cbp).read_volatile() };
                let n = actual_len(self.dma.phys(REPORT_OFF + i * 64), cbp, report_len);
                let mut report = [0u8; 64];
                unsafe {
                    for (j, byte) in report.iter_mut().enumerate().take(n) {
                        *byte = report_ptr.add(j).read_volatile();
                    }
                }
                match device.kind {
                    HidKind::BootKeyboard => self.keyboard.feed_report(&report[..n]),
                    HidKind::BootMouse => {
                        if let Some(next) =
                            hid::decode_rel(&report[..n], self.pointer, screen_w, screen_h)
                        {
                            pointer_changed |= next != self.pointer;
                            self.pointer = next;
                        }
                    }
                    HidKind::Tablet => {
                        if let Some(next) = hid::decode_tablet(
                            &report[..n],
                            device.tablet_format,
                            AbsLayout::QEMU_TABLET,
                            screen_w,
                            screen_h,
                        ) {
                            pointer_changed |= next != self.pointer;
                            self.pointer = next;
                        }
                    }
                }
            }
            self.arm_periodic(i);
        }
        pointer_changed
    }

    /// Consume the OHCI done queue and return one bit per completed periodic
    /// TD. Hardware rewrites each TD's `next_td` to build this reverse list.
    fn take_done_mask(&mut self) -> usize {
        let hcca = unsafe { self.dma.ptr::<Hcca>(HCCA_OFF) };
        let mut done = unsafe { core::ptr::addr_of!((*hcca).done_head).read_volatile() } & !0xF;
        if done == 0 {
            return 0;
        }
        unsafe {
            core::ptr::addr_of_mut!((*hcca).done_head).write_volatile(0);
        }
        // HCCA.done_head and HcInterruptStatus.WDH form one ownership
        // handshake. Clearing only the RAM word leaves WDH asserted, so OHCI
        // retains every later completion in its internal HcDoneHead instead
        // of publishing another report to the HCCA. That looks exactly like
        // a pointer which enumerates correctly and then never moves.
        self.write(INTERRUPT_STATUS, INTERRUPT_WDH);

        let mut mask = 0usize;
        let mut walked = 0;
        while done != 0 && walked < 32 {
            for i in 0..self.count {
                if done == self.dma.phys(PERIODIC_TD_OFF + i * 16) {
                    mask |= 1 << i;
                    break;
                }
            }

            let Some(off) = done.checked_sub(self.dma.phys) else {
                break;
            };
            let off = off as usize;
            if off > PAGE.saturating_sub(core::mem::size_of::<Td>()) || off & 0xF != 0 {
                break;
            }
            let td = unsafe { self.dma.ptr::<Td>(off) };
            done = unsafe { core::ptr::addr_of!((*td).next_td).read_volatile() } & !0xF;
            walked += 1;
        }

        // Publish the acknowledgment before OHCI's next frame writeback.
        dma_publish(&self.dma);
        mask
    }

    fn read(&self, off: usize) -> u32 {
        let addr = unsafe { self.regs.add(off / 4) };
        #[cfg(all(target_arch = "aarch64", target_os = "none"))]
        {
            let value: u32;
            unsafe {
                // See `write`: VirtualBox's ARM interpreter must receive the
                // non-writeback MMIO form or the guest PC can remain pinned
                // to this load forever.
                core::arch::asm!(
                    "ldr {value:w}, [{addr}]",
                    addr = in(reg) addr,
                    value = out(reg) value,
                    options(nostack, preserves_flags)
                );
            }
            value
        }
        #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
        unsafe {
            addr.read_volatile()
        }
    }

    fn write(&self, off: usize, value: u32) {
        let addr = unsafe { self.regs.add(off / 4) };
        #[cfg(all(target_arch = "aarch64", target_os = "none"))]
        unsafe {
            // Keep this as the plain-register addressing form. In optimized
            // builds VirtualBox 7.2 ARM can repeatedly trap a pre/post-indexed
            // MMIO store without advancing PC, pinning the guest forever on a
            // valid OHCI write.
            core::arch::asm!(
                "str {value:w}, [{addr}]",
                addr = in(reg) addr,
                value = in(reg) value,
                options(nostack, preserves_flags)
            );
        }
        #[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
        unsafe {
            addr.write_volatile(value);
        }
    }

    fn reset_controller(&mut self) -> bool {
        if self.read(REVISION) & 0xFF != 0x10 {
            return false;
        }

        self.write(INTERRUPT_DISABLE, u32::MAX);
        if self.read(CONTROL) & CONTROL_IR != 0 {
            self.write(COMMAND_STATUS, CMD_OCR);
            let mut deadline = self.clock.deadline(500);
            while self.read(CONTROL) & CONTROL_IR != 0 {
                if deadline.expired() {
                    return false;
                }
                core::hint::spin_loop();
            }
        }

        // VirtualBox ARM presents the controller in the USB reset state after
        // each VM power-on, but its OHCI model can pin the vCPU on the HCR
        // MMIO write itself. Reinitialising every software-visible register
        // below is sufficient there. Other controllers still get the normal
        // host-controller reset and timeout.
        if !serial::is_virtualbox_arm() {
            self.write(COMMAND_STATUS, CMD_HCR);
            let mut deadline = self.clock.deadline(20);
            while self.read(COMMAND_STATUS) & CMD_HCR != 0 {
                if deadline.expired() {
                    return false;
                }
                core::hint::spin_loop();
            }
        }

        self.write(HCCA_REG, self.dma.phys(HCCA_OFF));
        self.write(CONTROL_HEAD_ED, self.dma.phys(CONTROL_ED_OFF));
        self.write(BULK_HEAD_ED, 0);

        // 12 MHz USB frames: 11,999 bit times, with the standard maximum
        // packet start value used by firmware and production OHCI drivers.
        self.write(FM_INTERVAL, 0x2778_2EDF);
        self.write(PERIODIC_START, 0x2A2F);
        self.write(LS_THRESHOLD, 0x0628);
        self.write(
            CONTROL,
            (self.read(CONTROL) & !CONTROL_HCFS_MASK) | CONTROL_HCFS_OPERATIONAL,
        );
        time::delay_ms(&self.clock, 10);
        true
    }

    fn power_root_hub(&self, ports: usize) {
        self.write(RH_STATUS, RH_STATUS_LPSC);
        for port in 0..ports {
            self.write(RH_PORT_STATUS + port * 4, PORT_PPS);
        }
        let pgood = ((self.read(RH_DESCRIPTOR_A) >> 24) & 0xFF).max(10);
        time::delay_ms(&self.clock, pgood.saturating_mul(2));
    }

    fn enumerate_port(&mut self, port: usize, address: u8) -> Result<Device, &'static str> {
        let reg = RH_PORT_STATUS + port * 4;
        let mut status = self.read(reg);
        if status & PORT_CCS == 0 {
            return Err("empty");
        }
        self.write(reg, status & PORT_CHANGES);
        self.write(reg, PORT_PRS);
        let mut deadline = self.clock.deadline(100);
        loop {
            status = self.read(reg);
            if status & PORT_PRS == 0 {
                break;
            }
            if deadline.expired() {
                return Err("port-reset");
            }
            core::hint::spin_loop();
        }
        self.write(reg, status & PORT_CHANGES);
        if status & (PORT_CCS | PORT_PES) != (PORT_CCS | PORT_PES) {
            return Err("port-disabled");
        }
        let low_speed = status & PORT_LSDA != 0;

        let mut first = [0u8; 8];
        self.get_descriptor(0, low_speed, 8, 1, 0, &mut first)
            .map_err(|_| "device-desc")?;
        if first[1] != 1 {
            return Err("bad-device");
        }
        let ep0_mps = match first[7] {
            8 | 16 | 32 | 64 => first[7] as u16,
            _ => 8,
        };
        let mut empty: [u8; 0] = [];
        self.control_transfer(
            0,
            low_speed,
            ep0_mps,
            0x00,
            0x05,
            address as u16,
            0,
            &mut empty,
        )
        .map_err(|_| "set-address")?;
        time::delay_ms(&self.clock, 3);

        // The interface triple identifies an absolute tablet, not its packet
        // byte layout. VirtualBox's 80ee:0021 device puts wheels and padding
        // before X/Y, unlike QEMU's six-byte tablet report.
        let mut device_desc = [0u8; 18];
        self.get_descriptor(address, low_speed, ep0_mps, 1, 0, &mut device_desc)
            .map_err(|_| "device-full")?;
        let vendor = u16::from_le_bytes([device_desc[8], device_desc[9]]);
        let product = u16::from_le_bytes([device_desc[10], device_desc[11]]);
        let tablet_format = if vendor == 0x80EE && product == 0x0021 {
            TabletFormat::VirtualBox
        } else {
            TabletFormat::Qemu
        };

        let mut cfg_head = [0u8; 9];
        self.get_descriptor(address, low_speed, ep0_mps, 2, 0, &mut cfg_head)
            .map_err(|_| "config-head")?;
        if cfg_head[1] != 2 {
            return Err("bad-config");
        }
        let total = u16::from_le_bytes([cfg_head[2], cfg_head[3]]) as usize;
        if !(9..=MAX_CONFIG).contains(&total) {
            return Err("config-size");
        }
        let mut cfg = [0u8; MAX_CONFIG];
        self.get_descriptor(address, low_speed, ep0_mps, 2, 0, &mut cfg[..total])
            .map_err(|_| "config-body")?;

        let mut ifaces = [HidIface {
            kind: HidKind::BootMouse,
            iface: 0,
            ep: 0,
            ep_mps: 0,
            interval: 0,
        }; 4];
        let n_ifaces = hid::list_hid_ifaces(&cfg[..total], &mut ifaces);
        let iface = *ifaces[..n_ifaces].first().ok_or("not-hid")?;
        let config_value = cfg[5].max(1);
        self.control_transfer(
            address,
            low_speed,
            ep0_mps,
            0x00,
            0x09,
            config_value as u16,
            0,
            &mut empty,
        )
        .map_err(|_| "set-config")?;
        time::delay_ms(&self.clock, 2);

        // SET_IDLE is optional for tablets and some firmware stalls it.
        let _ = self.control_transfer(
            address,
            low_speed,
            ep0_mps,
            0x21,
            0x0A,
            0,
            iface.iface as u16,
            &mut empty,
        );
        // VirtualBox's built-in HID devices can stall SET_PROTOCOL even
        // though they already emit the descriptor-declared boot/report
        // layout. This class request is an optimization, not grounds for
        // throwing away an otherwise configured interrupt endpoint.
        let _ = self.control_transfer(
            address,
            low_speed,
            ep0_mps,
            0x21,
            0x0B,
            iface.kind.protocol(),
            iface.iface as u16,
            &mut empty,
        );

        Ok(Device {
            active: true,
            kind: iface.kind,
            address,
            endpoint: iface.ep,
            max_packet: iface.ep_mps.clamp(1, 64),
            low_speed,
            tablet_format,
        })
    }

    fn get_descriptor(
        &mut self,
        address: u8,
        low_speed: bool,
        ep0_mps: u16,
        desc_type: u8,
        index: u8,
        data: &mut [u8],
    ) -> Result<(), &'static str> {
        self.control_transfer(
            address,
            low_speed,
            ep0_mps,
            0x80,
            0x06,
            ((desc_type as u16) << 8) | index as u16,
            0,
            data,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn control_transfer(
        &mut self,
        address: u8,
        low_speed: bool,
        ep0_mps: u16,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
    ) -> Result<(), &'static str> {
        if data.len() > 256 {
            return Err("control-size");
        }

        let setup = unsafe { self.dma.ptr::<u8>(SETUP_OFF) };
        let buffer = unsafe { self.dma.ptr::<u8>(CONTROL_BUF_OFF) };
        unsafe {
            let packet = [
                request_type,
                request,
                value as u8,
                (value >> 8) as u8,
                index as u8,
                (index >> 8) as u8,
                data.len() as u8,
                (data.len() >> 8) as u8,
            ];
            for (i, byte) in packet.iter().enumerate() {
                setup.add(i).write_volatile(*byte);
            }
            for i in 0..data.len() {
                buffer
                    .add(i)
                    .write_volatile(if request_type & 0x80 == 0 { data[i] } else { 0 });
            }
        }

        let ed = unsafe { self.dma.ptr::<Ed>(CONTROL_ED_OFF) };
        let td0 = unsafe { self.dma.ptr::<Td>(CONTROL_TD0_OFF) };
        let td1 = unsafe { self.dma.ptr::<Td>(CONTROL_TD1_OFF) };
        let td2 = unsafe { self.dma.ptr::<Td>(CONTROL_TD2_OFF) };
        let dummy = unsafe { self.dma.ptr::<Td>(CONTROL_DUMMY_OFF) };
        let td0_p = self.dma.phys(CONTROL_TD0_OFF);
        let td1_p = self.dma.phys(CONTROL_TD1_OFF);
        let td2_p = self.dma.phys(CONTROL_TD2_OFF);
        let dummy_p = self.dma.phys(CONTROL_DUMMY_OFF);
        let data_in = request_type & 0x80 != 0;
        let has_data = !data.is_empty();

        unsafe {
            dummy.write_volatile(Td {
                control: 0,
                cbp: 0,
                next_td: 0,
                be: 0,
            });
            td0.write_volatile(Td {
                control: TD_CC_NOT_ACCESSED | TD_DI_NONE | TD_T_DATA0 | TD_DP_SETUP,
                cbp: self.dma.phys(SETUP_OFF),
                next_td: if has_data { td1_p } else { td2_p },
                be: self.dma.phys(SETUP_OFF + 7),
            });
            if has_data {
                td1.write_volatile(Td {
                    control: TD_CC_NOT_ACCESSED
                        | TD_DI_NONE
                        | TD_T_DATA1
                        | if data_in {
                            TD_DP_IN | TD_ROUNDING
                        } else {
                            TD_DP_OUT
                        },
                    cbp: self.dma.phys(CONTROL_BUF_OFF),
                    next_td: td2_p,
                    be: self.dma.phys(CONTROL_BUF_OFF + data.len() - 1),
                });
            }
            td2.write_volatile(Td {
                control: TD_CC_NOT_ACCESSED
                    | TD_DI_NONE
                    | TD_T_DATA1
                    | if has_data && data_in {
                        TD_DP_OUT
                    } else {
                        TD_DP_IN
                    },
                cbp: 0,
                next_td: dummy_p,
                be: 0,
            });
            ed.write_volatile(Ed {
                control: ed_control(address, 0, ep0_mps, low_speed, ED_DIR_FROM_TD),
                tail_p: dummy_p,
                head_p: td0_p,
                next_ed: 0,
            });
        }
        dma_publish(&self.dma);

        self.write(CONTROL_HEAD_ED, self.dma.phys(CONTROL_ED_OFF));
        self.write(CONTROL, self.read(CONTROL) | CONTROL_CLE);
        self.write(COMMAND_STATUS, CMD_CLF);

        let mut deadline = self.clock.deadline(500);
        loop {
            dma_consume(&self.dma);
            let head = unsafe { core::ptr::addr_of!((*ed).head_p).read_volatile() };
            if head & 1 != 0 {
                return Err("control-stall");
            }
            if head & !0xF == dummy_p {
                break;
            }
            if deadline.expired() {
                return Err("control-timeout");
            }
            core::hint::spin_loop();
        }
        dma_consume(&self.dma);

        let setup_cc = unsafe { core::ptr::addr_of!((*td0).control).read_volatile() >> 28 };
        let data_cc = if has_data {
            unsafe { core::ptr::addr_of!((*td1).control).read_volatile() >> 28 }
        } else {
            TD_CC_NO_ERROR
        };
        let status_cc = unsafe { core::ptr::addr_of!((*td2).control).read_volatile() >> 28 };
        if setup_cc != TD_CC_NO_ERROR
            || !matches!(data_cc, TD_CC_NO_ERROR | TD_CC_DATA_UNDERRUN)
            || status_cc != TD_CC_NO_ERROR
        {
            return Err("control-error");
        }
        if has_data && data_in {
            unsafe {
                for (i, byte) in data.iter_mut().enumerate() {
                    *byte = buffer.add(i).read_volatile();
                }
            }
        }
        Ok(())
    }

    fn install_periodic_schedule(&mut self) {
        for i in 0..self.count {
            let next = if i + 1 < self.count {
                self.dma.phys(PERIODIC_ED_OFF + (i + 1) * 16)
            } else {
                0
            };
            let device = self.devices[i];
            let ed = unsafe { self.periodic_ed(i) };
            unsafe {
                ed.write_volatile(Ed {
                    control: ed_control(
                        device.address,
                        device.endpoint,
                        device.max_packet,
                        device.low_speed,
                        ED_DIR_IN,
                    ) | ED_SKIP,
                    tail_p: self.dma.phys(PERIODIC_DUMMY_OFF + i * 16),
                    head_p: self.dma.phys(PERIODIC_DUMMY_OFF + i * 16),
                    next_ed: next,
                });
            }
        }

        let hcca = unsafe { self.dma.ptr::<Hcca>(HCCA_OFF) };
        let first = self.dma.phys(PERIODIC_ED_OFF);
        unsafe {
            for slot in &mut (*hcca).interrupt_table {
                core::ptr::write_volatile(slot, first);
            }
        }
        for i in 0..self.count {
            self.arm_periodic(i);
        }
        dma_publish(&self.dma);
        self.write(CONTROL, self.read(CONTROL) | CONTROL_PLE);
    }

    fn arm_periodic(&mut self, i: usize) {
        let device = self.devices[i];
        if !device.active {
            return;
        }
        let ed = unsafe { self.periodic_ed(i) };
        let td = unsafe { self.periodic_td(i) };
        let dummy = unsafe { self.periodic_dummy(i) };
        let report = unsafe { self.report_ptr(i) };
        let td_p = self.dma.phys(PERIODIC_TD_OFF + i * 16);
        let dummy_p = self.dma.phys(PERIODIC_DUMMY_OFF + i * 16);
        let report_p = self.dma.phys(REPORT_OFF + i * 64);
        let carry = unsafe { core::ptr::addr_of!((*ed).head_p).read_volatile() & 0x2 };

        unsafe {
            for j in 0..device.max_packet as usize {
                report.add(j).write_volatile(0);
            }
            dummy.write_volatile(Td {
                control: 0,
                cbp: 0,
                next_td: 0,
                be: 0,
            });
            td.write_volatile(Td {
                // DI=0 asks OHCI to publish this completion through
                // HCCA.done_head at the end of the frame. The driver polls
                // that RAM queue instead of using a GIC interrupt, but it
                // still needs the controller's normal writeback handshake.
                control: TD_CC_NOT_ACCESSED | TD_ROUNDING,
                cbp: report_p,
                next_td: dummy_p,
                be: report_p + device.max_packet as u32 - 1,
            });
            core::ptr::addr_of_mut!((*ed).tail_p).write_volatile(dummy_p);
            core::ptr::addr_of_mut!((*ed).head_p).write_volatile(td_p | carry);
            let control = ed_control(
                device.address,
                device.endpoint,
                device.max_packet,
                device.low_speed,
                ED_DIR_IN,
            );
            core::ptr::addr_of_mut!((*ed).control).write_volatile(control);
        }
        dma_publish(&self.dma);
    }

    unsafe fn periodic_ed(&self, i: usize) -> *mut Ed {
        unsafe { self.dma.ptr(PERIODIC_ED_OFF + i * 16) }
    }

    unsafe fn periodic_td(&self, i: usize) -> *mut Td {
        unsafe { self.dma.ptr(PERIODIC_TD_OFF + i * 16) }
    }

    unsafe fn periodic_dummy(&self, i: usize) -> *mut Td {
        unsafe { self.dma.ptr(PERIODIC_DUMMY_OFF + i * 16) }
    }

    unsafe fn report_ptr(&self, i: usize) -> *mut u8 {
        unsafe { self.dma.virt.add(REPORT_OFF + i * 64) }
    }
}

fn ed_control(address: u8, endpoint: u8, mps: u16, low_speed: bool, direction: u32) -> u32 {
    (address as u32 & 0x7F)
        | ((endpoint as u32 & 0x0F) << 7)
        | direction
        | if low_speed { ED_LOW_SPEED } else { 0 }
        | ((mps.clamp(1, 1023) as u32) << 16)
}

fn actual_len(buffer_phys: u32, current: u32, requested: usize) -> usize {
    if current == 0 {
        return requested;
    }
    current
        .checked_sub(buffer_phys)
        .map(|n| n as usize)
        .unwrap_or(0)
        .min(requested)
}

fn dma_publish(dma: &Dma) {
    compiler_fence(Ordering::SeqCst);
    #[cfg(all(target_arch = "aarch64", target_os = "none"))]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let _ = dma;
}

fn dma_consume(dma: &Dma) {
    #[cfg(all(target_arch = "aarch64", target_os = "none"))]
    unsafe {
        core::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
    let _ = dma;
    compiler_fence(Ordering::SeqCst);
}

fn set_err(buf: &mut [u8], msg: &str) {
    buf.fill(0);
    let bytes = msg.as_bytes();
    let n = bytes.len().min(buf.len().saturating_sub(1));
    buf[..n].copy_from_slice(&bytes[..n]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_control_keeps_address_endpoint_speed_and_packet_size_separate() {
        let control = ed_control(5, 3, 64, true, ED_DIR_IN);
        assert_eq!(control & 0x7F, 5);
        assert_eq!((control >> 7) & 0x0F, 3);
        assert_eq!((control >> 11) & 0x03, 2);
        assert_ne!(control & ED_LOW_SPEED, 0);
        assert_eq!((control >> 16) & 0x7FF, 64);
    }

    #[test]
    fn a_fully_consumed_buffer_reports_the_requested_size() {
        assert_eq!(actual_len(0x1000, 0, 8), 8);
    }

    #[test]
    fn a_short_packet_reports_only_the_bytes_the_controller_advanced_past() {
        assert_eq!(actual_len(0x1000, 0x1006, 8), 6);
        assert_eq!(actual_len(0x1000, 0x2000, 8), 8);
        assert_eq!(actual_len(0x1000, 0x0FF0, 8), 0);
    }
}

