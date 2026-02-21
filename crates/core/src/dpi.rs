//! DPI read/write via HID++ 2.0 ADJUSTABLE_DPI feature (0x2201).
//!
//! HID++ 2.0 ADJUSTABLE_DPI functions:
//!   - Function 0: getSensorCount → params[0] = sensor count
//!   - Function 1: getSensorDpi(sensor_idx) → params[0..1] = current DPI, params[2..3] = default DPI
//!   - Function 2: setSensorDpi(sensor_idx, dpi) → sets DPI
//!
//! Protocol reference: libratbag (MIT), HID++ 2.0 specification.

use crate::error::Result;
use crate::hidpp::{self, HidppRequest};
use crate::safety;
use crate::transport::{hidpp_request, lookup_feature_index, HidTransport};

/// Read the current DPI from the device's first sensor.
///
/// Steps:
/// 1. Look up ADJUSTABLE_DPI feature index via ROOT
/// 2. Call getSensorDpi (function 1) for sensor 0
/// 3. Decode response: params[0..1] = current DPI (big-endian)
pub fn read_dpi(transport: &dyn HidTransport, device_index: u8) -> Result<u16> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::ADJUSTABLE_DPI)?;

    // getSensorDpi: function 1, params[0] = sensor index 0
    let req = HidppRequest::new(device_index, feature_idx, 0x01, vec![0x00]);
    let resp = hidpp_request(transport, &req)?;

    // Response params[0..1] = current DPI (big-endian)
    let dpi = ((resp.params[0] as u16) << 8) | (resp.params[1] as u16);
    Ok(dpi)
}

/// Write a DPI value to the device's first sensor.
///
/// The value is validated against safe bounds before sending.
///
/// Steps:
/// 1. Validate DPI via safety module
/// 2. Look up ADJUSTABLE_DPI feature index via ROOT
/// 3. Call setSensorDpi (function 2) for sensor 0 with DPI value
pub fn write_dpi(transport: &dyn HidTransport, device_index: u8, dpi: u16) -> Result<u16> {
    let validated = safety::validate_dpi(dpi)?;
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::ADJUSTABLE_DPI)?;

    // setSensorDpi: function 2, params = [sensor_idx, dpi_hi, dpi_lo]
    let req = HidppRequest::new(
        device_index,
        feature_idx,
        0x02,
        vec![0x00, (validated >> 8) as u8, (validated & 0xFF) as u8],
    );
    let _resp = hidpp_request(transport, &req)?;

    Ok(validated)
}

/// Read the sensor count from the device.
pub fn read_sensor_count(transport: &dyn HidTransport, device_index: u8) -> Result<u8> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::ADJUSTABLE_DPI)?;

    // getSensorCount: function 0, no params
    let req = HidppRequest::new(device_index, feature_idx, 0x00, vec![]);
    let resp = hidpp_request(transport, &req)?;

    Ok(resp.params[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;

    const DEV_IDX: u8 = 0x01;
    const DPI_FEATURE_IDX: u8 = 0x07;

    /// Set up mock to respond to ROOT feature lookup for ADJUSTABLE_DPI.
    fn setup_dpi_feature_lookup(mock: &MockTransport) {
        // ROOT request: device=0x01, feature_idx=0x00, function_sw=(0<<4)|1=0x01
        // params = [0x22, 0x01] (ADJUSTABLE_DPI feature ID)
        // Response: params[0]=DPI_FEATURE_IDX
        mock.on_short_request(
            DEV_IDX,
            0x00,
            0x01,
            &[0x22, 0x01],
            &[DPI_FEATURE_IDX, 0x00, 0x00],
        );
    }

    #[test]
    fn read_dpi_returns_current_value() {
        let mock = MockTransport::new();
        setup_dpi_feature_lookup(&mock);

        // getSensorDpi response: DPI=800 (0x0320)
        mock.on_short_request(
            DEV_IDX,
            DPI_FEATURE_IDX,
            0x11,                // function=1 << 4 | sw_id=1
            &[0x00],             // sensor 0
            &[0x03, 0x20, 0x03], // current=800, default MSB
        );

        let dpi = read_dpi(&mock, DEV_IDX).unwrap();
        assert_eq!(dpi, 800);
    }

    #[test]
    fn read_dpi_high_value() {
        let mock = MockTransport::new();
        setup_dpi_feature_lookup(&mock);

        // getSensorDpi response: DPI=16000 (0x3E80)
        mock.on_short_request(DEV_IDX, DPI_FEATURE_IDX, 0x11, &[0x00], &[0x3E, 0x80, 0x03]);

        let dpi = read_dpi(&mock, DEV_IDX).unwrap();
        assert_eq!(dpi, 16000);
    }

    #[test]
    fn write_dpi_sends_validated_value() {
        let mock = MockTransport::new();
        setup_dpi_feature_lookup(&mock);

        // setSensorDpi: function 2, params = [sensor=0, dpi_hi, dpi_lo]
        // For DPI=1600 (0x0640): params = [0x00, 0x06, 0x40]
        mock.on_short_request(
            DEV_IDX,
            DPI_FEATURE_IDX,
            0x21, // function=2 << 4 | sw_id=1
            &[0x00, 0x06, 0x40],
            &[0x00, 0x06, 0x40], // echo back
        );

        let result = write_dpi(&mock, DEV_IDX, 1600).unwrap();
        assert_eq!(result, 1600);
    }

    #[test]
    fn write_dpi_rounds_value() {
        let mock = MockTransport::new();
        setup_dpi_feature_lookup(&mock);

        // 810 rounds to 800 (0x0320)
        mock.on_short_request(
            DEV_IDX,
            DPI_FEATURE_IDX,
            0x21,
            &[0x00, 0x03, 0x20],
            &[0x00, 0x03, 0x20],
        );

        let result = write_dpi(&mock, DEV_IDX, 810).unwrap();
        assert_eq!(result, 800);
    }

    #[test]
    fn write_dpi_rejects_out_of_range() {
        let mock = MockTransport::new();
        // No mock setup needed — validation rejects before HID communication
        let result = write_dpi(&mock, DEV_IDX, 50);
        assert!(result.is_err());
    }

    #[test]
    fn read_sensor_count() {
        let mock = MockTransport::new();
        setup_dpi_feature_lookup(&mock);

        // getSensorCount response: 1 sensor
        mock.on_short_request(
            DEV_IDX,
            DPI_FEATURE_IDX,
            0x01, // function=0 << 4 | sw_id=1
            &[],
            &[0x01, 0x00, 0x00],
        );

        let count = super::read_sensor_count(&mock, DEV_IDX).unwrap();
        assert_eq!(count, 1);
    }
}
