//! Substrate-independent logic: the part of the system that is not a kernel.
//!
//! `kernel/src/{caps,search,skills}.rs` carry the same logic in `no_std` form
//! against fixed buffers and an FPU-less target. Those constraints are
//! properties of the old substrate, not of the ideas, so this crate holds the
//! std port that outlives the substrate move (docs/linux-os-doc-v02.md).
//!
//! The kernel copies stay where they are: `main` must remain bootable until
//! the replacement is demonstrably better, so the two live side by side.

pub mod caps;
pub mod level;
pub mod search;
pub mod skills;
