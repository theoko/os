//! A millisecond clock that also exists on aarch64.
//!
//! `serial::rdtsc()` is a constant zero off x86, which makes `beep::sleep_ms`
//! spin its whole 200-million guard on every call regardless of the argument —
//! so USB bring-up cannot be timed with it, and a controller that never answers
//! would hang boot rather than report itself. The ARM generic timer is an
//! integer-Hz counter readable at EL1, which is all a bounded wait needs and
//! costs no FPU.
//!
//! Nothing here branches on `cfg!(target_arch)` for *behaviour*: the presence
//! of a clock is a runtime fact (`have_clock`), because the machine running
//! `cargo test` is an arm64 Mac and would otherwise take the target's branch.

/// Spins per millisecond assumed when the machine has no readable time base.
///
/// Deliberately generous. A wait that ends early looks like a dead controller;
/// one that ends late costs a few milliseconds once, during boot.
const SPINS_PER_MS: u32 = 200_000;

/// A counter and its frequency, or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timebase {
    hz: u64,
}

impl Timebase {
    /// Read `CNTFRQ_EL0`. Zero means "this machine has no clock we can read",
    /// which every wait below is written to survive.
    pub fn probe() -> Self {
        Self { hz: read_cntfrq() }
    }

    /// A fixed frequency, for host tests.
    pub const fn fixed(hz: u64) -> Self {
        Self { hz }
    }

    pub const fn hz(&self) -> u64 {
        self.hz
    }

    pub const fn have_clock(&self) -> bool {
        self.hz != 0
    }

    /// Current tick, or 0 when there is no clock.
    pub fn now(&self) -> u64 {
        if self.hz == 0 {
            return 0;
        }
        read_cntvct()
    }

    /// Milliseconds to ticks. Saturating, because a caller asking for
    /// `u32::MAX` ms wants "effectively forever", not a wrapped tiny wait.
    pub const fn ms_to_ticks(&self, ms: u32) -> u64 {
        self.hz.saturating_mul(ms as u64) / 1000
    }

    pub fn deadline(&self, ms: u32) -> Deadline {
        Deadline {
            tb: *self,
            end: self.now().saturating_add(self.ms_to_ticks(ms)),
            spins: ms.saturating_mul(SPINS_PER_MS),
        }
    }
}

/// A bounded wait that works whether or not a real clock exists.
///
/// Every OHCI wait is "poll until a bit flips, or give up". With no time base a
/// tick deadline would expire instantly — hang-free but useless — and a bare
/// spin count is uncalibrated. Carrying both means the same call site is
/// correct on a machine with `CNTFRQ` and merely conservative on one without.
#[derive(Clone, Copy)]
pub struct Deadline {
    tb: Timebase,
    end: u64,
    spins: u32,
}

impl Deadline {
    /// Always terminates: with a clock it compares ticks, without one it burns
    /// a fixed budget. The spin budget is not consulted while a clock exists,
    /// or a fast machine would abandon a wait long before its deadline.
    pub fn expired(&mut self) -> bool {
        if self.tb.have_clock() {
            return self.tb.now() >= self.end;
        }
        if self.spins == 0 {
            return true;
        }
        self.spins -= 1;
        false
    }

    /// Spin once. Returns false when the wait is over.
    pub fn wait(&mut self) -> bool {
        core::hint::spin_loop();
        !self.expired()
    }
}

pub fn delay_ms(tb: &Timebase, ms: u32) {
    let mut d = tb.deadline(ms);
    while !d.expired() {
        core::hint::spin_loop();
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
fn read_cntfrq() -> u64 {
    let hz: u64;
    unsafe {
        core::arch::asm!("mrs {}, cntfrq_el0", out(reg) hz, options(nomem, nostack));
    }
    hz
}

#[cfg(all(target_arch = "aarch64", target_os = "none"))]
fn read_cntvct() -> u64 {
    let t: u64;
    // `isb` first: the counter read is otherwise free to be satisfied out of
    // order, and a wait loop that reads a stale tick never advances.
    unsafe {
        core::arch::asm!("isb", "mrs {}, cntvct_el0", out(reg) t, options(nomem, nostack));
    }
    t
}

/// Every other target — including the arm64 Mac that runs `cargo test`, which
/// is `target_os = "macos"` and must not execute a system register read inside
/// a userspace test process.
#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
fn read_cntfrq() -> u64 {
    0
}

#[cfg(not(all(target_arch = "aarch64", target_os = "none")))]
fn read_cntvct() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_millisecond_is_the_frequency_divided_by_a_thousand() {
        let tb = Timebase::fixed(24_000_000);
        assert_eq!(tb.ms_to_ticks(1), 24_000);
        assert_eq!(tb.ms_to_ticks(1000), 24_000_000);
    }

    #[test]
    fn a_machine_with_no_clock_converts_every_wait_to_zero_ticks() {
        let tb = Timebase::fixed(0);
        assert!(!tb.have_clock());
        assert_eq!(tb.ms_to_ticks(50), 0);
        assert_eq!(tb.now(), 0);
    }

    #[test]
    fn an_absurd_wait_does_not_wrap_into_a_short_one() {
        // u32::MAX ms at 1 GHz overflows u64 multiplication; wrapping would
        // turn "wait forever" into "wait a moment" and report a live
        // controller as dead.
        let tb = Timebase::fixed(1_000_000_000);
        assert_eq!(tb.ms_to_ticks(u32::MAX), u64::MAX / 1000);
    }

    #[test]
    fn a_wait_without_a_clock_ends_instead_of_hanging() {
        let tb = Timebase::fixed(0);
        let mut d = Deadline { tb, end: 0, spins: 3 };
        assert!(!d.expired());
        assert!(!d.expired());
        assert!(!d.expired());
        assert!(d.expired(), "a clockless wait must run out of budget");
    }

    #[test]
    fn a_wait_with_a_clock_ignores_the_spin_budget() {
        // The budget is a fallback, not a second deadline: on a fast machine
        // it would otherwise expire while the real deadline is far away.
        let tb = Timebase::fixed(24_000_000);
        let mut d = Deadline { tb, end: u64::MAX, spins: 0 };
        assert!(!d.expired(), "the clock says the wait is not over");
    }

    #[test]
    fn a_deadline_that_has_passed_is_over_immediately() {
        let tb = Timebase::fixed(24_000_000);
        let mut d = Deadline { tb, end: 0, spins: u32::MAX };
        assert!(d.expired());
    }
}
