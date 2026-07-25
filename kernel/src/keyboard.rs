//! PS/2 keyboard (scancode set 1) — enough to type a query.
//!
//! Shares the i8042 with the PS/2 mouse, so this only consumes bytes when the
//! controller's AUX flag is clear. Under UTM the pointer is a USB tablet, so
//! nothing else is draining port 0x60.

const DATA: u16 = 0x60;
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
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Scancode set 1, unshifted. Index = make code. 0 means "no character".
const MAP: [u8; 0x40] = [
    0, 0, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 0, 0,
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', 0, 0,
    b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`', 0, b'\\',
    b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0, b'*', 0, b' ', 0, 0,
    0, 0, 0, 0,
];

/// Shifted forms for the printable keys we map.
fn shift_char(c: u8) -> u8 {
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
    /// Set by the 0xE0 prefix. Navigation keys arrive this way, and the same
    /// make codes mean digits on the keypad without it — so the prefix has to
    /// be tracked, not just swallowed.
    extended: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self { shift: false, extended: false }
    }

    /// Decode one scancode byte. Returns a key on a *press*, never a release.
    pub fn feed(&mut self, code: u8) -> Option<Key> {
        if code == 0xE0 {
            self.extended = true;
            return None;
        }
        let extended = core::mem::replace(&mut self.extended, false);

        // Break codes have bit 7 set; only shift needs its release tracked.
        if code & 0x80 != 0 {
            let make = code & 0x7F;
            if make == 0x2A || make == 0x36 {
                self.shift = false;
            }
            return None;
        }

        match code {
            0x2A | 0x36 => {
                self.shift = true;
                None
            }
            0x0E => Some(Key::Backspace),
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
                Some(Key::Char(if self.shift { shift_char(c) } else { c }))
            }
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
        }
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
        Self { buf: [0; N], len: 0 }
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
            Key::Backspace => {
                if self.len == 0 {
                    return false;
                }
                self.len -= 1;
                true
            }
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
    fn extended_keys_are_ignored_not_mistyped() {
        let mut k = kb();
        assert_eq!(k.feed(0xE0), None);
        // 0x4B would be keypad '4' unshifted; as an E0 prefix it's Left Arrow.
        assert_eq!(k.feed(0x4B), None, "arrow key must not insert a character");
        // The prefix must not persist.
        assert_eq!(k.feed(0x1E), Some(Key::Char(b'a')));
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
        for key in [Key::Up, Key::Down, Key::PageUp, Key::PageDown, Key::Home, Key::End] {
            assert!(!f.apply(key), "{key:?} was treated as text");
        }
        assert!(f.is_empty());
    }
}
