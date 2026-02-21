//! open-g-hub-core: HID++ protocol, device discovery, and mouse configuration.
//!
//! This crate provides the cross-platform core logic for communicating with
//! Logitech G mice via the HID++ 2.0 protocol over USB HID.

pub mod buttons;
pub mod comm;
pub mod device;
pub mod dpi;
pub mod error;
pub mod hidpp;
#[cfg(test)]
mod integration_tests;
pub mod onboard;
pub mod profile;
pub mod report_rate;
pub mod safety;
pub mod transport;

/// Logitech USB Vendor ID.
pub const LOGITECH_VID: u16 = 0x046D;

/// Known Logitech G502 product IDs.
pub mod pids {
    /// G502 Lightspeed (wireless receiver mode).
    pub const G502_LIGHTSPEED: u16 = 0xC08D;
    /// G502 HERO (wired).
    pub const G502_HERO: u16 = 0xC08B;
}
