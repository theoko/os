//! Host-testable kernel helpers (serial message constants, formatting).
//! Freestanding when not under `cfg(test)`.

#![cfg_attr(not(test), no_std)]

pub mod anim;
pub mod beep;
pub mod caps;
pub mod fb;
pub mod font;
pub mod keyboard;
pub mod mcp;
pub mod mouse;
pub mod pci;
pub mod port;
pub mod screens;
pub mod search;
pub mod searchui;
pub mod serial;
pub mod setup;
pub mod skills;
pub mod ui;
pub mod usb_tablet;

/// Canonical early-boot banner printed to COM1.
pub const HELLO_MESSAGE: &str = "os: hello from kernel";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_message_is_stable() {
        assert_eq!(HELLO_MESSAGE, "os: hello from kernel");
        assert!(HELLO_MESSAGE.starts_with("os:"));
    }
}
