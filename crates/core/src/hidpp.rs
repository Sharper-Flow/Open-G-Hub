//! HID++ 2.0 protocol encoding and decoding.
//!
//! HID++ uses two report formats:
//! - Short reports: 7 bytes (report ID 0x10)
//! - Long reports: 20 bytes (report ID 0x11)
//!
//! Protocol reference: libratbag (MIT) and Solaar (GPLv2, protocol knowledge only).

use crate::error::{Error, Result};

/// HID++ report ID for short messages (7 bytes total).
pub const SHORT_REPORT_ID: u8 = 0x10;
/// HID++ report ID for long messages (20 bytes total).
pub const LONG_REPORT_ID: u8 = 0x11;

/// Short report length (including report ID).
pub const SHORT_REPORT_LEN: usize = 7;
/// Long report length (including report ID).
pub const LONG_REPORT_LEN: usize = 20;

/// HID++ 2.0 well-known feature IDs.
pub mod features {
    /// Root feature — device ping and feature index lookup.
    pub const ROOT: u16 = 0x0000;
    /// Feature set — enumerate all supported features.
    pub const FEATURE_SET: u16 = 0x0001;
    /// Device name and type.
    pub const DEVICE_NAME: u16 = 0x0005;
    /// Adjustable DPI setting.
    pub const ADJUSTABLE_DPI: u16 = 0x2201;
    /// USB report rate (polling rate).
    pub const REPORT_RATE: u16 = 0x8060;
    /// Programmable button remapping.
    pub const REPROG_CONTROLS_V4: u16 = 0x1B04;
    /// Onboard profiles.
    pub const ONBOARD_PROFILES: u16 = 0x8100;
    /// Battery status.
    pub const BATTERY_STATUS: u16 = 0x1000;
}

/// A HID++ 2.0 request message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidppRequest {
    /// Device index on the receiver (0xFF for receiver itself, 0x01..0x06 for paired devices).
    pub device_index: u8,
    /// Feature index (looked up from feature ID via ROOT feature).
    pub feature_index: u8,
    /// Function ID within the feature (bits 7:4) and software ID (bits 3:0).
    pub function_sw: u8,
    /// Parameter bytes (up to 3 for short, up to 16 for long).
    pub params: Vec<u8>,
}

impl HidppRequest {
    /// Create a new request for a given feature index and function.
    pub fn new(device_index: u8, feature_index: u8, function: u8, params: Vec<u8>) -> Self {
        Self {
            device_index,
            feature_index,
            // Function in upper nibble, software ID = 0x01 in lower nibble
            function_sw: (function << 4) | 0x01,
            params,
        }
    }

    /// Encode into a HID report byte array.
    ///
    /// Returns a short (7-byte) report if params fit, otherwise a long (20-byte) report.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let param_len = self.params.len();

        if param_len <= 3 {
            // Short report
            let mut buf = vec![0u8; SHORT_REPORT_LEN];
            buf[0] = SHORT_REPORT_ID;
            buf[1] = self.device_index;
            buf[2] = self.feature_index;
            buf[3] = self.function_sw;
            for (i, &b) in self.params.iter().enumerate() {
                buf[4 + i] = b;
            }
            Ok(buf)
        } else if param_len <= 16 {
            // Long report
            let mut buf = vec![0u8; LONG_REPORT_LEN];
            buf[0] = LONG_REPORT_ID;
            buf[1] = self.device_index;
            buf[2] = self.feature_index;
            buf[3] = self.function_sw;
            for (i, &b) in self.params.iter().enumerate() {
                buf[4 + i] = b;
            }
            Ok(buf)
        } else {
            Err(Error::HidppProtocol {
                feature: 0,
                code: 0xFF,
            })
        }
    }
}

/// A decoded HID++ 2.0 response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidppResponse {
    /// Whether this is a long report.
    pub is_long: bool,
    /// Device index.
    pub device_index: u8,
    /// Feature index.
    pub feature_index: u8,
    /// Function and software ID byte.
    pub function_sw: u8,
    /// Response payload bytes.
    pub params: Vec<u8>,
}

impl HidppResponse {
    /// Decode a raw HID report into a structured response.
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < SHORT_REPORT_LEN {
            return Err(Error::Hid(format!(
                "response too short: {} bytes (minimum {})",
                data.len(),
                SHORT_REPORT_LEN
            )));
        }

        let report_id = data[0];
        let (is_long, expected_len) = match report_id {
            SHORT_REPORT_ID => (false, SHORT_REPORT_LEN),
            LONG_REPORT_ID => (true, LONG_REPORT_LEN),
            other => {
                return Err(Error::Hid(format!("unknown report ID: 0x{other:02X}")));
            }
        };

        if data.len() < expected_len {
            return Err(Error::Hid(format!(
                "incomplete report: got {} bytes, expected {}",
                data.len(),
                expected_len
            )));
        }

        let params = data[4..expected_len].to_vec();

        Ok(Self {
            is_long,
            device_index: data[1],
            feature_index: data[2],
            function_sw: data[3],
            params,
        })
    }

    /// Extract the function ID from the function_sw byte.
    pub fn function(&self) -> u8 {
        self.function_sw >> 4
    }

    /// Check if this response is an error report.
    /// HID++ 2.0 errors have feature_index == 0xFF.
    pub fn is_error(&self) -> bool {
        self.feature_index == 0xFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_short_report() {
        let req = HidppRequest::new(0x01, 0x05, 0x00, vec![0xAA, 0xBB]);
        let encoded = req.encode().unwrap();
        assert_eq!(encoded.len(), SHORT_REPORT_LEN);
        assert_eq!(encoded[0], SHORT_REPORT_ID);
        assert_eq!(encoded[1], 0x01); // device index
        assert_eq!(encoded[2], 0x05); // feature index
        assert_eq!(encoded[3], 0x01); // function=0 << 4 | sw_id=1
        assert_eq!(encoded[4], 0xAA);
        assert_eq!(encoded[5], 0xBB);
        assert_eq!(encoded[6], 0x00); // padding
    }

    #[test]
    fn encode_long_report() {
        let params = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let req = HidppRequest::new(0x01, 0x03, 0x02, params.clone());
        let encoded = req.encode().unwrap();
        assert_eq!(encoded.len(), LONG_REPORT_LEN);
        assert_eq!(encoded[0], LONG_REPORT_ID);
        assert_eq!(encoded[3], (0x02 << 4) | 0x01); // function=2, sw_id=1
        assert_eq!(&encoded[4..12], &params[..]);
    }

    #[test]
    fn encode_rejects_oversized_params() {
        let params = vec![0u8; 17]; // exceeds 16-byte long report limit
        let req = HidppRequest::new(0x01, 0x00, 0x00, params);
        assert!(req.encode().is_err());
    }

    #[test]
    fn decode_short_response() {
        let data = [SHORT_REPORT_ID, 0x01, 0x05, 0x01, 0xAA, 0xBB, 0x00];
        let resp = HidppResponse::decode(&data).unwrap();
        assert!(!resp.is_long);
        assert_eq!(resp.device_index, 0x01);
        assert_eq!(resp.feature_index, 0x05);
        assert_eq!(resp.function(), 0x00);
        assert_eq!(resp.params, vec![0xAA, 0xBB, 0x00]);
    }

    #[test]
    fn decode_long_response() {
        let mut data = vec![LONG_REPORT_ID, 0x01, 0x03, 0x21];
        data.extend_from_slice(&[0x0C, 0x80, 0x00, 0x00]); // DPI bytes
        data.resize(LONG_REPORT_LEN, 0);
        let resp = HidppResponse::decode(&data).unwrap();
        assert!(resp.is_long);
        assert_eq!(resp.function(), 0x02);
        assert_eq!(resp.params[0], 0x0C);
        assert_eq!(resp.params[1], 0x80);
    }

    #[test]
    fn decode_rejects_short_data() {
        let data = [0x10, 0x01, 0x02]; // too short
        assert!(HidppResponse::decode(&data).is_err());
    }

    #[test]
    fn decode_rejects_unknown_report_id() {
        let data = [0x99, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        assert!(HidppResponse::decode(&data).is_err());
    }

    #[test]
    fn error_response_detected() {
        let data = [SHORT_REPORT_ID, 0x01, 0xFF, 0x01, 0x05, 0x02, 0x00];
        let resp = HidppResponse::decode(&data).unwrap();
        assert!(resp.is_error());
    }

    #[test]
    fn roundtrip_request_response() {
        let req = HidppRequest::new(0x01, 0x05, 0x00, vec![0xAA]);
        let encoded = req.encode().unwrap();
        let resp = HidppResponse::decode(&encoded).unwrap();
        assert_eq!(resp.device_index, req.device_index);
        assert_eq!(resp.feature_index, req.feature_index);
        assert_eq!(resp.function_sw, req.function_sw);
    }
}
