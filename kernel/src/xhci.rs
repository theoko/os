//! xHCI controller discovery and register mapping.
//!
//! Ring setup is intentionally separate: resetting/running a controller before
//! DCBAA, command and event rings exist can strand every USB device. This file
//! validates the MMIO capability block and gives the ring layer exact offsets.

use crate::pci;

pub const USBCMD_RUN: u32 = 1 << 0;
pub const USBCMD_HCRST: u32 = 1 << 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub cap_length: usize,
    pub max_slots: u8,
    pub max_ports: u8,
}

pub struct Controller {
    pub mmio: usize,
    pub caps: Capabilities,
}

impl Controller {
    /// Locate and validate the xHCI capability header without mutating hardware.
    pub fn discover(hhdm: u64) -> Option<Self> {
        let (_, _, _, bar) = pci::find_xhci_mmio()?;
        let mmio = (hhdm + bar) as usize;
        let caplength_version = unsafe { core::ptr::read_volatile(mmio as *const u32) };
        let cap_length = (caplength_version & 0xff) as usize;
        if !(0x20..=0x100).contains(&cap_length) {
            return None;
        }
        let hcsparams1 = unsafe { core::ptr::read_volatile((mmio + 0x04) as *const u32) };
        Some(Self {
            mmio,
            caps: Capabilities {
                cap_length,
                max_slots: (hcsparams1 & 0xff) as u8,
                max_ports: ((hcsparams1 >> 24) & 0xff) as u8,
            },
        })
    }

    pub fn operational(&self) -> usize {
        self.mmio + self.caps.cap_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_and_reset_bits_do_not_overlap() {
        assert_eq!(USBCMD_RUN & USBCMD_HCRST, 0);
    }

    #[test]
    fn operational_offset_adds_cap_length_to_mmio() {
        let c = Controller {
            mmio: 0xFE00_0000,
            caps: Capabilities {
                cap_length: 0x40,
                max_slots: 32,
                max_ports: 8,
            },
        };
        assert_eq!(c.operational(), 0xFE00_0040);
    }

    #[test]
    fn capabilities_struct_derived_traits() {
        let caps = Capabilities {
            cap_length: 0x20,
            max_slots: 16,
            max_ports: 4,
        };
        assert_eq!(caps, caps.clone());
    }
}

