//! PS/2 keyboard (scancode set 1) — enough to type a query.
//!
//! Shares the i8042 with the PS/2 mouse, so this only consumes bytes when the
//! controller's AUX flag is clear. Under UTM the pointer is a USB tablet, so
//! nothing else is draining port 0x60.

#[allow(dead_code)]
const DATA: u16 = 0x60;
#[allow(dead_code)]
const STATUS: u16 = 0x64;

#[cfg(target_arch = "x86_64")]
mod port {
    use core::arch::asm;

    #[inline]
    pub unsafe fn inb(port: u16) -> u8 {
        let val: u8;
        unsafe {
            asm!("in al, dx", out("al") val, in("dx") port, options(nostack, preserves_flags));
        }
        val
    }
}

/// What a keypress produced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(u8),
    Backspace,
    Enter,
    Escape,
    /// Moves the focus ring during setup. `inputdiag::note()` has always told
    /// people to "Use Tab and Enter"; until this existed that was advice
    /// nothing implemented.
    Tab,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    /// Ctrl+Shift+letter, carrying the uppercase ASCII letter.
    ///
    /// The payload is deliberately not a `Char`: a chord is a command, and
    /// `TextField` must never treat it as something to insert.
    Chord(u8),
}

/// Scancode set 1, unshifted. Index = make code. 0 means "no character".
const MAP: [u8; 0x40] = [
    0, 0, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 0, 0, b'q', b'w',
    b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', 0, 0, b'a', b's', b'd', b'f', b'g',
    b'h', b'j', b'k', b'l', b';', b'\'', b'`', 0, b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm',
    b',', b'.', b'/', 0, b'*', 0, b' ', 0, 0, 0, 0, 0, 0,
];

/// Shifted forms for the printable keys we map.
///
/// Scancode-independent, ASCII in and ASCII out, so the USB HID decoder uses
/// the same table rather than growing a second one that can drift.
pub(crate) fn shift_char(c: u8) -> u8 {
    match c {
        b'a'..=b'z' => c - 32,
        b'1' => b'!',
        b'2' => b'@',
        b'3' => b'#',
        b'4' => b'$',
        b'5' => b'%',
        b'6' => b'^',
        b'7' => b'&',
        b'8' => b'*',
        b'9' => b'(',
        b'0' => b')',
        b'-' => b'_',
        b'=' => b'+',
        b'[' => b'{',
        b']' => b'}',
        b';' => b':',
        b'\'' => b'"',
        b'`' => b'~',
        b'\\' => b'|',
        b',' => b'<',
        b'.' => b'>',
        b'/' => b'?',
        other => other,
    }
}

pub struct Keyboard {
    shift: bool,
    /// Left Ctrl is make `0x1D` / break `0x9D`; right Ctrl is the same pair
    /// behind an `0xE0` prefix. Tracked so a chord can be told apart from
    /// typing — a Ctrl'd letter is a command and must never reach a field.
    ctrl: bool,
    /// Set by the 0xE0 prefix. Navigation keys arrive this way, and the same
    /// make codes mean digits on the keypad without it — so the prefix has to
    /// be tracked, not just swallowed.
    extended: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            shift: false,
            ctrl: false,
            extended: false,
        }
    }

    /// Is Ctrl held right now?
    pub fn ctrl_held(&self) -> bool {
        self.ctrl
    }

    /// Is Shift held right now?
    pub fn shift_held(&self) -> bool {
        self.shift
    }

    /// Decode one scancode byte. Returns a key on a *press*, never a release.
    pub fn feed(&mut self, code: u8) -> Option<Key> {
        if code == 0xE0 {
            self.extended = true;
            return None;
        }
        let extended = core::mem::replace(&mut self.extended, false);

        // Break codes have bit 7 set; only the modifiers need their release
        // tracked. Ctrl's break is the same make code either side of the 0xE0
        // prefix, so the left and right keys are handled by one arm.
        if code & 0x80 != 0 {
            let make = code & 0x7F;
            if make == 0x2A || make == 0x36 {
                self.shift = false;
            } else if make == 0x1D {
                self.ctrl = false;
            }
            return None;
        }

        match code {
            0x2A | 0x36 => {
                self.shift = true;
                None
            }
            // Left Ctrl bare, right Ctrl behind 0xE0. Both set the same flag.
            0x1D => {
                self.ctrl = true;
                None
            }
            0x0E => Some(Key::Backspace),
            0x0F => Some(Key::Tab),
            0x1C => Some(Key::Enter),
            0x01 => Some(Key::Escape),
            // Navigation, only in their extended form.
            0x48 if extended => Some(Key::Up),
            0x50 if extended => Some(Key::Down),
            0x49 if extended => Some(Key::PageUp),
            0x51 if extended => Some(Key::PageDown),
            0x47 if extended => Some(Key::Home),
            0x4F if extended => Some(Key::End),
            _ if extended => None,
            _ => {
                let c = *MAP.get(code as usize)?;
                if c == 0 {
                    return None;
                }
                if self.ctrl {
                    // Ctrl never types. With Shift it is a command chord;
                    // alone (or on a non-letter) it produces nothing at all.
                    return match (self.shift, c) {
                        (true, b'a'..=b'z') => Some(Key::Chord(c - 32)),
                        _ => None,
                    };
                }
                Some(Key::Char(if self.shift { shift_char(c) } else { c }))
            }
        }
    }

    /// Translate a byte from a serial console into the same UI key type used
    /// by the PS/2 path. ARM virtual machines can expose a PL011 console long
    /// before USB/xHCI input exists, so this deliberately accepts only the
    /// portable terminal subset rather than pretending serial sends PC scan
    /// codes.
    pub fn from_serial(byte: u8) -> Option<Key> {
        match byte {
            b'\r' | b'\n' => Some(Key::Enter),
            0x08 | 0x7f => Some(Key::Backspace),
            0x1b => Some(Key::Escape),
            // A terminal collapses Ctrl+Shift+P and Ctrl+P into the same
            // control byte, so this is the only faithful mapping. Without it
            // the chord would be unreachable on aarch64, where `poll` has no
            // PS/2 controller to read scan codes from.
            0x10 => Some(Key::Chord(b'P')),
            0x20..=0x7e => Some(Key::Char(byte)),
            _ => None,
        }
    }

    /// Is there an i8042 controller at all?
    ///
    /// A machine without one floats the bus, so every read comes back 0xFF.
    /// This is worth asking on real hardware: modern laptops route the built-in
    /// keyboard over USB or I2C-HID and have no i8042 to find, and assuming one
    /// is present means the UI silently claims a keyboard that is not there.
    pub fn present() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            unsafe { port::inb(STATUS) != 0xFF }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Drain pending keyboard bytes from the i8042.
    ///
    /// Leaves AUX (mouse) bytes alone so a PS/2 pointer keeps working.
    pub fn poll(&mut self) -> Option<Key> {
        #[cfg(target_arch = "x86_64")]
        {
            for _ in 0..32 {
                let st = unsafe { port::inb(STATUS) };
                if st & 0x01 == 0 {
                    return None;
                }
                if st & 0x20 != 0 {
                    // Belongs to the mouse; leave it queued.
                    return None;
                }
                let code = unsafe { port::inb(DATA) };
                if let Some(k) = self.feed(code) {
                    return Some(k);
                }
            }
            None
        }
        #[cfg(target_arch = "aarch64")]
        {
            // PL011 COM1 is a deliberate fallback path, not a substitute for
            // the eventual USB HID driver. It makes the guest interactive in
            // QEMU/VirtualBox configurations that redirect COM1 to a host
            // terminal, without waiting for xHCI endpoint rings and GIC IRQs.
            crate::serial::Serial::com1()
                .try_read_byte()
                .and_then(Self::from_serial)
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        None
    }
}

/// A fixed-capacity ASCII text field.
pub struct TextField<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> TextField<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            Some(self.buf[self.len])
        }
    }

    /// Delete the last word (trailing spaces then non-spaces). Returns true if text changed.
    pub fn delete_word(&mut self) -> bool {
        if self.len == 0 {
            return false;
        }
        let old_len = self.len;
        // Trim trailing spaces first
        while self.len > 0 && self.buf[self.len - 1] == b' ' {
            self.len -= 1;
        }
        // Trim non-space characters
        while self.len > 0 && self.buf[self.len - 1] != b' ' {
            self.len -= 1;
        }
        self.len != old_len
    }

    /// Apply a key. Returns true when the text changed.
    pub fn apply(&mut self, key: Key) -> bool {
        match key {
            Key::Char(c) => {
                // The font atlas only covers printable ASCII.
                if !(0x20..=0x7E).contains(&c) || self.len >= N {
                    return false;
                }
                self.buf[self.len] = c;
                self.len += 1;
                true
            }
            Key::Backspace => self.pop().is_some(),
            Key::Escape | Key::Chord(b'U') => {
                if self.len == 0 {
                    false
                } else {
                    self.clear();
                    true
                }
            }
            Key::Chord(b'W') => self.delete_word(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb() -> Keyboard {
        Keyboard::new()
    }

    #[test]
    fn decodes_letters() {
        let mut k = kb();
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'a')));
        assert_eq!(k.feed(0x20), Some(Key::Char(b'd')));
    }

    #[test]
    fn release_codes_produce_nothing() {
        let mut k = kb();
        assert_eq!(k.feed(0x1E | 0x80), None, "key release must not type");
    }

    #[test]
    fn shift_capitalises_and_releases() {
        let mut k = kb();
        assert_eq!(k.feed(0x2A), None, "shift itself types nothing");
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'A')));
        assert_eq!(k.feed(0x2A | 0x80), None);
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'a')), "shift must not stick");
    }

    #[test]
    fn shifted_digits_are_symbols() {
        let mut k = kb();
        k.feed(0x36); // right shift
        assert_eq!(k.feed(0x02), Some(Key::Char(b'!')));
    }

    #[test]
    fn control_keys() {
        let mut k = kb();
        assert_eq!(k.feed(0x0E), Some(Key::Backspace));
        assert_eq!(k.feed(0x1C), Some(Key::Enter));
        assert_eq!(k.feed(0x01), Some(Key::Escape));
        assert_eq!(k.feed(0x39), Some(Key::Char(b' ')));
    }

    #[test]
    fn serial_console_uses_the_same_ui_keys() {
        assert_eq!(Keyboard::from_serial(b'a'), Some(Key::Char(b'a')));
        assert_eq!(Keyboard::from_serial(b'\r'), Some(Key::Enter));
        assert_eq!(Keyboard::from_serial(0x7f), Some(Key::Backspace));
        assert_eq!(Keyboard::from_serial(0x1b), Some(Key::Escape));
        assert_eq!(Keyboard::from_serial(0x00), None);
    }

    #[test]
    fn extended_keys_are_ignored_not_mistyped() {
        let mut k = kb();
        assert_eq!(k.feed(0xE0), None);
        // 0x4B would be keypad '4' unshifted; as an E0 prefix it's Left Arrow.
        assert_eq!(k.feed(0x4B), None, "arrow key must not insert a character");
        // The prefix must not persist.
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'a')));
    }

    #[test]
    fn tab_arrives_as_a_key_and_not_as_a_character() {
        // Scancode 0x0F used to fall through the map and produce nothing,
        // which is why the "Use Tab and Enter" advice went nowhere.
        let mut k = kb();
        assert_eq!(k.feed(0x0F), Some(Key::Tab));
        let mut f = TextField::<8>::new();
        assert!(!f.apply(Key::Tab), "Tab must not insert a character");
    }

    #[test]
    fn out_of_range_scancodes_are_safe() {
        let mut k = kb();
        assert_eq!(k.feed(0x7F), None, "must not index past the map");
    }

    #[test]
    fn field_types_and_deletes() {
        let mut f = TextField::<8>::new();
        for c in b"hi" {
            f.apply(Key::Char(*c));
        }
        assert_eq!(f.as_str(), "hi");
        assert!(f.apply(Key::Backspace));
        assert_eq!(f.as_str(), "h");
    }

    #[test]
    fn field_backspace_on_empty_is_a_noop() {
        let mut f = TextField::<8>::new();
        assert!(!f.apply(Key::Backspace));
        assert!(f.is_empty());
    }

    #[test]
    fn field_respects_capacity() {
        let mut f = TextField::<4>::new();
        for c in b"abcdefgh" {
            f.apply(Key::Char(*c));
        }
        assert_eq!(f.as_str(), "abcd", "overflowed its buffer");
    }

    #[test]
    fn field_rejects_unrenderable_bytes() {
        // The atlas is ASCII 0x20..=0x7E; anything else would render as '?'.
        let mut f = TextField::<8>::new();
        assert!(!f.apply(Key::Char(0x07)));
        assert!(!f.apply(Key::Char(0xC3)));
        assert!(f.is_empty());
    }

    #[test]
    fn field_clears_on_escape_or_ctrl_u() {
        let mut f = TextField::<16>::new();
        for c in b"hello world" {
            f.apply(Key::Char(*c));
        }
        assert_eq!(f.as_str(), "hello world");
        assert!(f.apply(Key::Escape));
        assert!(f.is_empty());

        for c in b"test" {
            f.apply(Key::Char(*c));
        }
        assert_eq!(f.as_str(), "test");
        assert!(f.apply(Key::Chord(b'U')));
        assert!(f.is_empty());
    }

    #[test]
    fn field_deletes_word_on_ctrl_w() {
        let mut f = TextField::<32>::new();
        for c in b"hello world test " {
            f.apply(Key::Char(*c));
        }
        assert!(f.apply(Key::Chord(b'W')));
        assert_eq!(f.as_str(), "hello world ");
        assert!(f.apply(Key::Chord(b'W')));
        assert_eq!(f.as_str(), "hello ");
        assert!(f.apply(Key::Chord(b'W')));
        assert_eq!(f.as_str(), "");
        assert!(!f.apply(Key::Chord(b'W')));
    }
}

/// Ctrl tracking and the Ctrl+Shift+letter chord.
///
/// The chord is the only way into the portal config screen, and nothing in the
/// UI hints at it — so these have to hold exactly, in both directions: the
/// chord must fire, and Ctrl must never put a character anywhere.
#[cfg(test)]
mod chord_tests {
    use super::*;

    /// Left Ctrl, left Shift, 'p'.
    const CTRL: u8 = 0x1D;
    const SHIFT: u8 = 0x2A;
    const P: u8 = 0x19;

    #[test]
    fn ctrl_shift_p_produces_the_chord() {
        let mut k = Keyboard::new();
        assert_eq!(k.feed(CTRL), None, "Ctrl itself types nothing");
        assert_eq!(k.feed(SHIFT), None, "Shift itself types nothing");
        assert_eq!(k.feed(P), Some(Key::Chord(b'P')));
    }

    #[test]
    fn the_chord_does_not_care_which_modifier_came_first() {
        let mut k = Keyboard::new();
        k.feed(SHIFT);
        k.feed(CTRL);
        assert_eq!(k.feed(P), Some(Key::Chord(b'P')));
    }

    #[test]
    fn right_ctrl_arrives_behind_the_extended_prefix() {
        // E0 1D is right Ctrl; E0 9D releases it. Missing this would leave the
        // chord working on only one half of the keyboard.
        let mut k = Keyboard::new();
        assert_eq!(k.feed(0xE0), None);
        assert_eq!(k.feed(0x1D), None);
        assert!(k.ctrl_held(), "right Ctrl did not register");
        k.feed(SHIFT);
        assert_eq!(k.feed(P), Some(Key::Chord(b'P')));
        assert_eq!(k.feed(0xE0), None);
        assert_eq!(k.feed(0x9D), None);
        assert!(!k.ctrl_held(), "right Ctrl stuck down after its break code");
    }

    #[test]
    fn plain_p_still_types_a_p() {
        let mut k = Keyboard::new();
        assert_eq!(k.feed(P), Some(Key::Char(b'p')));
    }

    #[test]
    fn shift_p_still_types_a_capital_p() {
        let mut k = Keyboard::new();
        k.feed(SHIFT);
        assert_eq!(k.feed(P), Some(Key::Char(b'P')));
    }

    #[test]
    fn ctrl_alone_types_nothing() {
        let mut k = Keyboard::new();
        k.feed(CTRL);
        for code in [P, 0x1E, 0x02, 0x39] {
            assert_eq!(k.feed(code), None, "Ctrl+{code:#x} produced a key");
        }
    }

    #[test]
    fn ctrl_shift_on_a_non_letter_is_not_a_chord() {
        // Only letters carry a chord; Ctrl+Shift+1 must not type '!' either.
        let mut k = Keyboard::new();
        k.feed(CTRL);
        k.feed(SHIFT);
        assert_eq!(k.feed(0x02), None, "Ctrl+Shift+1 leaked a character");
        assert_eq!(k.feed(0x39), None, "Ctrl+Shift+space leaked a character");
    }

    #[test]
    fn releasing_ctrl_restores_typing() {
        // A Ctrl that stuck down would make the keyboard silently stop working.
        let mut k = Keyboard::new();
        k.feed(CTRL);
        assert_eq!(k.feed(P), None);
        assert_eq!(k.feed(CTRL | 0x80), None);
        assert!(!k.ctrl_held());
        assert_eq!(k.feed(P), Some(Key::Char(b'p')), "Ctrl stuck after release");
    }

    #[test]
    fn a_chord_is_never_text() {
        let mut f = TextField::<8>::new();
        assert!(!f.apply(Key::Chord(b'P')), "the chord was treated as text");
        assert!(f.is_empty());
    }

    #[test]
    fn the_serial_console_can_reach_the_chord_too() {
        // aarch64 polls a PL011 console, not an i8042 — without this the
        // screen would be unreachable on one of the two shipped targets.
        assert_eq!(Keyboard::from_serial(0x10), Some(Key::Chord(b'P')));
        // And the existing terminal subset is untouched.
        assert_eq!(Keyboard::from_serial(b'p'), Some(Key::Char(b'p')));
        assert_eq!(Keyboard::from_serial(b'P'), Some(Key::Char(b'P')));
    }
}

#[cfg(test)]
mod nav_tests {
    use super::*;

    #[test]
    fn navigation_keys_need_the_extended_prefix() {
        let mut k = Keyboard::new();
        // Without E0, 0x48 is keypad 8 — not Up.
        assert_ne!(k.feed(0x48), Some(Key::Up));
        assert_eq!(k.feed(0xE0), None);
        assert_eq!(k.feed(0x48), Some(Key::Up));
    }

    #[test]
    fn all_navigation_keys_decode() {
        let mut k = Keyboard::new();
        for (code, want) in [
            (0x48, Key::Up),
            (0x50, Key::Down),
            (0x49, Key::PageUp),
            (0x51, Key::PageDown),
            (0x47, Key::Home),
            (0x4F, Key::End),
        ] {
            assert_eq!(k.feed(0xE0), None);
            assert_eq!(k.feed(code), Some(want), "code {code:#x}");
        }
    }

    #[test]
    fn navigation_release_types_nothing() {
        let mut k = Keyboard::new();
        k.feed(0xE0);
        assert_eq!(k.feed(0x48 | 0x80), None, "key release must not scroll");
    }

    #[test]
    fn navigation_keys_are_not_text() {
        // TextField must ignore them rather than inserting a stray character.
        let mut f = TextField::<8>::new();
        for key in [
            Key::Up,
            Key::Down,
            Key::PageUp,
            Key::PageDown,
            Key::Home,
            Key::End,
        ] {
            assert!(!f.apply(key), "{key:?} was treated as text");
        }
        assert!(f.is_empty());
    }

    #[test]
    fn text_field_backspace_and_capacity_bounds() {
        let mut f = TextField::<4>::new();
        assert!(!f.apply(Key::Backspace), "backspace on empty should return false");
        assert!(f.apply(Key::Char(b'a')));
        assert!(f.apply(Key::Char(b'b')));
        assert!(f.apply(Key::Char(b'c')));
        assert!(f.apply(Key::Char(b'd')));
        assert!(!f.apply(Key::Char(b'e')), "over-capacity char should return false");
        assert_eq!(f.as_str(), "abcd");
        assert!(f.apply(Key::Backspace));
        assert_eq!(f.as_str(), "abc");
        f.clear();
        assert!(f.is_empty());
    }

    #[test]
    fn serial_unprintable_bytes_return_none() {
        assert_eq!(Keyboard::from_serial(0x01), None);
        assert_eq!(Keyboard::from_serial(0x1F), None);
        assert_eq!(Keyboard::from_serial(0x80), None);
        assert_eq!(Keyboard::from_serial(0xFF), None);
    }

    #[test]
    fn text_field_delete_word_and_pop() {
        let mut f = TextField::<16>::new();
        assert_eq!(f.len(), 0);
        f.apply(Key::Char(b'a'));
        f.apply(Key::Char(b'b'));
        f.apply(Key::Char(b' '));
        f.apply(Key::Char(b'c'));
        f.apply(Key::Char(b'd'));
        assert_eq!(f.as_str(), "ab cd");
        assert!(f.delete_word());
        assert_eq!(f.as_str(), "ab ");
        assert_eq!(f.pop(), Some(b' '));
        assert_eq!(f.as_str(), "ab");
    }



    #[test]
    fn serial_control_key_mappings() {
        assert_eq!(Keyboard::from_serial(0x1B), Some(Key::Escape));
        assert_eq!(Keyboard::from_serial(0x08), Some(Key::Backspace));
        assert_eq!(Keyboard::from_serial(0x7F), Some(Key::Backspace));
        assert_eq!(Keyboard::from_serial(b'\n'), Some(Key::Enter));
        assert_eq!(Keyboard::from_serial(b'\r'), Some(Key::Enter));
    }
}


