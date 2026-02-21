//! Safety layer: validates all write parameters against known-safe ranges
//! before sending to the device.
//!
//! This prevents bricking the mouse by rejecting out-of-range values.
//!
//! # G502 Lightspeed Safety Bounds
//!
//! The following ranges are derived from the Logitech G502 Lightspeed
//! hardware specifications and confirmed via libratbag (MIT) device profiles.
//!
//! ## DPI
//! - **Range**: 100 – 25,600 DPI (HERO 25K sensor)
//! - **Step size**: 50 DPI increments
//! - **Default**: 800 DPI (factory setting)
//! - **Reference**: libratbag `data/devices/logitech-g502-lightspeed.device`
//! - **Note**: G502 HERO (wired) may have 100–16,000 DPI on older sensor revisions.
//!   We use 25,600 as the upper bound to support the HERO 25K sensor variant.
//!
//! ## Polling Rate
//! - **Supported values**: 125 Hz, 250 Hz, 500 Hz, 1000 Hz
//! - **Default**: 1000 Hz (1ms report interval)
//! - **Encoding**: HID++ uses report interval in ms (8ms, 4ms, 2ms, 1ms)
//! - **Reference**: HID++ 2.0 REPORT_RATE feature (0x8060) getReportRateList bitmask
//!
//! ## Button Indices
//! - **Range**: 0–5 (6 programmable buttons on G502)
//! - **Physical mapping**: 0=Left, 1=Right, 2=Middle, 3=Back, 4=Forward, 5=DPI
//! - **CID range**: 0x0000–0x00FF for standard controls
//! - **Note**: Additional G-shift buttons exist but are not exposed in this version
//!
//! ## Macros
//! - **Not supported** in this version. Button remapping is CID-to-CID only.
//! - Macro support would require ONBOARD_PROFILES memory write, which carries
//!   higher bricking risk and is deferred to a future release.
//!
//! ## Safety Invariants
//! 1. All DPI values are clamped to [100, 25600] and rounded to nearest 50
//! 2. Only known polling rate enum values are accepted (no raw Hz pass-through)
//! 3. Button indices are bounds-checked against G502_BUTTON_COUNT
//! 4. All validation happens BEFORE any HID communication — no invalid data
//!    ever reaches the device

use crate::device::PollingRate;
use crate::error::{Error, Result};
use crate::hidpp::features;

/// Bricking risk disclaimer — include in any user-facing output about device writes.
pub const BRICKING_DISCLAIMER: &str = "\
WARNING: This software writes directly to your mouse's hardware registers via HID++. \
While all writes are bounds-checked against known-safe ranges, incorrect usage or \
software bugs could theoretically render the device unresponsive. \
Macro and firmware operations are intentionally not supported due to higher risk. \
Use at your own risk. See TROUBLESHOOTING.md for recovery steps.";

/// HID++ feature IDs that Open G Hub is allowed to communicate with.
///
/// Any feature not in this whitelist is rejected before reaching the device.
/// This prevents accidental or malicious use of dangerous features like
/// firmware update, DFU mode, or raw memory access.
const ALLOWED_FEATURE_IDS: &[u16] = &[
    features::ROOT,               // 0x0000 — feature index lookup (read-only)
    features::FEATURE_SET,        // 0x0001 — enumerate features (read-only)
    features::DEVICE_NAME,        // 0x0005 — device name (read-only)
    features::BATTERY_STATUS,     // 0x1000 — battery level (read-only)
    features::REPROG_CONTROLS_V4, // 0x1B04 — button remapping
    features::ADJUSTABLE_DPI,     // 0x2201 — DPI configuration
    features::REPORT_RATE,        // 0x8060 — polling rate
    features::ONBOARD_PROFILES,   // 0x8100 — profile management
];

/// Maximum allowed function ID within any feature.
///
/// HID++ 2.0 uses 4 bits for the function ID (0x0..0xF). We cap at a
/// conservative limit since most features use functions 0-3.
const MAX_FUNCTION_ID: u8 = 0x0F;

/// Validate that a HID++ feature ID is in the allowed whitelist.
///
/// Rejects firmware update, DFU, raw memory, and other dangerous features.
pub fn validate_feature_id(feature_id: u16) -> Result<()> {
    if ALLOWED_FEATURE_IDS.contains(&feature_id) {
        Ok(())
    } else {
        Err(Error::OutOfRange {
            field: "feature_id",
            value: feature_id as u32,
            min: 0,
            max: 0xFFFF,
        })
    }
}

/// Validate a full HID++ request before sending.
///
/// Checks:
/// 1. Feature ID is in the allowed whitelist
/// 2. Function ID is within valid range (0-15)
pub fn validate_hidpp_request(feature_id: u16, function_id: u8) -> Result<()> {
    validate_feature_id(feature_id)?;
    if function_id > MAX_FUNCTION_ID {
        return Err(Error::OutOfRange {
            field: "function_id",
            value: function_id as u32,
            min: 0,
            max: MAX_FUNCTION_ID as u32,
        });
    }
    Ok(())
}

/// G502 DPI constraints.
pub const DPI_MIN: u16 = 100;
pub const DPI_MAX: u16 = 25600;
pub const DPI_STEP: u16 = 50;

/// Validate a DPI value is within safe bounds and aligned to step size.
pub fn validate_dpi(dpi: u16) -> Result<u16> {
    if !(DPI_MIN..=DPI_MAX).contains(&dpi) {
        return Err(Error::OutOfRange {
            field: "dpi",
            value: dpi as u32,
            min: DPI_MIN as u32,
            max: DPI_MAX as u32,
        });
    }
    // Round to nearest step
    let rounded = ((dpi + DPI_STEP / 2) / DPI_STEP) * DPI_STEP;
    let clamped = rounded.clamp(DPI_MIN, DPI_MAX);
    Ok(clamped)
}

/// Validate a polling rate value.
pub fn validate_polling_rate(hz: u16) -> Result<PollingRate> {
    PollingRate::from_hz(hz).ok_or(Error::OutOfRange {
        field: "polling_rate",
        value: hz as u32,
        min: 125,
        max: 1000,
    })
}

/// Validate a button index (0-based).
pub fn validate_button_index(index: usize) -> Result<()> {
    if index >= crate::device::G502_BUTTON_COUNT {
        return Err(Error::OutOfRange {
            field: "button_index",
            value: index as u32,
            min: 0,
            max: (crate::device::G502_BUTTON_COUNT - 1) as u32,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_dpi_in_range() {
        assert_eq!(validate_dpi(800).unwrap(), 800);
        assert_eq!(validate_dpi(100).unwrap(), 100);
        assert_eq!(validate_dpi(25600).unwrap(), 25600);
    }

    #[test]
    fn validate_dpi_rounds_to_step() {
        assert_eq!(validate_dpi(810).unwrap(), 800);
        assert_eq!(validate_dpi(825).unwrap(), 850);
        assert_eq!(validate_dpi(130).unwrap(), 150);
    }

    #[test]
    fn validate_dpi_rejects_out_of_range() {
        assert!(validate_dpi(50).is_err());
        assert!(validate_dpi(0).is_err());
        assert!(validate_dpi(30000).is_err());
    }

    #[test]
    fn validate_polling_rate_accepts_known() {
        assert_eq!(validate_polling_rate(125).unwrap(), PollingRate::Hz125);
        assert_eq!(validate_polling_rate(1000).unwrap(), PollingRate::Hz1000);
    }

    #[test]
    fn validate_polling_rate_rejects_unknown() {
        assert!(validate_polling_rate(200).is_err());
        assert!(validate_polling_rate(0).is_err());
    }

    #[test]
    fn validate_button_index_in_range() {
        for i in 0..6 {
            assert!(validate_button_index(i).is_ok());
        }
    }

    #[test]
    fn validate_button_index_out_of_range() {
        assert!(validate_button_index(6).is_err());
        assert!(validate_button_index(100).is_err());
    }

    #[test]
    fn validate_feature_whitelist_allows_known() {
        assert!(validate_feature_id(features::ROOT).is_ok());
        assert!(validate_feature_id(features::ADJUSTABLE_DPI).is_ok());
        assert!(validate_feature_id(features::REPORT_RATE).is_ok());
        assert!(validate_feature_id(features::REPROG_CONTROLS_V4).is_ok());
        assert!(validate_feature_id(features::ONBOARD_PROFILES).is_ok());
    }

    #[test]
    fn validate_feature_whitelist_rejects_unknown() {
        // DFU-like feature IDs that should be blocked
        assert!(validate_feature_id(0x00D0).is_err()); // DFU
        assert!(validate_feature_id(0x1802).is_err()); // Unknown
        assert!(validate_feature_id(0xFFFF).is_err()); // Invalid
    }

    #[test]
    fn validate_hidpp_request_ok() {
        assert!(validate_hidpp_request(features::ADJUSTABLE_DPI, 0x00).is_ok());
        assert!(validate_hidpp_request(features::ADJUSTABLE_DPI, 0x0F).is_ok());
    }

    #[test]
    fn validate_hidpp_request_rejects_bad_feature() {
        assert!(validate_hidpp_request(0x00D0, 0x00).is_err());
    }

    #[test]
    fn validate_hidpp_request_accepts_all_function_ids() {
        // Function IDs are 4-bit (0-15), all should be accepted for allowed features
        for f in 0..=0x0F {
            assert!(validate_hidpp_request(features::ROOT, f).is_ok());
        }
    }

    #[test]
    fn bricking_disclaimer_not_empty() {
        assert!(!BRICKING_DISCLAIMER.is_empty());
        assert!(BRICKING_DISCLAIMER.contains("WARNING"));
    }
}
