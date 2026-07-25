//! PC speaker — the startup chime.
//!
//! Square-wave tones via PIT channel 2 gated through port 0x61. It is the one
//! sound device available with no driver stack, no DMA and no allocator, which
//! makes it the right fit for a boot jingle in a freestanding kernel.
//!
//! The melody here is original. A recognisable pop song would be someone
//! else's copyright baked into a build artifact in a public repo — see
//! `docs/` for pointing this at your own licensed audio instead.

use crate::anim;
use crate::port;

const PIT_CMD: u16 = 0x43;
const PIT_CH2: u16 = 0x42;
const SPEAKER: u16 = 0x61;

/// PIT input clock.
const PIT_HZ: u32 = 1_193_182;


/// PIT divisor for a frequency, clamped to what the 16-bit counter can hold.
///
/// A divisor of 0 would mean "65536" and a silent/garbage tone, so the low end
/// is clamped to 1.
pub fn divisor_for(hz: u32) -> u16 {
    if hz == 0 {
        return u16::MAX;
    }
    let d = PIT_HZ / hz;
    d.clamp(1, u16::MAX as u32) as u16
}

/// One note. `hz == 0` is a rest.
#[derive(Clone, Copy)]
pub struct Note {
    pub hz: u32,
    pub ms: u32,
}

/// Startup chime: an original rising figure that resolves up an octave.
///
/// Kept under a second so it never delays the first frame noticeably.
pub const STARTUP: [Note; 6] = [
    Note { hz: 523, ms: 90 },  // C5
    Note { hz: 659, ms: 90 },  // E5
    Note { hz: 784, ms: 90 },  // G5
    Note { hz: 0, ms: 40 },    // breath
    Note { hz: 1047, ms: 160 }, // C6
    Note { hz: 0, ms: 30 },
];

fn tone_on(hz: u32) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let div = divisor_for(hz);
        // Channel 2, lobyte/hibyte, square wave (mode 3).
        port::outb(PIT_CMD, 0xB6);
        port::outb(PIT_CH2, (div & 0xFF) as u8);
        port::outb(PIT_CH2, (div >> 8) as u8);
        // Gate the counter and connect it to the speaker.
        let v = port::inb(SPEAKER);
        port::outb(SPEAKER, v | 0x03);
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = hz;
}

fn tone_off() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let v = port::inb(SPEAKER);
        port::outb(SPEAKER, v & !0x03);
    }
}

/// Play a sequence, leaving the speaker off afterwards.
pub fn play(notes: &[Note]) {
    for n in notes {
        if n.hz == 0 {
            tone_off();
        } else {
            tone_on(n.hz);
        }
        let _ = anim::pace(crate::serial::rdtsc(), n.ms.saturating_mul(1000));
    }
    tone_off();
}

/// The boot jingle.
pub fn startup() {
    play(&STARTUP);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divisor_matches_the_pit_clock() {
        // 1193182 / 1000 Hz
        assert_eq!(divisor_for(1000), 1193);
        assert_eq!(divisor_for(523), (PIT_HZ / 523) as u16);
    }

    #[test]
    fn divisor_never_returns_zero() {
        // A zero divisor means 65536 on the PIT — a silent or garbage tone.
        for hz in [1, 100, 1000, 20_000, 1_000_000, u32::MAX] {
            assert!(divisor_for(hz) >= 1, "hz={hz} produced a zero divisor");
        }
    }

    #[test]
    fn silence_is_handled() {
        assert_eq!(divisor_for(0), u16::MAX);
    }

    #[test]
    fn startup_is_short_enough_not_to_delay_boot() {
        let total: u32 = STARTUP.iter().map(|n| n.ms).sum();
        assert!(total <= 1000, "chime runs {total}ms");
    }

    #[test]
    fn startup_notes_are_audible_or_deliberate_rests() {
        for n in STARTUP {
            assert!(n.ms > 0);
            // Human hearing, roughly; anything outside is a bug not a note.
            assert!(n.hz == 0 || (100..=8000).contains(&n.hz), "{} Hz", n.hz);
        }
    }

    #[test]
    fn startup_ends_silent() {
        assert_eq!(STARTUP.last().unwrap().hz, 0, "speaker must be left off");
    }
}
