//! Polling rate read/write via HID++ 2.0 REPORT_RATE feature (0x8060).
//!
//! HID++ 2.0 REPORT_RATE functions:
//!   - Function 0: getReportRateList → params[0..2] = bitmask of supported rates
//!   - Function 1: getReportRate → params[0] = current rate encoding
//!   - Function 2: setReportRate(rate) → sets polling rate
//!
//! Rate encoding: 1=1ms(1000Hz), 2=2ms(500Hz), 4=4ms(250Hz), 8=8ms(125Hz)

use crate::device::PollingRate;
use crate::error::{Error, Result};
use crate::hidpp::{self, HidppRequest};
use crate::transport::{hidpp_request, lookup_feature_index, HidTransport};

/// Convert a PollingRate to the HID++ report interval encoding.
///
/// The device uses interval in ms: 1ms=1000Hz, 2ms=500Hz, 4ms=250Hz, 8ms=125Hz.
fn rate_to_interval(rate: PollingRate) -> u8 {
    match rate {
        PollingRate::Hz1000 => 1,
        PollingRate::Hz500 => 2,
        PollingRate::Hz250 => 4,
        PollingRate::Hz125 => 8,
    }
}

/// Convert a HID++ interval encoding back to PollingRate.
fn interval_to_rate(interval: u8) -> Result<PollingRate> {
    match interval {
        1 => Ok(PollingRate::Hz1000),
        2 => Ok(PollingRate::Hz500),
        4 => Ok(PollingRate::Hz250),
        8 => Ok(PollingRate::Hz125),
        other => Err(Error::HidppProtocol {
            feature: hidpp::features::REPORT_RATE,
            code: other,
        }),
    }
}

/// Read the current polling rate from the device.
pub fn read_report_rate(transport: &dyn HidTransport, device_index: u8) -> Result<PollingRate> {
    let feature_idx = lookup_feature_index(transport, device_index, hidpp::features::REPORT_RATE)?;

    // getReportRate: function 1, no params
    let req = HidppRequest::new(device_index, feature_idx, 0x01, vec![]);
    let resp = hidpp_request(transport, &req)?;

    interval_to_rate(resp.params[0])
}

/// Write a polling rate to the device.
pub fn write_report_rate(
    transport: &dyn HidTransport,
    device_index: u8,
    rate: PollingRate,
) -> Result<()> {
    let feature_idx = lookup_feature_index(transport, device_index, hidpp::features::REPORT_RATE)?;

    let interval = rate_to_interval(rate);

    // setReportRate: function 2, params[0] = interval
    let req = HidppRequest::new(device_index, feature_idx, 0x02, vec![interval]);
    let _resp = hidpp_request(transport, &req)?;

    Ok(())
}

/// Read the list of supported polling rates from the device.
pub fn read_supported_rates(
    transport: &dyn HidTransport,
    device_index: u8,
) -> Result<Vec<PollingRate>> {
    let feature_idx = lookup_feature_index(transport, device_index, hidpp::features::REPORT_RATE)?;

    // getReportRateList: function 0, no params
    let req = HidppRequest::new(device_index, feature_idx, 0x00, vec![]);
    let resp = hidpp_request(transport, &req)?;

    // Response is a bitmask — bit N set means interval N is supported
    let bitmask = resp.params[0];
    let mut rates = Vec::new();
    for interval in [1u8, 2, 4, 8] {
        if bitmask & interval != 0 {
            if let Ok(rate) = interval_to_rate(interval) {
                rates.push(rate);
            }
        }
    }

    Ok(rates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;

    const DEV_IDX: u8 = 0x01;
    const RATE_FEATURE_IDX: u8 = 0x08;

    fn setup_rate_feature_lookup(mock: &MockTransport) {
        // ROOT lookup: REPORT_RATE (0x8060) → feature index 0x08
        mock.on_short_request(
            DEV_IDX,
            0x00,
            0x01,
            &[0x80, 0x60],
            &[RATE_FEATURE_IDX, 0x00, 0x00],
        );
    }

    #[test]
    fn read_report_rate_1000hz() {
        let mock = MockTransport::new();
        setup_rate_feature_lookup(&mock);

        // getReportRate response: interval=1 (1000Hz)
        mock.on_short_request(
            DEV_IDX,
            RATE_FEATURE_IDX,
            0x11, // function=1 << 4 | sw_id=1
            &[],
            &[0x01, 0x00, 0x00],
        );

        let rate = read_report_rate(&mock, DEV_IDX).unwrap();
        assert_eq!(rate, PollingRate::Hz1000);
    }

    #[test]
    fn read_report_rate_125hz() {
        let mock = MockTransport::new();
        setup_rate_feature_lookup(&mock);

        mock.on_short_request(DEV_IDX, RATE_FEATURE_IDX, 0x11, &[], &[0x08, 0x00, 0x00]);

        let rate = read_report_rate(&mock, DEV_IDX).unwrap();
        assert_eq!(rate, PollingRate::Hz125);
    }

    #[test]
    fn write_report_rate_500hz() {
        let mock = MockTransport::new();
        setup_rate_feature_lookup(&mock);

        // setReportRate: function 2, params[0]=2 (500Hz)
        mock.on_short_request(
            DEV_IDX,
            RATE_FEATURE_IDX,
            0x21, // function=2 << 4 | sw_id=1
            &[0x02],
            &[0x02, 0x00, 0x00],
        );

        write_report_rate(&mock, DEV_IDX, PollingRate::Hz500).unwrap();
    }

    #[test]
    fn write_report_rate_250hz() {
        let mock = MockTransport::new();
        setup_rate_feature_lookup(&mock);

        mock.on_short_request(
            DEV_IDX,
            RATE_FEATURE_IDX,
            0x21,
            &[0x04],
            &[0x04, 0x00, 0x00],
        );

        write_report_rate(&mock, DEV_IDX, PollingRate::Hz250).unwrap();
    }

    #[test]
    fn read_supported_rates() {
        let mock = MockTransport::new();
        setup_rate_feature_lookup(&mock);

        // getReportRateList: bitmask 0x0F = supports 1ms, 2ms, 4ms, 8ms (all rates)
        mock.on_short_request(
            DEV_IDX,
            RATE_FEATURE_IDX,
            0x01, // function=0 << 4 | sw_id=1
            &[],
            &[0x0F, 0x00, 0x00],
        );

        let rates = super::read_supported_rates(&mock, DEV_IDX).unwrap();
        assert_eq!(rates.len(), 4);
        assert!(rates.contains(&PollingRate::Hz1000));
        assert!(rates.contains(&PollingRate::Hz125));
    }

    #[test]
    fn rate_interval_roundtrip() {
        for rate in PollingRate::ALL {
            let interval = rate_to_interval(*rate);
            let back = interval_to_rate(interval).unwrap();
            assert_eq!(back, *rate);
        }
    }

    #[test]
    fn invalid_interval_rejected() {
        assert!(interval_to_rate(3).is_err());
        assert!(interval_to_rate(0).is_err());
    }
}
