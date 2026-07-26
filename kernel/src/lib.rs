//! Host-testable kernel helpers (serial message constants, formatting).
//! Freestanding when not under `cfg(test)`.

#![cfg_attr(not(test), no_std)]

pub mod anim;
pub mod beep;
pub mod caps;
pub mod fb;
pub(crate) mod font;
pub mod keyboard;
pub mod mcp;
pub mod mouse;
pub(crate) mod pci;
pub(crate) mod port;
pub mod screens;
pub(crate) mod search;
pub mod searchui;
pub mod serial;
pub mod setup;
pub mod skills;
pub mod ui;
pub mod usb_tablet;

/// Canonical early-boot banner printed to COM1.
pub const HELLO_MESSAGE: &str = "os: hello from kernel";
