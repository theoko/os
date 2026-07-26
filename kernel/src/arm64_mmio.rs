//! Low MMIO window for the ARM64 virtual-machine platform.
//!
//! Limine enters the kernel in the higher half and supplies a higher-half
//! direct map for RAM. VirtualBox's ARM PCI ECAM and PCI BARs, however, live
//! below 4 GiB and are not necessarily retained in TTBR0 after UEFI exits.
//! Accessing an unmapped ECAM address produces a synchronous abort before the
//! USB driver can say what went wrong.
//!
//! The kernel, stack, framebuffer backing store, and DMA buffers are all
//! reached through TTBR1. We can therefore give TTBR0 one deliberately small
//! job: identity-map the 32-bit device aperture as Device memory. Four 1-GiB
//! level-1 blocks cover the range without an allocator or a page-table walk.

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
use core::arch::asm;

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
#[repr(C, align(4096))]
struct Root([u64; 512]);

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
static mut LOW_ROOT: Root = Root([0; 512]);

const VALID: u64 = 1;
const ACCESS_FLAG: u64 = 1 << 10;
const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;

/// A level-1 1-GiB block descriptor for a 4-KiB translation granule.
const fn device_block(phys: u64, attr_index: u8) -> u64 {
    (phys & 0x0000_FFFF_C000_0000)
        | ((attr_index as u64 & 7) << 2)
        | ACCESS_FLAG
        | PXN
        | UXN
        | VALID
}

/// Install an identity-mapped Device window covering physical 0..4 GiB.
///
/// Returns false when the live translation regime is not the 4-KiB setup this
/// compact table is built for, or when the kernel stack is unexpectedly in
/// the low address space.
#[cfg(all(target_arch = "aarch64", target_os = "none"))]
pub fn install_low_device_window() -> bool {
    let tcr: u64;
    let mair: u64;
    let sp: u64;
    unsafe {
        asm!("mrs {0}, tcr_el1", out(reg) tcr, options(nomem, nostack));
        asm!("mrs {0}, mair_el1", out(reg) mair, options(nomem, nostack));
        asm!("mov {0}, sp", out(reg) sp, options(nomem, nostack));
    }

    // TG0=00 is 4 KiB. Replacing TTBR0 must not replace the live stack.
    if (tcr >> 14) & 0b11 != 0 || sp < 0x1_0000_0000 {
        return false;
    }

    // Reuse a MAIR slot already describing Device memory. Limine normally
    // supplies one; avoiding a MAIR rewrite guarantees TTBR1's attributes do
    // not change underneath the running kernel.
    let Some(attr) = (0..8u8).find(|i| {
        matches!(
            ((mair >> (*i as u64 * 8)) & 0xFF) as u8,
            0x00 | 0x04 | 0x08 | 0x0C
        )
    }) else {
        return false;
    };

    unsafe {
        let root = &raw mut LOW_ROOT;
        for i in 0..512 {
            (*root).0[i] = 0;
        }
        for i in 0..4usize {
            (*root).0[i] = device_block((i as u64) << 30, attr);
        }

        let root_va = root as u64;
        let par: u64;
        asm!(
            "at s1e1r, {va}",
            "isb",
            "mrs {par}, par_el1",
            va = in(reg) root_va,
            par = out(reg) par,
            options(nostack)
        );
        if par & 1 != 0 {
            return false;
        }
        let root_pa = (par & 0x0000_FFFF_FFFF_F000) | (root_va & 0xFFF);

        // A 32-bit TTBR0 VA space starts at level 1 with a 4-KiB granule.
        let next_tcr = (tcr & !0x3F) | 32;
        asm!(
            "dsb ishst",
            "msr ttbr0_el1, {root}",
            "msr tcr_el1, {tcr}",
            "isb",
            "tlbi vmalle1is",
            "dsb ish",
            "isb",
            root = in(reg) root_pa,
            tcr = in(reg) next_tcr,
            options(nostack)
        );
    }
    true
}

#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
pub fn install_low_device_window() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_are_identity_mapped_in_one_gib_steps() {
        for i in 0..4u64 {
            let d = device_block(i << 30, 3);
            assert_eq!(d & 0x0000_FFFF_C000_0000, i << 30);
            assert_eq!((d >> 2) & 7, 3);
            assert_ne!(d & VALID, 0);
            assert_ne!(d & ACCESS_FLAG, 0);
            assert_ne!(d & (PXN | UXN), 0);
            assert_eq!(d & 2, 0, "level-1 entries are blocks, not tables");
        }
    }
}
