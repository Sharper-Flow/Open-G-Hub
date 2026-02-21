//! Error types for open-g-hub-core.

use thiserror::Error;

/// Core library error type.
#[derive(Debug, Error)]
pub enum Error {
    /// HID device communication failure.
    #[error("HID error: {0}")]
    Hid(String),

    /// Device not found during enumeration.
    #[error("device not found: {0}")]
    DeviceNotFound(String),

    /// HID++ protocol error (device returned error code).
    #[error("HID++ error: feature 0x{feature:04X}, code {code}")]
    HidppProtocol { feature: u16, code: u8 },

    /// Value out of safe range.
    #[error("value out of range: {field} = {value} (allowed {min}..={max})")]
    OutOfRange {
        field: &'static str,
        value: u32,
        min: u32,
        max: u32,
    },

    /// Profile serialization/deserialization error.
    #[error("profile error: {0}")]
    Profile(String),

    /// Permission denied (likely Windows HID exclusive access).
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Operation timed out.
    #[error("timeout: {0}")]
    Timeout(String),
}

/// Convenience Result alias.
pub type Result<T> = std::result::Result<T, Error>;
