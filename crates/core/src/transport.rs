//! HID transport abstraction for device communication.
//!
//! Provides a trait-based transport layer so that real HID devices and
//! mock devices share the same interface.

use crate::error::{Error, Result};
use crate::hidpp::{HidppRequest, HidppResponse};
use tracing::{debug, trace, warn};

/// Abstraction over raw HID read/write.
///
/// Implementations must be able to send a HID++ request and receive a response.
pub trait HidTransport: Send {
    /// Write a raw HID report and return the response.
    fn send_report(&self, data: &[u8]) -> Result<Vec<u8>>;
}

/// Send a HID++ request and decode the response.
pub fn hidpp_request(transport: &dyn HidTransport, req: &HidppRequest) -> Result<HidppResponse> {
    let encoded = req.encode()?;
    trace!(
        device_index = req.device_index,
        feature_index = req.feature_index,
        function_sw = format_args!("0x{:02X}", req.function_sw),
        report_hex = format_args!("{:02X?}", encoded),
        "HID++ TX"
    );

    let raw = transport.send_report(&encoded)?;
    let resp = HidppResponse::decode(&raw)?;

    trace!(
        is_long = resp.is_long,
        feature_index = resp.feature_index,
        function = resp.function(),
        params_hex = format_args!("{:02X?}", resp.params),
        "HID++ RX"
    );

    // Check for HID++ error responses
    if resp.is_error() {
        let error_feature = if resp.params.len() >= 2 {
            resp.params[0]
        } else {
            0
        };
        let error_code = if resp.params.len() >= 3 {
            resp.params[2]
        } else {
            0
        };
        warn!(
            error_feature = error_feature,
            error_code = error_code,
            "HID++ error response"
        );
        return Err(Error::HidppProtocol {
            feature: error_feature as u16,
            code: error_code,
        });
    }

    Ok(resp)
}

/// Look up the feature index for a given HID++ 2.0 feature ID using the ROOT feature.
///
/// ROOT feature (index 0x00) function 0 = getFeatureID:
///   params[0..1] = feature ID (big-endian)
///   response params[0] = feature index, params[1] = feature type
pub fn lookup_feature_index(
    transport: &dyn HidTransport,
    device_index: u8,
    feature_id: u16,
) -> Result<u8> {
    let req = HidppRequest::new(
        device_index,
        0x00, // ROOT feature index is always 0
        0x00, // function 0 = getFeatureID
        vec![(feature_id >> 8) as u8, (feature_id & 0xFF) as u8],
    );
    let resp = hidpp_request(transport, &req)?;

    let feature_index = resp.params[0];
    if feature_index == 0 {
        debug!(
            feature_id = format_args!("0x{:04X}", feature_id),
            "Feature not supported by device"
        );
        return Err(Error::HidppProtocol {
            feature: feature_id,
            code: 0x05, // NOT_FOUND
        });
    }

    debug!(
        feature_id = format_args!("0x{:04X}", feature_id),
        feature_index = feature_index,
        "Feature lookup success"
    );
    Ok(feature_index)
}

/// A mock HID transport for testing.
///
/// Stores predefined request→response mappings.
#[cfg(test)]
pub mod mock {
    use super::*;
    use crate::hidpp::{LONG_REPORT_LEN, SHORT_REPORT_LEN};
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Mock transport that returns preconfigured responses.
    pub struct MockTransport {
        responses: Mutex<HashMap<Vec<u8>, Vec<u8>>>,
    }

    impl MockTransport {
        pub fn new() -> Self {
            Self {
                responses: Mutex::new(HashMap::new()),
            }
        }

        /// Register a response for a given request.
        pub fn on_request(&self, request: Vec<u8>, response: Vec<u8>) {
            self.responses.lock().unwrap().insert(request, response);
        }

        /// Register a short HID++ response for a request.
        pub fn on_short_request(
            &self,
            device_idx: u8,
            feature_idx: u8,
            function_sw: u8,
            req_params: &[u8],
            resp_params: &[u8],
        ) {
            let mut req = vec![0x10, device_idx, feature_idx, function_sw];
            let mut req_pad = req_params.to_vec();
            req_pad.resize(SHORT_REPORT_LEN - 4, 0);
            req.extend_from_slice(&req_pad);

            let mut resp = vec![0x10, device_idx, feature_idx, function_sw];
            let mut resp_pad = resp_params.to_vec();
            resp_pad.resize(SHORT_REPORT_LEN - 4, 0);
            resp.extend_from_slice(&resp_pad);

            self.on_request(req, resp);
        }

        /// Register a long HID++ response for a request.
        pub fn on_long_request(
            &self,
            device_idx: u8,
            feature_idx: u8,
            function_sw: u8,
            req_params: &[u8],
            resp_params: &[u8],
        ) {
            // Request may be short or long depending on params length
            let req_report_id = if req_params.len() <= 3 {
                0x10u8
            } else {
                0x11u8
            };
            let req_len = if req_report_id == 0x10 {
                SHORT_REPORT_LEN
            } else {
                LONG_REPORT_LEN
            };
            let mut req = vec![req_report_id, device_idx, feature_idx, function_sw];
            let mut req_pad = req_params.to_vec();
            req_pad.resize(req_len - 4, 0);
            req.extend_from_slice(&req_pad);

            let mut resp = vec![0x11, device_idx, feature_idx, function_sw];
            let mut resp_pad = resp_params.to_vec();
            resp_pad.resize(LONG_REPORT_LEN - 4, 0);
            resp.extend_from_slice(&resp_pad);

            self.on_request(req, resp);
        }
    }

    impl HidTransport for MockTransport {
        fn send_report(&self, data: &[u8]) -> Result<Vec<u8>> {
            let responses = self.responses.lock().unwrap();
            responses.get(data).cloned().ok_or_else(|| {
                Error::Hid(format!(
                    "mock: no response registered for request {:02X?}",
                    data
                ))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_feature_index_success() {
        let mock = mock::MockTransport::new();
        // ROOT request: feature_index=0, function=0, params=[0x22, 0x01] (ADJUSTABLE_DPI)
        // Response: params[0]=feature_index=0x07, params[1]=type=0x00, params[2]=0x00
        mock.on_short_request(0x01, 0x00, 0x01, &[0x22, 0x01], &[0x07, 0x00, 0x00]);
        let idx = lookup_feature_index(&mock, 0x01, 0x2201).unwrap();
        assert_eq!(idx, 0x07);
    }

    #[test]
    fn lookup_feature_index_not_found() {
        let mock = mock::MockTransport::new();
        // ROOT returns index 0 = feature not supported
        mock.on_short_request(0x01, 0x00, 0x01, &[0xFF, 0xFF], &[0x00, 0x00, 0x00]);
        let result = lookup_feature_index(&mock, 0x01, 0xFFFF);
        assert!(result.is_err());
    }

    #[test]
    fn hidpp_request_detects_error_response() {
        let mock = mock::MockTransport::new();
        // Craft a request
        let req = HidppRequest::new(0x01, 0x07, 0x01, vec![0x00]);
        let encoded = req.encode().unwrap();

        // Register an error response (feature_index=0xFF)
        let mut err_resp = vec![0x10, 0x01, 0xFF, 0x01];
        err_resp.extend_from_slice(&[0x07, 0x00, 0x02]); // feature=0x07, 0x00, code=0x02
        mock.on_request(encoded, err_resp);

        let result = hidpp_request(&mock, &req);
        assert!(result.is_err());
    }
}
