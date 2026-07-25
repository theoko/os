//! Say which input devices came up, especially when none did.
//!
//! Under QEMU this is uninteresting: the UHCI tablet and the PS/2 controller
//! are both there because the config asks for them. On real hardware it is the
//! whole story. A laptop built any time in the last fifteen years has an xHCI
//! controller and no i8042, and this kernel drives neither — so it boots to a
//! correct, fully rendered home screen with a cursor that never moves and a
//! keyboard that does nothing.
//!
//! That is indistinguishable from a hang, and it is the wrong thing to show
//! someone. A machine that cannot be driven should say which part is missing.

use crate::pci::UsbSurvey;

/// What actually came up.
#[derive(Debug, Clone, Copy)]
pub struct Inputs {
    pub ps2_keyboard: bool,
    pub ps2_mouse: bool,
    pub usb_tablet: bool,
    pub usb: UsbSurvey,
}

impl Inputs {
    pub fn can_point(&self) -> bool {
        self.usb_tablet || self.ps2_mouse
    }

    pub fn can_type(&self) -> bool {
        self.ps2_keyboard
    }

    /// True when the machine is usable at all.
    pub fn usable(&self) -> bool {
        self.can_point() || self.can_type()
    }

    /// One line for the person in front of the screen, or `None` when
    /// everything needed is present and there is nothing worth saying.
    ///
    /// Plain language and a next action: a message that only names the missing
    /// controller tells someone with a dead mouse nothing they can act on.
    pub fn note(&self) -> Option<&'static str> {
        match (self.can_point(), self.can_type()) {
            (true, true) => None,
            (false, false) => Some(if self.usb.has_unsupported() {
                "No usable keyboard or mouse. This machine's USB is xHCI, which this OS cannot drive yet."
            } else {
                "No keyboard or mouse detected. Try a wired USB keyboard, or run this in UTM."
            }),
            (false, true) => Some(if self.usb.has_unsupported() {
                "Keyboard only - the pointer needs xHCI, which this OS cannot drive yet. Use Tab and Enter."
            } else {
                "Keyboard only - no pointer detected. Use Tab and Enter."
            }),
            (true, false) => Some("Pointer only - no keyboard detected. Typing will not work."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(ps2_kbd: bool, ps2_mouse: bool, tablet: bool, usb: UsbSurvey) -> Inputs {
        Inputs { ps2_keyboard: ps2_kbd, ps2_mouse, usb_tablet: tablet, usb }
    }

    #[test]
    fn a_working_machine_says_nothing() {
        // A banner on every boot would be noise, and noise gets ignored
        // exactly when it finally matters.
        let good = inputs(true, false, true, UsbSurvey { uhci: 1, ..Default::default() });
        assert_eq!(good.note(), None);
    }

    #[test]
    fn a_modern_laptop_is_told_why_nothing_responds() {
        let laptop = inputs(false, false, false, UsbSurvey { xhci: 2, ..Default::default() });
        assert!(!laptop.usable());
        let note = laptop.note().expect("an unusable machine must say so");
        assert!(note.contains("xHCI"), "{note}");
    }

    #[test]
    fn a_missing_pointer_suggests_the_keys_that_still_work() {
        let kbd_only = inputs(true, false, false, UsbSurvey::default());
        let note = kbd_only.note().expect("a dead pointer is worth mentioning");
        assert!(note.contains("Tab"), "offer the way through: {note}");
    }

    #[test]
    fn a_machine_with_no_usb_at_all_does_not_blame_xhci() {
        // Blaming a controller that is not there sends someone chasing the
        // wrong problem.
        let bare = inputs(false, false, false, UsbSurvey::default());
        let note = bare.note().unwrap();
        assert!(!note.contains("xHCI"), "{note}");
    }

    #[test]
    fn either_input_alone_still_counts_as_usable() {
        assert!(inputs(true, false, false, UsbSurvey::default()).usable());
        assert!(inputs(false, true, false, UsbSurvey::default()).usable());
        assert!(!inputs(false, false, false, UsbSurvey::default()).usable());
    }
}
