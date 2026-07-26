//! Say what went wrong instead of vanishing.
//!
//! This kernel shipped without an interrupt descriptor table. That is fine
//! right up until the CPU raises an exception — a bad pointer, a divide by
//! zero, an overflow check firing in a debug build. With no IDT there is no
//! handler, so the CPU escalates: fault, then double fault, then triple fault,
//! and the machine resets. From the outside that looks exactly like "it just
//! randomly crashes": the window blinks and comes back to the boot screen with
//! no message anywhere, in the log or on the display.
//!
//! Installing handlers does not stop the bug that caused the fault. It stops
//! the *silence*. Every fault now prints its vector, faulting address, and
//! instruction pointer to COM1 and halts where it happened, so a crash report
//! becomes a line you can read instead of a reboot you have to guess at.

use crate::serial::Serial;

/// One 64-bit IDT gate.
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct Gate {
    off_lo: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    off_mid: u16,
    off_hi: u32,
    reserved: u32,
}

impl Gate {
    /// Present, DPL 0, 64-bit interrupt gate.
    const PRESENT_INTERRUPT: u8 = 0x8E;

    fn new(handler: unsafe extern "C" fn(), selector: u16) -> Self {
        let addr = handler as usize as u64;
        Self {
            off_lo: addr as u16,
            selector,
            ist: 0,
            flags: Self::PRESENT_INTERRUPT,
            off_mid: (addr >> 16) as u16,
            off_hi: (addr >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct Descriptor {
    limit: u16,
    base: u64,
}

/// The CPU's first 32 vectors — everything the processor itself raises.
const VECTORS: usize = 32;

static mut IDT: [Gate; VECTORS] = [Gate {
    off_lo: 0,
    selector: 0,
    ist: 0,
    flags: 0,
    off_mid: 0,
    off_hi: 0,
    reserved: 0,
}; VECTORS];

/// Which vectors push an error code. The stub for the others pushes a zero so
/// the reporting code sees one stack shape rather than two.
///
/// The stub list below is checked against this at compile time. Disagreeing
/// would shift the whole frame by eight bytes and print a plausible, wrong
/// `rip` — the worst possible failure for a crash report.
const HAS_ERROR_CODE: [bool; VECTORS] = {
    let mut v = [false; VECTORS];
    v[8] = true; // double fault
    v[10] = true; // invalid TSS
    v[11] = true; // segment not present
    v[12] = true; // stack-segment fault
    v[13] = true; // general protection
    v[14] = true; // page fault
    v[17] = true; // alignment check
    v[21] = true; // control protection
    v[29] = true; // VMM communication
    v[30] = true; // security exception
    v
};

pub fn name(vector: u64) -> &'static str {
    match vector {
        0 => "divide by zero",
        1 => "debug",
        2 => "non-maskable interrupt",
        3 => "breakpoint",
        4 => "overflow",
        5 => "bound range exceeded",
        6 => "invalid opcode",
        7 => "device not available",
        8 => "double fault",
        10 => "invalid TSS",
        11 => "segment not present",
        12 => "stack-segment fault",
        13 => "general protection fault",
        14 => "page fault",
        16 => "x87 floating point",
        17 => "alignment check",
        18 => "machine check",
        19 => "SIMD floating point",
        _ => "exception",
    }
}

/// Install the table. Safe to call once, early, before anything can fault.
#[cfg(target_arch = "x86_64")]
pub fn init() {
    let selector: u16;
    unsafe {
        core::arch::asm!("mov {0:x}, cs", out(reg) selector, options(nomem, nostack, preserves_flags));
    }

    // SAFETY: single-threaded boot path, called once before interrupts matter.
    unsafe {
        let idt = &raw mut IDT;
        for (i, gate) in (*idt).iter_mut().enumerate() {
            *gate = Gate::new(STUBS[i], selector);
        }
        let descriptor = Descriptor {
            limit: (core::mem::size_of::<[Gate; VECTORS]>() - 1) as u16,
            base: idt as u64,
        };
        core::arch::asm!("lidt [{}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags));
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn init() {}

/// Format a fault report. Split out from the handler so it is testable on the
/// host — a crash message with a bug in it is worse than none.
pub fn report_line(buf: &mut [u8; 160], vector: u64, code: u64, rip: u64, cr2: u64) -> usize {
    let mut n = 0;
    put(buf, &mut n, "os: FAULT ");
    put(buf, &mut n, name(vector));
    put(buf, &mut n, " vec=");
    put_hex(buf, &mut n, vector);
    put(buf, &mut n, " err=");
    put_hex(buf, &mut n, code);
    put(buf, &mut n, " rip=");
    put_hex(buf, &mut n, rip);
    if vector == 14 {
        put(buf, &mut n, " addr=");
        put_hex(buf, &mut n, cr2);
    }
    put(buf, &mut n, "\n");
    n
}

fn put(buf: &mut [u8; 160], n: &mut usize, s: &str) {
    for b in s.bytes() {
        if *n < buf.len() {
            buf[*n] = b;
            *n += 1;
        }
    }
}

fn put_hex(buf: &mut [u8; 160], n: &mut usize, mut v: u64) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut tmp = [0u8; 16];
    let mut len = 0;
    loop {
        tmp[len] = DIGITS[(v & 0xf) as usize];
        len += 1;
        v >>= 4;
        if v == 0 {
            break;
        }
    }
    if *n + 2 <= buf.len() {
        buf[*n] = b'0';
        buf[*n + 1] = b'x';
        *n += 2;
    }
    while len > 0 {
        len -= 1;
        if *n < buf.len() {
            buf[*n] = tmp[len];
            *n += 1;
        }
    }
}

/// Called by every stub with a normalised frame: `[err, rip, cs, rflags, ...]`.
///
/// # Safety
/// `frame` must point at the interrupt stack frame the stub set up.
#[cfg(target_arch = "x86_64")]
#[unsafe(no_mangle)]
unsafe extern "C" fn os_fault_report(vector: u64, frame: *const u64) -> ! {
    let (code, rip) = unsafe { (*frame, *frame.add(1)) };
    let cr2: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
    }

    let com1 = Serial::com1();
    let mut buf = [0u8; 160];
    let n = report_line(&mut buf, vector, code, rip, cr2);
    com1.write_bytes(&buf[..n]);
    com1.write_str("os: halted at the fault - this is a bug, not a reboot\n");

    crate::serial::halt()
}

/// Build the 32 entry stubs.
///
/// Each one normalises the stack (pushing a zero error code where the CPU did
/// not push one), loads the vector number, and hands both to the reporter. The
/// handlers never return: every vector here is fatal for this kernel, and
/// resuming from an unknown fault would just crash again somewhere less
/// obvious.
#[cfg(target_arch = "x86_64")]
macro_rules! stub {
    ($name:ident, $vector:expr, true) => {
        const _: () = assert!(HAS_ERROR_CODE[$vector], "stub claims an error code the CPU does not push");
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "mov rdi, {v}",
                "mov rsi, rsp",
                "call {report}",
                v = const $vector,
                report = sym os_fault_report,
            )
        }
    };
    ($name:ident, $vector:expr, false) => {
        const _: () = assert!(!HAS_ERROR_CODE[$vector], "stub omits an error code the CPU pushes");
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "push 0",
                "mov rdi, {v}",
                "mov rsi, rsp",
                "call {report}",
                v = const $vector,
                report = sym os_fault_report,
            )
        }
    };
}

#[cfg(target_arch = "x86_64")]
macro_rules! stubs {
    ($(($name:ident, $vector:expr, $code:tt)),* $(,)?) => {
        $(stub!($name, $vector, $code);)*
        static STUBS: [unsafe extern "C" fn(); VECTORS] = [$($name),*];
    };
}

#[cfg(target_arch = "x86_64")]
stubs![
    (v0, 0, false),
    (v1, 1, false),
    (v2, 2, false),
    (v3, 3, false),
    (v4, 4, false),
    (v5, 5, false),
    (v6, 6, false),
    (v7, 7, false),
    (v8, 8, true),
    (v9, 9, false),
    (v10, 10, true),
    (v11, 11, true),
    (v12, 12, true),
    (v13, 13, true),
    (v14, 14, true),
    (v15, 15, false),
    (v16, 16, false),
    (v17, 17, true),
    (v18, 18, false),
    (v19, 19, false),
    (v20, 20, false),
    (v21, 21, true),
    (v22, 22, false),
    (v23, 23, false),
    (v24, 24, false),
    (v25, 25, false),
    (v26, 26, false),
    (v27, 27, false),
    (v28, 28, false),
    (v29, 29, true),
    (v30, 30, true),
    (v31, 31, false),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn line(vector: u64, code: u64, rip: u64, cr2: u64) -> heapless_str {
        let mut buf = [0u8; 160];
        let n = report_line(&mut buf, vector, code, rip, cr2);
        heapless_str(buf, n)
    }

    struct heapless_str([u8; 160], usize);
    impl heapless_str {
        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.0[..self.1]).unwrap()
        }
    }

    #[test]
    fn a_page_fault_names_the_address_it_touched() {
        let s = line(14, 0x2, 0xffff_8000_0010_1234, 0xdead_beef);
        assert!(s.as_str().contains("page fault"), "{}", s.as_str());
        assert!(s.as_str().contains("addr=0xdeadbeef"), "{}", s.as_str());
        assert!(
            s.as_str().contains("rip=0xffff800000101234"),
            "{}",
            s.as_str()
        );
    }

    #[test]
    fn a_fault_without_an_address_does_not_print_a_stale_one() {
        // cr2 holds whatever the last page fault left there. Printing it for a
        // divide-by-zero would send someone hunting a pointer that is not the
        // problem.
        let s = line(0, 0, 0x1000, 0xdead_beef);
        assert!(!s.as_str().contains("addr="), "{}", s.as_str());
        assert!(s.as_str().contains("divide by zero"), "{}", s.as_str());
    }

    #[test]
    fn every_vector_reports_something_readable() {
        for v in 0..VECTORS as u64 {
            let s = line(v, 0, 0, 0);
            assert!(
                s.as_str().starts_with("os: FAULT "),
                "vector {v}: {}",
                s.as_str()
            );
            assert!(s.as_str().ends_with('\n'), "vector {v} has no line ending");
        }
    }

    #[test]
    fn the_error_code_vectors_match_the_manual() {
        // Getting this wrong shifts the whole stack frame, so the report would
        // print a plausible but entirely wrong rip.
        assert!(HAS_ERROR_CODE[14], "page fault pushes an error code");
        assert!(HAS_ERROR_CODE[13], "GP fault pushes an error code");
        assert!(!HAS_ERROR_CODE[6], "invalid opcode does not");
        assert!(!HAS_ERROR_CODE[0], "divide by zero does not");
    }

    #[test]
    fn the_report_never_overruns_its_buffer() {
        let mut buf = [0u8; 160];
        let n = report_line(&mut buf, u64::MAX, u64::MAX, u64::MAX, u64::MAX);
        assert!(n <= buf.len());
    }
}
