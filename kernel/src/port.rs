//! x86 port I/O used by serial, PIT, PS/2, PCI, and UHCI.

#[cfg(target_arch = "x86_64")]
mod arch {
    use core::arch::asm;

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
}

#[cfg(target_arch = "x86_64")]
pub use arch::*;

#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn outb(_port: u16, _val: u8) {}
#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn inb(_port: u16) -> u8 {
    0
}
#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn outw(_port: u16, _val: u16) {}
#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn inw(_port: u16) -> u16 {
    0
}
#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn outl(_port: u16, _val: u32) {}
#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn inl(_port: u16) -> u32 {
    0
}
