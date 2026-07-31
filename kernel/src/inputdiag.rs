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
    /// Does this machine even have an i8042 to talk to? On aarch64 the answer
    /// is always no - QEMU's ARM `virt` has no such device and neither does
    /// VirtualBox's - so input there has to arrive through OHCI.
    pub ps2_controller: bool,
    pub ps2_keyboard: bool,
    pub ps2_mouse: bool,
    pub usb_keyboard: bool,
    pub usb_tablet: bool,
    pub usb: UsbSurvey,
}

impl Inputs {
    pub fn can_point(&self) -> bool {
        self.usb_tablet || self.ps2_mouse
    }

    pub fn can_type(&self) -> bool {
        self.ps2_keyboard || self.usb_keyboard
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
            (false, false) => Some(if !self.ps2_controller {
                // No i8042 at all, which is every arm64 machine. Not a fault
                // to diagnose: input here has to come from USB or virtio.
                //
                // Keyed off the controller rather than cfg!(target_arch),
                // which describes the machine running the *tests* - on an
                // arm64 Mac that made every host test take the arm64 branch.
                "No input detected. This machine has no PS/2 controller; enable VirtualBox USB 1.1 (OHCI)."
            } else if self.usb.has_unsupported() {
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
        Inputs {
            ps2_controller: true,
            ps2_keyboard: ps2_kbd,
            ps2_mouse,
            usb_keyboard: false,
            usb_tablet: tablet,
            usb,
        }
    }

    /// A machine with no i8042 anywhere: every arm64 target.
    fn no_ps2_bus(usb: UsbSurvey) -> Inputs {
        Inputs {
            ps2_controller: false,
            ps2_keyboard: false,
            ps2_mouse: false,
            usb_keyboard: false,
            usb_tablet: false,
            usb,
        }
    }

    #[test]
    fn a_working_machine_says_nothing() {
        // A banner on every boot would be noise, and noise gets ignored
        // exactly when it finally matters.
        let good = inputs(
            true,
            false,
            true,
            UsbSurvey {
                uhci: 1,
                ..Default::default()
            },
        );
        assert_eq!(good.note(), None);
    }

    #[test]
    fn a_modern_laptop_is_told_why_nothing_responds() {
        let laptop = inputs(
            false,
            false,
            false,
            UsbSurvey {
                xhci: 2,
                ..Default::default()
            },
        );
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
    fn a_machine_without_an_i8042_is_told_that_is_the_reason() {
        let note = no_ps2_bus(UsbSurvey::default()).note().unwrap();
        assert!(note.contains("no PS/2 controller"), "{note}");
        assert!(note.contains("USB 1.1"), "name what is missing: {note}");
    }

    #[test]
    fn a_machine_that_has_an_i8042_is_not_told_it_lacks_one() {
        // The x86 case: the controller is there, the devices are not.
        let note = inputs(false, false, false, UsbSurvey::default())
            .note()
            .unwrap();
        assert!(!note.contains("no PS/2 controller"), "{note}");
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

    #[test]
    fn an_ohci_keyboard_counts_even_without_ps2() {
        let arm = Inputs {
            ps2_controller: false,
            ps2_keyboard: false,
            ps2_mouse: false,
            usb_keyboard: true,
            usb_tablet: false,
            usb: UsbSurvey {
                ohci: 1,
                ..Default::default()
            },
        };
        assert!(arm.can_type());
        assert!(arm.usable());
        assert!(arm.note().unwrap().contains("Keyboard only"));
    }

    #[test]
    fn pointer_only_machine_notes_missing_keyboard() {
        let ptr_only = Inputs {
            ps2_controller: true,
            ps2_keyboard: false,
            ps2_mouse: true,
            usb_keyboard: false,
            usb_tablet: false,
            usb: UsbSurvey::default(),
        };
        assert!(ptr_only.can_point());
        assert!(!ptr_only.can_type());
        assert!(ptr_only.usable());
        let note = ptr_only.note().expect("pointer-only needs note");
        assert!(note.contains("Pointer only"));
    }

    #[test]
    fn dual_usb_input_is_fully_usable() {
        let dual = Inputs {
            ps2_controller: false,
            ps2_keyboard: false,
            ps2_mouse: false,
            usb_keyboard: true,
            usb_tablet: true,
            usb: UsbSurvey {
                ohci: 1,
                ..Default::default()
            },
        };
        assert!(dual.can_point());
        assert!(dual.can_type());
        assert!(dual.usable());
        assert_eq!(dual.note(), None);
    }

    #[test]
    fn inputs_both_present_has_no_note() {
        let full = Inputs {
            ps2_controller: true,
            ps2_keyboard: true,
            ps2_mouse: true,
            usb_keyboard: false,
            usb_tablet: false,
            usb: UsbSurvey::default(),
        };
        assert!(full.can_point());
        assert!(full.can_type());
        assert!(full.usable());
        assert_eq!(full.note(), None);
    }

    #[test]
    fn inputs_no_devices_usable_check() {
        let empty = Inputs {
            ps2_controller: false,
            ps2_keyboard: false,
            ps2_mouse: false,
            usb_keyboard: false,
            usb_tablet: false,
            usb: UsbSurvey::default(),
        };
        assert!(!empty.can_point());
        assert!(!empty.can_type());
        assert!(!empty.usable());
        let note = empty.note().expect("no input devices needs note");
        assert!(!note.is_empty());
    }
}


