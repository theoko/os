//! Just enough ACPI to find where PCI config space lives.
//!
//! aarch64 has no configuration ports, so the only way to read a device's
//! vendor ID is the memory-mapped ECAM window — and its base is not a constant.
//! QEMU `virt` moves it depending on machine type, RAM size and whether UEFI
//! firmware is loaded; VirtualBox's `armv8virtual` is its own device model
//! entirely. Guessing wrong is not a wrong answer, it is a synchronous
//! external abort into a kernel with no aarch64 vector table: a silent reset.
//!
//! So: read the MCFG table, or say we could not and leave PCI unreachable.
//! Everything that does not dereference hardware is a free function over a
//! slice, so the table walk is host-testable without a machine.

/// One MCFG allocation: a config window and the buses it covers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ecam {
    pub base: u64,
    pub segment: u16,
    pub start_bus: u8,
    pub end_bus: u8,
}

/// VirtualBox ARM's fixed 16-MiB PCI configuration window.
///
/// The platform advertises the same address through MCFG, but some current
/// VirtualBox firmware builds contain table pointers outside the mappings
/// Limine keeps. The RSDP OEM gate below makes this a platform quirk, not a
/// blind MMIO guess on every ARM machine.
pub const VBOX_ARM_ECAM: Ecam = Ecam {
    base: 0xFEDD_C000,
    segment: 0,
    start_bus: 0,
    end_bus: 15,
};

/// ACPI tables are summed to zero over their whole length.
pub fn checksum_ok(bytes: &[u8]) -> bool {
    bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b)) == 0
}

/// Does an already-readable RSDP belong to VirtualBox?
///
/// # Safety
/// `rsdp` and `hhdm` must be the values supplied by Limine. Firmware/bootloader
/// combinations disagree on whether the response pointer is already biased,
/// so normalize it exactly as `find_ecam` does.
pub unsafe fn rsdp_is_virtualbox(rsdp: usize, hhdm: u64) -> bool {
    let root = if hhdm != 0 && (rsdp as u64) < hhdm {
        rsdp + hhdm as usize
    } else {
        rsdp
    };
    let mut header = [0u8; 20];
    unsafe { copy_from(root, &mut header) };
    &header[..8] == b"RSD PTR " && checksum_ok(&header) && header[9..15].starts_with(b"VBOX")
}

/// Length field of a system description table header.
///
/// Bounded on both ends: a zero length walks nothing and a nonsense length
/// would have us read megabytes of unmapped memory looking for a signature.
pub fn sdt_len(header: &[u8]) -> Option<usize> {
    if header.len() < 36 {
        return None;
    }
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    (36..=0x10_0000).contains(&len).then_some(len)
}

/// First allocation entry of an MCFG table.
///
/// Layout: 36-byte SDT header, 8 reserved bytes, then 16-byte entries of
/// `u64 base, u16 segment, u8 start_bus, u8 end_bus, u32 reserved`.
pub fn parse_mcfg(table: &[u8]) -> Option<Ecam> {
    if table.len() < 60 || &table[..4] != b"MCFG" {
        return None;
    }
    let len = sdt_len(table)?;
    // A truncated entry is not a short window, it is a table we misread.
    if len < 60 || len > table.len() {
        return None;
    }
    let e = &table[44..60];
    let base = u64::from_le_bytes([e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7]]);
    let segment = u16::from_le_bytes([e[8], e[9]]);
    let start_bus = e[10];
    let end_bus = e[11];
    if base == 0 || end_bus < start_bus {
        return None;
    }
    Some(Ecam {
        base,
        segment,
        start_bus,
        end_bus,
    })
}

/// Bytes the window occupies: one MiB per bus.
pub const fn ecam_span(e: &Ecam) -> u64 {
    ((e.end_bus - e.start_bus) as u64 + 1) << 20
}

/// Is `[base, base + span)` somewhere the bootloader actually mapped?
///
/// Limine's higher-half direct map covers the first 4 GiB plus every memory
/// map region. An MMIO window outside both is not in the HHDM, and touching
/// `hhdm + base` there aborts. QEMU `virt` plausibly puts its high ECAM around
/// 256 GiB, so this check is the difference between a reported failure and a
/// machine that resets with nothing on screen. It is not defensive; it is the
/// reason the ECAM path is safe to attempt at all.
pub fn mmap_covers(mmap: &limine::response::MemoryMapResponse, base: u64, span: u64) -> bool {
    let end = base.saturating_add(span);
    if end <= 0x1_0000_0000 {
        return true;
    }
    mmap.entries()
        .iter()
        .any(|e| base >= e.base && end <= e.base.saturating_add(e.length))
}

/// Walk RSDP → XSDT/RSDT → MCFG.
///
/// # Safety
/// `rsdp` must be the address limine reported, and `hhdm` its direct-map
/// offset. Nothing else is dereferenced: every table address comes from a
/// table we already validated.
pub unsafe fn find_ecam(rsdp: usize, hhdm: u64) -> Option<Ecam> {
    // Base revision 3 reports a *physical* RSDP, so the direct map is where it
    // is readable. An address already above the HHDM offset is one limine
    // biased for us; adding the offset twice would be the fault this whole
    // module exists to avoid.
    let root = if hhdm != 0 && (rsdp as u64) < hhdm {
        rsdp + hhdm as usize
    } else {
        rsdp
    };

    let mut hdr = [0u8; 36];
    unsafe { copy_from(root, &mut hdr[..20]) };
    if &hdr[..8] != b"RSD PTR " || !checksum_ok(&hdr[..20]) {
        return None;
    }

    let (table_phys, entry_size) = if hdr[15] >= 2 {
        let mut ext = [0u8; 12];
        unsafe { copy_from(root + 20, &mut ext) };
        (
            u64::from_le_bytes([
                ext[4], ext[5], ext[6], ext[7], ext[8], ext[9], ext[10], ext[11],
            ]),
            8usize,
        )
    } else {
        (
            u32::from_le_bytes([hdr[16], hdr[17], hdr[18], hdr[19]]) as u64,
            4usize,
        )
    };
    if table_phys == 0 {
        return None;
    }

    let root_sdt = (table_phys + hhdm) as usize;
    let mut sdt = [0u8; 36];
    unsafe { copy_from(root_sdt, &mut sdt) };
    let len = sdt_len(&sdt)?;

    // Capped: a corrupt length must not turn into a long walk through memory
    // that may not be mapped.
    let count = ((len - 36) / entry_size).min(64);
    for i in 0..count {
        let mut raw = [0u8; 8];
        unsafe { copy_from(root_sdt + 36 + i * entry_size, &mut raw[..entry_size]) };
        let phys = if entry_size == 8 {
            u64::from_le_bytes(raw)
        } else {
            u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as u64
        };
        if phys == 0 {
            continue;
        }
        let virt = (phys + hhdm) as usize;
        let mut head = [0u8; 36];
        unsafe { copy_from(virt, &mut head) };
        if &head[..4] != b"MCFG" {
            continue;
        }
        let tlen = sdt_len(&head)?.min(256);
        let mut table = [0u8; 256];
        unsafe { copy_from(virt, &mut table[..tlen]) };
        return parse_mcfg(&table[..tlen]);
    }
    None
}

/// Byte-at-a-time so an odd table address cannot fault on an unaligned word.
unsafe fn copy_from(addr: usize, out: &mut [u8]) {
    for (i, b) in out.iter_mut().enumerate() {
        *b = unsafe { core::ptr::read_volatile((addr + i) as *const u8) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed MCFG with one allocation.
    fn mcfg(base: u64, start_bus: u8, end_bus: u8) -> [u8; 60] {
        let mut t = [0u8; 60];
        t[..4].copy_from_slice(b"MCFG");
        t[4..8].copy_from_slice(&60u32.to_le_bytes());
        t[44..52].copy_from_slice(&base.to_le_bytes());
        t[52..54].copy_from_slice(&0u16.to_le_bytes());
        t[54] = start_bus;
        t[55] = end_bus;
        t
    }

    #[test]
    fn the_first_allocation_names_a_window_and_its_buses() {
        let got = parse_mcfg(&mcfg(0x4010_0000_00, 0, 15)).expect("a valid MCFG must parse");
        assert_eq!(got.base, 0x4010_0000_00);
        assert_eq!((got.start_bus, got.end_bus), (0, 15));
    }

    #[test]
    fn a_table_that_stops_mid_entry_is_refused() {
        // Half an allocation read as a whole one yields a plausible, wrong
        // base — the exact failure that aborts on the first config read.
        let full = mcfg(0x4010_0000_00, 0, 15);
        assert!(parse_mcfg(&full[..52]).is_none());
    }

    #[test]
    fn a_window_at_address_zero_is_not_a_window() {
        assert!(parse_mcfg(&mcfg(0, 0, 15)).is_none());
    }

    #[test]
    fn a_bus_range_that_runs_backwards_is_refused() {
        assert!(parse_mcfg(&mcfg(0x4000_0000, 32, 4)).is_none());
    }

    #[test]
    fn another_table_is_not_mistaken_for_mcfg() {
        let mut t = mcfg(0x4000_0000, 0, 15);
        t[..4].copy_from_slice(b"FACP");
        assert!(parse_mcfg(&t).is_none());
    }

    #[test]
    fn a_lying_length_field_does_not_become_a_walk_through_memory() {
        let mut t = mcfg(0x4000_0000, 0, 15);
        t[4..8].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(sdt_len(&t).is_none());
        assert!(parse_mcfg(&t).is_none());
    }

    #[test]
    fn a_span_is_one_megabyte_per_bus() {
        let e = Ecam {
            base: 0,
            segment: 0,
            start_bus: 0,
            end_bus: 15,
        };
        assert_eq!(ecam_span(&e), 16 << 20);
        let one = Ecam {
            base: 0,
            segment: 0,
            start_bus: 4,
            end_bus: 4,
        };
        assert_eq!(ecam_span(&one), 1 << 20);
    }

    #[test]
    fn a_checksum_that_does_not_sum_to_zero_is_rejected() {
        let mut rsdp = [0u8; 20];
        rsdp[..8].copy_from_slice(b"RSD PTR ");
        assert!(!checksum_ok(&rsdp));
        // Byte 8 is the checksum field: make the whole thing sum to zero.
        rsdp[8] = 0u8.wrapping_sub(rsdp.iter().fold(0u8, |a, b| a.wrapping_add(*b)));
        assert!(checksum_ok(&rsdp));
    }

    #[test]
    fn the_virtualbox_window_matches_the_logged_arm_platform_map() {
        assert_eq!(VBOX_ARM_ECAM.base, 0xFEDD_C000);
        assert_eq!(ecam_span(&VBOX_ARM_ECAM), 16 << 20);
    }
}
