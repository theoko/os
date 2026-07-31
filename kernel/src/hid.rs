//! USB HID decoding, independent of the host controller underneath it.
//!
//! Nothing here touches MMIO, volatile memory or `unsafe`: it is bytes in,
//! `Key`s and pointer positions out. That is deliberate — it is the only part
//! of the USB stack `cargo test` can prove on a machine with no USB at all,
//! and both the UHCI driver and the OHCI driver call into it rather than
//! keeping two copies of the same report layout.

use crate::keyboard::{Key, shift_char};

/// Which HID device this is, by the interface descriptor's own triple.
///
/// QEMU's cuts, which VirtualBox mirrors:
///   `usb-kbd`    subclass 1 (boot), protocol 1
///   `usb-mouse`  subclass 1 (boot), protocol 2 — relative
///   `usb-tablet` subclass 0,        protocol 0 — absolute
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HidKind {
    BootKeyboard,
    BootMouse,
    Tablet,
}

/// Absolute-tablet packet format selected during USB enumeration.
///
/// QEMU and VirtualBox expose the same logical axes but put them at different
/// byte offsets. VirtualBox inserts two wheel bytes and padding before X/Y.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabletFormat {
    Qemu,
    VirtualBox,
}

impl HidKind {
    /// SET_PROTOCOL argument. Boot protocol is what makes a keyboard knowable
    /// without a report-descriptor parser: the layout is fixed by the spec.
    /// The tablet has no boot interface, so it stays on Report protocol and
    /// the hardcoded absolute layout below.
    pub const fn protocol(self) -> u16 {
        match self {
            HidKind::Tablet => 1,
            _ => 0,
        }
    }

    pub const fn is_pointer(self) -> bool {
        matches!(self, HidKind::BootMouse | HidKind::Tablet)
    }
}

/// An interface worth binding, and the endpoint that reports on it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HidIface {
    pub kind: HidKind,
    pub iface: u8,
    /// Endpoint number, already stripped of the direction bit.
    pub ep: u8,
    pub ep_mps: u16,
    pub interval: u8,
}

pub fn classify(class: u8, subclass: u8, protocol: u8) -> Option<HidKind> {
    match (class, subclass, protocol) {
        (0x03, 0x01, 0x01) => Some(HidKind::BootKeyboard),
        (0x03, 0x01, 0x02) => Some(HidKind::BootMouse),
        (0x03, 0x00, 0x00) => Some(HidKind::Tablet),
        _ => None,
    }
}

/// Every HID interface in a CONFIGURATION descriptor that has an IN endpoint.
///
/// Lifted out of the UHCI driver, where the same walk was welded to `self` and
/// unreachable from a test. Composite devices are real — VirtualBox can put a
/// keyboard and a pointer behind one config — so this collects rather than
/// returning the first hit.
pub fn list_hid_ifaces(cfg: &[u8], out: &mut [HidIface]) -> usize {
    let mut n = 0;
    let mut i = 0usize;
    let mut pending: Option<HidIface> = None;
    while i + 1 < cfg.len() && n < out.len() {
        let len = cfg[i] as usize;
        // A zero length would loop here forever, and a descriptor claiming to
        // run past the buffer means we misread wTotalLength.
        if len < 2 || i + len > cfg.len() {
            break;
        }
        match cfg[i + 1] {
            0x04 if len >= 9 => {
                pending = classify(cfg[i + 5], cfg[i + 6], cfg[i + 7]).map(|kind| HidIface {
                    kind,
                    iface: cfg[i + 2],
                    ep: 1,
                    ep_mps: 8,
                    interval: 10,
                });
            }
            0x05 if len >= 7 => {
                if let Some(mut f) = pending {
                    let addr = cfg[i + 2];
                    // Interrupt IN only: an OUT endpoint on a keyboard is the
                    // LED pipe, and binding it would poll something that never
                    // reports.
                    if addr & 0x80 != 0 && cfg[i + 3] & 0x03 == 0x03 {
                        f.ep = addr & 0x0F;
                        let mps = u16::from_le_bytes([cfg[i + 4], cfg[i + 5]]) & 0x07FF;
                        f.ep_mps = if mps == 0 { 8 } else { mps.min(64) };
                        f.interval = cfg[i + 6];
                        out[n] = f;
                        n += 1;
                        pending = None;
                    }
                }
            }
            _ => {}
        }
        i += len;
    }
    n
}

/// The first interface of the kind asked for.
pub fn find_hid_iface(cfg: &[u8], want: HidKind) -> Option<HidIface> {
    let mut found = [HidIface {
        kind: HidKind::BootMouse,
        iface: 0,
        ep: 0,
        ep_mps: 0,
        interval: 0,
    }; 8];
    let n = list_hid_ifaces(cfg, &mut found);
    found[..n].iter().copied().find(|f| f.kind == want)
}

/// Where a pointer is and what is held down.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pointer {
    pub x: i32,
    pub y: i32,
    /// Bit 0 left, bit 1 right, bit 2 middle — the same shape `mouse.rs` uses
    /// and the only thing the UI tests (`buttons & 0x01`).
    pub buttons: u8,
}

/// Axis range of an absolute report.
///
/// Kept as a value rather than a constant so that if VirtualBox's tablet turns
/// out to use a different logical maximum, the fix is filling in a report
/// descriptor parser behind this type instead of rewriting the decoder.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AbsLayout {
    pub x_max: i32,
    pub y_max: i32,
}

impl AbsLayout {
    pub const QEMU_TABLET: Self = Self {
        x_max: 32767,
        y_max: 32767,
    };
}

/// `[buttons, x_lo, x_hi, y_lo, y_hi, wheel]` in a fixed logical range.
///
/// Integer scale, multiply before divide: the kernel never enables the FPU, and
/// 32767 × 3839 still fits an i32 with room to spare.
pub fn decode_abs(report: &[u8], layout: AbsLayout, w: i32, h: i32) -> Option<Pointer> {
    if report.len() < 6 || layout.x_max <= 0 || layout.y_max <= 0 {
        return None;
    }
    decode_abs_axes(
        report[0],
        u16::from_le_bytes([report[1], report[2]]),
        u16::from_le_bytes([report[3], report[4]]),
        layout,
        w,
        h,
    )
}

/// Decode the packet layout advertised by the enumerated tablet.
///
/// VirtualBox's packed report is:
/// `[buttons, wheel-y, wheel-x, padding, x_lo, x_hi, y_lo, y_hi]`.
pub fn decode_tablet(
    report: &[u8],
    format: TabletFormat,
    layout: AbsLayout,
    w: i32,
    h: i32,
) -> Option<Pointer> {
    match format {
        TabletFormat::Qemu => decode_abs(report, layout, w, h),
        TabletFormat::VirtualBox => {
            if report.len() < 8 {
                return None;
            }
            decode_abs_axes(
                report[0],
                u16::from_le_bytes([report[4], report[5]]),
                u16::from_le_bytes([report[6], report[7]]),
                layout,
                w,
                h,
            )
        }
    }
}

fn decode_abs_axes(
    buttons: u8,
    x: u16,
    y: u16,
    layout: AbsLayout,
    w: i32,
    h: i32,
) -> Option<Pointer> {
    if layout.x_max <= 0 || layout.y_max <= 0 {
        return None;
    }
    let ax = i32::from(x).clamp(0, layout.x_max);
    let ay = i32::from(y).clamp(0, layout.y_max);
    let last_x = (w - 1).max(0);
    let last_y = (h - 1).max(0);
    Some(Pointer {
        x: (ax * last_x) / layout.x_max,
        y: (ay * last_y) / layout.y_max,
        buttons: buttons & 0x07,
    })
}

/// `[buttons, dx, dy, wheel]`, signed and relative.
///
/// HID counts +y **downward**, the opposite of the PS/2 protocol (`mouse.rs`
/// subtracts dy for exactly this reason). Getting it backwards is silent: the
/// pointer simply moves the wrong way.
pub fn decode_rel(report: &[u8], prev: Pointer, w: i32, h: i32) -> Option<Pointer> {
    if report.len() < 3 {
        return None;
    }
    let dx = report[1] as i8 as i32;
    let dy = report[2] as i8 as i32;
    Some(Pointer {
        x: (prev.x + dx).clamp(0, (w - 1).max(0)),
        y: (prev.y + dy).clamp(0, (h - 1).max(0)),
        buttons: report[0] & 0x07,
    })
}

const QUEUE: usize = 12;

/// Stateful 8-byte keyboard boot-report decoder.
///
/// A boot keyboard reports *state*, not make/break: modifier byte, one
/// reserved byte, then six concurrent usages. Presses are the set difference
/// against the previous report — without the diff a held key retypes on every
/// poll, and the main loop spins far faster than a person types.
pub struct Keyboard {
    previous: [u8; 6],
    queue: [Key; QUEUE],
    head: usize,
    tail: usize,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            previous: [0; 6],
            queue: [Key::Escape; QUEUE],
            head: 0,
            tail: 0,
        }
    }

    /// Queue every newly pressed usage without consuming any of them.
    ///
    /// A host controller retires reports independently from the UI. Keeping
    /// enqueue and dequeue separate lets its polling path accept a whole
    /// report now and lets the view drain it later without losing the first
    /// key.
    pub fn feed_report(&mut self, report: &[u8]) {
        if report.len() < 8 {
            return;
        }
        let Ok(now): Result<[u8; 6], _> = report[2..8].try_into() else {
            return;
        };
        // 0x01..=0x03 are rollover / POST-fail codes filling every slot, not
        // keys. Decoding them would type six characters nobody pressed.
        if now.iter().any(|u| (0x01..=0x03).contains(u)) {
            self.previous = now;
            return;
        }
        let shift = report[0] & 0x22 != 0;
        for usage in now {
            if usage != 0 && !self.previous.contains(&usage) {
                if let Some(k) = usage_to_key(usage, shift) {
                    self.push(k);
                }
            }
        }
        self.previous = now;
    }

    /// Queue every newly pressed usage and return the first.
    ///
    /// Kept for the existing one-key-per-report callers and tests.
    pub fn feed(&mut self, report: &[u8]) -> Option<Key> {
        self.feed_report(report);
        self.next_key()
    }

    /// One key per call, `None` when drained — the same contract as
    /// `keyboard::Keyboard::poll`, so `main.rs`'s drains are unchanged.
    pub fn next_key(&mut self) -> Option<Key> {
        if self.head == self.tail {
            return None;
        }
        let k = self.queue[self.head];
        self.head = (self.head + 1) % QUEUE;
        Some(k)
    }

    fn push(&mut self, k: Key) {
        let next = (self.tail + 1) % QUEUE;
        // Full means the UI is not draining. Drop the newest rather than
        // advancing head: overwriting the oldest would reorder what was typed.
        if next == self.head {
            return;
        }
        self.queue[self.tail] = k;
        self.tail = next;
    }
}

/// Relative HID mouse report (buttons, dx, dy). Extra wheel bytes are ignored.
pub fn mouse_report(report: &[u8]) -> Option<(u8, i8, i8)> {
    (report.len() >= 3).then(|| (report[0] & 0x07, report[1] as i8, report[2] as i8))
}

/// HID usage → the key the UI understands.
///
/// `shift` comes from the report's modifier byte (LeftShift 0x02 | RightShift
/// 0x20), not from a tracked press and release, so the PS/2 driver's sticky
/// `shift` field has no analogue here. Shifted forms come from
/// `keyboard::shift_char` so the two input paths cannot drift apart.
pub fn usage_to_key(usage: u8, shift: bool) -> Option<Key> {
    let c = match usage {
        0x04..=0x1D => b'a' + (usage - 0x04),
        0x1E..=0x26 => b'1' + (usage - 0x1E),
        0x27 => b'0',
        0x28 => return Some(Key::Enter),
        0x29 => return Some(Key::Escape),
        0x2A => return Some(Key::Backspace),
        0x2B => return Some(Key::Tab),
        0x2C => b' ',
        0x2D => b'-',
        0x2E => b'=',
        0x2F => b'[',
        0x30 => b']',
        // 0x31 backslash and 0x32 non-US '#' sit on the same physical key on
        // most layouts; folding them means an ISO keyboard still types a pipe.
        0x31 | 0x32 => b'\\',
        0x33 => b';',
        0x34 => b'\'',
        0x35 => b'`',
        0x36 => b',',
        0x37 => b'.',
        0x38 => b'/',
        0x4A => return Some(Key::Home),
        0x4B => return Some(Key::PageUp),
        0x4D => return Some(Key::End),
        0x4E => return Some(Key::PageDown),
        // 0x4F and 0x50 are Right and Left, which this UI has no use for.
        0x51 => return Some(Key::Down),
        0x52 => return Some(Key::Up),
        _ => return None,
    };
    Some(Key::Char(if shift { shift_char(c) } else { c }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kbd_report(mods: u8, usages: [u8; 6]) -> [u8; 8] {
        let mut r = [0u8; 8];
        r[0] = mods;
        r[2..8].copy_from_slice(&usages);
        r
    }

    #[test]
    fn holding_a_key_types_it_once_not_every_frame() {
        let mut k = Keyboard::new();
        assert_eq!(
            k.feed(&kbd_report(0, [0x04, 0, 0, 0, 0, 0])),
            Some(Key::Char(b'a'))
        );
        assert_eq!(k.feed(&kbd_report(0, [0x04, 0, 0, 0, 0, 0])), None);
        assert_eq!(k.feed(&kbd_report(0, [0x04, 0, 0, 0, 0, 0])), None);
    }

    #[test]
    fn releasing_and_repressing_types_it_again() {
        let mut k = Keyboard::new();
        k.feed(&kbd_report(0, [0x04, 0, 0, 0, 0, 0]));
        assert_eq!(k.feed(&kbd_report(0, [0; 6])), None);
        assert_eq!(
            k.feed(&kbd_report(0, [0x04, 0, 0, 0, 0, 0])),
            Some(Key::Char(b'a'))
        );
    }

    #[test]
    fn six_keys_in_one_report_all_arrive() {
        let mut k = Keyboard::new();
        let first = k.feed(&kbd_report(0, [0x04, 0x05, 0x06, 0x07, 0x08, 0x09]));
        assert_eq!(first, Some(Key::Char(b'a')));
        let rest: [Option<Key>; 5] = core::array::from_fn(|_| k.next_key());
        assert_eq!(
            rest,
            [
                Some(Key::Char(b'b')),
                Some(Key::Char(b'c')),
                Some(Key::Char(b'd')),
                Some(Key::Char(b'e')),
                Some(Key::Char(b'f')),
            ]
        );
        assert_eq!(k.next_key(), None, "the queue must drain empty");
    }

    #[test]
    fn a_rollover_report_types_nothing() {
        // Every slot holds 0x01 when more keys are down than the device can
        // report. Decoding it would emit six characters nobody pressed.
        let mut k = Keyboard::new();
        assert_eq!(k.feed(&kbd_report(0, [0x01; 6])), None);
    }

    #[test]
    fn shift_comes_from_the_modifier_byte_not_a_press_event() {
        let mut k = Keyboard::new();
        // The shift key itself never occupies a usage slot.
        assert_eq!(k.feed(&kbd_report(0x02, [0; 6])), None);
        assert_eq!(
            k.feed(&kbd_report(0x02, [0x05, 0, 0, 0, 0, 0])),
            Some(Key::Char(b'B'))
        );
        // Right shift is a different bit and must work the same.
        k.feed(&kbd_report(0, [0; 6]));
        assert_eq!(
            k.feed(&kbd_report(0x20, [0x1E, 0, 0, 0, 0, 0])),
            Some(Key::Char(b'!'))
        );
    }

    #[test]
    fn the_key_queue_wraps_without_reordering_or_repeating() {
        let mut k = Keyboard::new();
        let mut got = std::vec::Vec::new();
        // Twelve slots, so four six-key reports overrun it twice over.
        for round in 0..4u8 {
            let base = 0x04 + round * 6;
            if let Some(Key::Char(c)) = k.feed(&kbd_report(
                0,
                [base, base + 1, base + 2, base + 3, base + 4, base + 5],
            )) {
                got.push(c);
            }
            k.feed(&kbd_report(0, [0; 6]));
        }
        while let Some(Key::Char(c)) = k.next_key() {
            got.push(c);
        }
        assert!(
            got.len() <= QUEUE + 1,
            "handed back more than it can hold: {got:?}"
        );
        assert!(
            got.windows(2).all(|w| w[0] < w[1]),
            "typing order must survive: {got:?}"
        );
    }

    #[test]
    fn every_key_variant_the_ui_consumes_has_a_usage() {
        // main.rs handles all of these; a half-finished table silently drops
        // one and the reader stops scrolling with no error anywhere.
        for want in [
            Key::Enter,
            Key::Escape,
            Key::Backspace,
            Key::Tab,
            Key::Up,
            Key::Down,
            Key::PageUp,
            Key::PageDown,
            Key::Home,
            Key::End,
        ] {
            let found = (0u8..=0xFF).any(|u| usage_to_key(u, false) == Some(want));
            assert!(found, "{want:?} is unreachable from any HID usage");
        }
        assert_eq!(usage_to_key(0x04, false), Some(Key::Char(b'a')));
    }

    #[test]
    fn the_arrow_usages_are_not_swapped_with_their_neighbours() {
        // 0x4F/0x50 are Right/Left; using them for Down/Up scrolls the reader
        // when the user presses sideways and does nothing when they press up.
        assert_eq!(usage_to_key(0x52, false), Some(Key::Up));
        assert_eq!(usage_to_key(0x51, false), Some(Key::Down));
        assert_eq!(usage_to_key(0x4F, false), None);
        assert_eq!(usage_to_key(0x50, false), None);
    }

    #[test]
    fn punctuation_types_the_character_on_the_key() {
        for (usage, plain, shifted) in [
            (0x2Du8, b'-', b'_'),
            (0x2E, b'=', b'+'),
            (0x36, b',', b'<'),
            (0x38, b'/', b'?'),
        ] {
            assert_eq!(usage_to_key(usage, false), Some(Key::Char(plain)));
            assert_eq!(usage_to_key(usage, true), Some(Key::Char(shifted)));
        }
    }

    #[test]
    fn mouse_requires_a_complete_boot_report() {
        assert_eq!(mouse_report(&[1, 255, 2]), Some((1, -1, 2)));
        assert_eq!(mouse_report(&[1, 2]), None);
    }

    #[test]
    fn a_relative_mouse_moves_down_when_the_report_says_down() {
        // HID counts +y downward. PS/2 counts it upward, and mouse.rs
        // subtracts for that reason; copying its sign here inverts the mouse.
        let prev = Pointer {
            x: 100,
            y: 100,
            buttons: 0,
        };
        let down = decode_rel(&[0, 0, 5, 0], prev, 640, 480).unwrap();
        assert_eq!(down.y, 105, "a positive dy must move toward the bottom");
        let up = decode_rel(&[0, 0, (-5i8) as u8, 0], prev, 640, 480).unwrap();
        assert_eq!(up.y, 95);
    }

    #[test]
    fn a_relative_mouse_cannot_be_pushed_off_screen() {
        let corner = Pointer {
            x: 0,
            y: 0,
            buttons: 0,
        };
        let p = decode_rel(&[0, (-100i8) as u8, (-100i8) as u8, 0], corner, 640, 480).unwrap();
        assert_eq!((p.x, p.y), (0, 0));
        let far = Pointer {
            x: 639,
            y: 479,
            buttons: 0,
        };
        let p = decode_rel(&[0, 100, 100, 0], far, 640, 480).unwrap();
        assert_eq!((p.x, p.y), (639, 479));
    }

    #[test]
    fn only_the_three_real_buttons_survive_a_report() {
        let p = decode_rel(&[0xFF, 0, 0, 0], Pointer::default(), 640, 480).unwrap();
        assert_eq!(p.buttons, 0x07);
    }

    #[test]
    fn an_absolute_report_at_full_scale_lands_on_the_last_pixel() {
        let l = AbsLayout::QEMU_TABLET;
        let mut r = [0u8; 6];
        r[1..3].copy_from_slice(&32767u16.to_le_bytes());
        r[3..5].copy_from_slice(&32767u16.to_le_bytes());
        let p = decode_abs(&r, l, 1024, 768).unwrap();
        assert_eq!((p.x, p.y), (1023, 767));

        let zero = decode_abs(&[0u8; 6], l, 1024, 768).unwrap();
        assert_eq!((zero.x, zero.y), (0, 0));
    }

    #[test]
    fn virtualbox_tablet_reads_xy_after_wheels_and_padding() {
        // Oracle VirtualBox USBHIDT_REPORT:
        // buttons, dz, dw, padding, x (LE), y (LE).
        let report = [0x01, 0x7F, 0x81, 0, 0x00, 0x40, 0x00, 0x20];
        let p = decode_tablet(
            &report,
            TabletFormat::VirtualBox,
            AbsLayout::QEMU_TABLET,
            1280,
            800,
        )
        .unwrap();
        assert_eq!(p.buttons, 1);
        assert_eq!(p.x, 639);
        assert_eq!(p.y, 199);
    }

    #[test]
    fn virtualbox_wheel_bytes_cannot_move_the_pointer() {
        let centered = [0, 0, 0, 0, 0x00, 0x40, 0x00, 0x40];
        let wheels = [0, 127, 129, 0, 0x00, 0x40, 0x00, 0x40];
        let a = decode_tablet(
            &centered,
            TabletFormat::VirtualBox,
            AbsLayout::QEMU_TABLET,
            1280,
            800,
        )
        .unwrap();
        let b = decode_tablet(
            &wheels,
            TabletFormat::VirtualBox,
            AbsLayout::QEMU_TABLET,
            1280,
            800,
        )
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn an_absolute_report_out_of_range_is_clamped_not_wrapped() {
        let l = AbsLayout {
            x_max: 4095,
            y_max: 4095,
        };
        let mut r = [0u8; 6];
        r[1..3].copy_from_slice(&60000u16.to_le_bytes());
        r[3..5].copy_from_slice(&60000u16.to_le_bytes());
        let p = decode_abs(&r, l, 800, 600).unwrap();
        assert_eq!((p.x, p.y), (799, 599));
    }

    #[test]
    fn an_absolute_report_shorter_than_six_bytes_is_refused() {
        assert!(decode_abs(&[0u8; 5], AbsLayout::QEMU_TABLET, 800, 600).is_none());
    }

    /// A config descriptor: header, then one interface per
    /// (subclass, protocol, endpoint, max packet) with an interrupt IN.
    fn config(ifaces: &[(u8, u8, u8, u16)]) -> std::vec::Vec<u8> {
        let mut v = std::vec![9u8, 0x02, 0, 0, ifaces.len() as u8, 1, 0, 0x80, 50];
        for (i, (sub, proto, ep, mps)) in ifaces.iter().enumerate() {
            v.extend_from_slice(&[9, 0x04, i as u8, 0, 1, 0x03, *sub, *proto, 0]);
            v.extend_from_slice(&[9, 0x21, 0x11, 0x01, 0, 1, 0x22, 0x34, 0]); // HID descriptor
            v.extend_from_slice(&[7, 0x05, 0x80 | *ep, 0x03]);
            v.extend_from_slice(&mps.to_le_bytes());
            v.push(10);
        }
        let total = v.len() as u16;
        v[2..4].copy_from_slice(&total.to_le_bytes());
        v
    }

    #[test]
    fn a_boot_mouse_is_not_mistaken_for_a_tablet() {
        // The UHCI driver was bitten by the reverse: binding the first device
        // that enumerated, then reporting "ready" while nothing ever moved.
        let cfg = config(&[(1, 2, 1, 4)]);
        assert!(find_hid_iface(&cfg, HidKind::Tablet).is_none());
        let m = find_hid_iface(&cfg, HidKind::BootMouse).expect("a boot mouse is a boot mouse");
        assert_eq!((m.ep, m.ep_mps), (1, 4));
    }

    #[test]
    fn a_config_with_a_keyboard_and_a_tablet_returns_the_one_asked_for() {
        let cfg = config(&[(1, 1, 1, 8), (0, 0, 2, 6)]);
        let k = find_hid_iface(&cfg, HidKind::BootKeyboard).unwrap();
        let t = find_hid_iface(&cfg, HidKind::Tablet).unwrap();
        assert_eq!((k.iface, k.ep), (0, 1));
        assert_eq!((t.iface, t.ep), (1, 2));
    }

    #[test]
    fn a_descriptor_whose_length_is_zero_terminates_the_walk() {
        // Otherwise the walk never advances and enumeration hangs the boot
        // with nothing printed anywhere.
        let mut cfg = config(&[(1, 1, 1, 8)]);
        cfg[9] = 0;
        let mut out = [HidIface {
            kind: HidKind::Tablet,
            iface: 0,
            ep: 0,
            ep_mps: 0,
            interval: 0,
        }; 4];
        assert_eq!(list_hid_ifaces(&cfg, &mut out), 0);
    }

    #[test]
    fn a_descriptor_running_past_the_buffer_is_not_read_past_it() {
        let cfg = config(&[(1, 1, 1, 8)]);
        let mut out = [HidIface {
            kind: HidKind::Tablet,
            iface: 0,
            ep: 0,
            ep_mps: 0,
            interval: 0,
        }; 4];
        // Truncated mid-endpoint: we misread wTotalLength, so bail rather than
        // decode whatever happens to follow in the buffer.
        assert_eq!(list_hid_ifaces(&cfg[..cfg.len() - 3], &mut out), 0);
    }

    #[test]
    fn a_non_hid_interface_is_ignored() {
        let mut cfg = config(&[(1, 1, 1, 8)]);
        cfg[9 + 5] = 0x08; // mass storage
        assert!(find_hid_iface(&cfg, HidKind::BootKeyboard).is_none());
    }

    #[test]
    fn the_tablet_stays_on_report_protocol_and_the_keyboard_goes_to_boot() {
        assert_eq!(HidKind::Tablet.protocol(), 1);
        assert_eq!(HidKind::BootKeyboard.protocol(), 0);
        assert_eq!(HidKind::BootMouse.protocol(), 0);
        assert!(HidKind::BootMouse.is_pointer() && !HidKind::BootKeyboard.is_pointer());
    }

    #[test]
    fn classify_unknown_class_subclass_returns_none() {
        assert_eq!(classify(0x00, 0x00, 0x00), None);
        assert_eq!(classify(0xFF, 0x01, 0x01), None);
        assert_eq!(classify(0x03, 0xFF, 0x01), None);
    }

    #[test]
    fn decode_rel_short_slice_returns_none() {
        assert!(decode_rel(&[], Pointer::default(), 640, 480).is_none());
        assert!(decode_rel(&[0, 0], Pointer::default(), 640, 480).is_none());
    }

    #[test]
    fn decode_vbox_tablet_short_report_returns_none() {
        assert!(decode_tablet(
            &[0u8; 7],
            TabletFormat::VirtualBox,
            AbsLayout::QEMU_TABLET,
            1280,
            800
        )
        .is_none());
    }
}

