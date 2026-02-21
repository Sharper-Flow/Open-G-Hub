//! Device communication layer with error handling and retry logic.
//!
//! Provides robust device communication by classifying errors and
//! implementing retry strategies for transient failures.

use crate::error::{Error, Result};
use crate::hidpp::HidppRequest;
use crate::transport::{hidpp_request, HidTransport};
use tracing::{debug, warn};

/// Maximum retry attempts for transient errors.
pub const MAX_RETRIES: u32 = 3;

/// Classification of communication errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// Transient errors that may succeed on retry (timeout, busy).
    Transient,
    /// Device is disconnected — stop retrying, notify user.
    Disconnected,
    /// Permission denied — likely Windows HID exclusive access.
    PermissionDenied,
    /// Protocol error — device returned an error code.
    Protocol,
    /// Invalid response — corrupted or unexpected data.
    InvalidResponse,
}

impl ErrorClass {
    /// Classify an error for retry decisions.
    pub fn classify(err: &Error) -> Self {
        match err {
            Error::Timeout(_) => Self::Transient,
            Error::PermissionDenied(_) => Self::PermissionDenied,
            Error::DeviceNotFound(_) => Self::Disconnected,
            Error::HidppProtocol { .. } => Self::Protocol,
            Error::Hid(msg) => {
                let lower = msg.to_lowercase();
                if lower.contains("disconnect")
                    || lower.contains("not found")
                    || lower.contains("no such device")
                {
                    Self::Disconnected
                } else if lower.contains("permission")
                    || lower.contains("access denied")
                    || lower.contains("access is denied")
                {
                    Self::PermissionDenied
                } else if lower.contains("timeout") || lower.contains("timed out") {
                    Self::Transient
                } else {
                    Self::InvalidResponse
                }
            }
            Error::OutOfRange { .. } | Error::Profile(_) => Self::InvalidResponse,
        }
    }

    /// Whether this error class is worth retrying.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Transient)
    }
}

/// Send a HID++ request with automatic retry for transient errors.
///
/// Returns the response on success, or the last error after exhausting retries.
pub fn send_with_retry(
    transport: &dyn HidTransport,
    req: &HidppRequest,
    max_retries: u32,
) -> Result<crate::hidpp::HidppResponse> {
    let mut last_error = None;

    for attempt in 0..=max_retries {
        match hidpp_request(transport, req) {
            Ok(resp) => {
                if attempt > 0 {
                    debug!("HID++ request succeeded on attempt {}", attempt + 1);
                }
                return Ok(resp);
            }
            Err(e) => {
                let class = ErrorClass::classify(&e);

                if !class.is_retryable() || attempt == max_retries {
                    warn!(
                        "HID++ request failed (class={:?}, attempt={}/{}): {}",
                        class,
                        attempt + 1,
                        max_retries + 1,
                        e
                    );
                    return Err(e);
                }

                debug!(
                    "HID++ transient error (attempt {}/{}): {}, retrying...",
                    attempt + 1,
                    max_retries + 1,
                    e
                );
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| Error::Hid("retry loop completed without result".into())))
}

/// Device connection status for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device is connected and responding.
    Connected,
    /// Device is not found / disconnected.
    Disconnected,
    /// Permission denied — needs driver setup.
    PermissionError,
    /// Communication error (transient or protocol).
    Error,
}

/// Check device connectivity by sending a ping via ROOT feature.
pub fn check_device_status(transport: &dyn HidTransport, device_index: u8) -> DeviceStatus {
    let ping = HidppRequest::new(device_index, 0x00, 0x00, vec![0x00, 0x00]);
    match hidpp_request(transport, &ping) {
        Ok(_) => DeviceStatus::Connected,
        Err(ref e) => match ErrorClass::classify(e) {
            ErrorClass::Disconnected => DeviceStatus::Disconnected,
            ErrorClass::PermissionDenied => DeviceStatus::PermissionError,
            _ => DeviceStatus::Error,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;

    #[test]
    fn classify_timeout_as_transient() {
        let err = Error::Timeout("1s elapsed".into());
        assert_eq!(ErrorClass::classify(&err), ErrorClass::Transient);
        assert!(ErrorClass::classify(&err).is_retryable());
    }

    #[test]
    fn classify_permission_denied() {
        let err = Error::PermissionDenied("access denied".into());
        assert_eq!(ErrorClass::classify(&err), ErrorClass::PermissionDenied);
        assert!(!ErrorClass::classify(&err).is_retryable());
    }

    #[test]
    fn classify_disconnect() {
        let err = Error::DeviceNotFound("G502".into());
        assert_eq!(ErrorClass::classify(&err), ErrorClass::Disconnected);
        assert!(!ErrorClass::classify(&err).is_retryable());
    }

    #[test]
    fn classify_hid_disconnect_message() {
        let err = Error::Hid("device disconnect detected".into());
        assert_eq!(ErrorClass::classify(&err), ErrorClass::Disconnected);
    }

    #[test]
    fn classify_hid_permission_message() {
        let err = Error::Hid("Access is denied".into());
        assert_eq!(ErrorClass::classify(&err), ErrorClass::PermissionDenied);
    }

    #[test]
    fn classify_hid_timeout_message() {
        let err = Error::Hid("timed out waiting for response".into());
        assert_eq!(ErrorClass::classify(&err), ErrorClass::Transient);
    }

    #[test]
    fn classify_protocol_error() {
        let err = Error::HidppProtocol {
            feature: 0x2201,
            code: 0x05,
        };
        assert_eq!(ErrorClass::classify(&err), ErrorClass::Protocol);
        assert!(!ErrorClass::classify(&err).is_retryable());
    }

    #[test]
    fn send_with_retry_succeeds_immediately() {
        let mock = MockTransport::new();

        // Register ROOT ping response
        mock.on_short_request(0x01, 0x00, 0x01, &[0x00, 0x00], &[0x00, 0x00, 0x00]);

        let req = HidppRequest::new(0x01, 0x00, 0x00, vec![0x00, 0x00]);
        let result = send_with_retry(&mock, &req, 3);
        assert!(result.is_ok());
    }

    #[test]
    fn send_with_retry_fails_non_retryable() {
        let mock = MockTransport::new();
        // No response registered → mock returns error (treated as non-retryable)

        let req = HidppRequest::new(0x01, 0x00, 0x00, vec![0x00, 0x00]);
        let result = send_with_retry(&mock, &req, 3);
        assert!(result.is_err());
    }

    #[test]
    fn check_device_status_connected() {
        let mock = MockTransport::new();
        mock.on_short_request(0x01, 0x00, 0x01, &[0x00, 0x00], &[0x00, 0x00, 0x00]);
        assert_eq!(check_device_status(&mock, 0x01), DeviceStatus::Connected);
    }

    #[test]
    fn check_device_status_disconnected() {
        let mock = MockTransport::new();
        // No response → error → classified as InvalidResponse → Error status
        assert_eq!(check_device_status(&mock, 0x01), DeviceStatus::Error);
    }
}
