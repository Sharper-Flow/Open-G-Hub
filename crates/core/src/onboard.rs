//! Onboard profile management via HID++ 2.0 ONBOARD_PROFILES feature (0x8100).
//!
//! HID++ 2.0 ONBOARD_PROFILES functions:
//!   - Function 0: getDescription → memory model, profile count, button count, etc.
//!   - Function 1: setOnboardMode(mode) → 1=host-mode, 2=onboard-mode
//!   - Function 2: getCurrentProfile → currently active profile page/offset
//!   - Function 3: setCurrentProfile(page, offset) → switch active profile
//!
//! For G502: typically 1 onboard profile at page 0, offset 0.
//! "Host mode" means the computer controls settings; "onboard mode" means
//! the mouse uses its stored profile.

use crate::error::Result;
use crate::hidpp::{self, HidppRequest};
use crate::transport::{hidpp_request, lookup_feature_index, HidTransport};

/// Onboard profile mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardMode {
    /// Host controls settings (software manages device).
    Host = 1,
    /// Mouse uses its stored onboard profile.
    Onboard = 2,
}

impl OnboardMode {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Host),
            2 => Some(Self::Onboard),
            _ => None,
        }
    }
}

/// Description of the device's onboard profile capabilities.
#[derive(Debug, Clone)]
pub struct ProfileDescription {
    /// Memory model (device-specific).
    pub memory_model: u8,
    /// Number of profiles stored on device.
    pub profile_count: u8,
    /// Number of buttons per profile.
    pub button_count: u8,
    /// Number of sectors.
    pub sector_count: u8,
}

/// Read the onboard profile capabilities.
pub fn read_profile_description(
    transport: &dyn HidTransport,
    device_index: u8,
) -> Result<ProfileDescription> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::ONBOARD_PROFILES)?;

    // getDescription: function 0
    let req = HidppRequest::new(device_index, feature_idx, 0x00, vec![]);
    let resp = hidpp_request(transport, &req)?;

    Ok(ProfileDescription {
        memory_model: resp.params[0],
        profile_count: resp.params[1],
        button_count: resp.params[2],
        sector_count: resp.params[3],
    })
}

/// Set the onboard mode (host vs onboard).
pub fn set_onboard_mode(
    transport: &dyn HidTransport,
    device_index: u8,
    mode: OnboardMode,
) -> Result<()> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::ONBOARD_PROFILES)?;

    // setOnboardMode: function 1, params[0]=mode
    let req = HidppRequest::new(device_index, feature_idx, 0x01, vec![mode as u8]);
    let _resp = hidpp_request(transport, &req)?;

    Ok(())
}

/// Read the currently active profile index.
///
/// Returns (page, offset) identifying the active profile in device memory.
pub fn get_current_profile(transport: &dyn HidTransport, device_index: u8) -> Result<(u8, u8)> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::ONBOARD_PROFILES)?;

    // getCurrentProfile: function 2
    let req = HidppRequest::new(device_index, feature_idx, 0x02, vec![]);
    let resp = hidpp_request(transport, &req)?;

    Ok((resp.params[0], resp.params[1]))
}

/// Switch to a specific onboard profile.
pub fn set_current_profile(
    transport: &dyn HidTransport,
    device_index: u8,
    page: u8,
    offset: u8,
) -> Result<()> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::ONBOARD_PROFILES)?;

    // setCurrentProfile: function 3, params=[page, offset]
    let req = HidppRequest::new(device_index, feature_idx, 0x03, vec![page, offset]);
    let _resp = hidpp_request(transport, &req)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;

    const DEV_IDX: u8 = 0x01;
    const PROFILE_FEATURE_IDX: u8 = 0x0A;

    fn setup_profile_feature_lookup(mock: &MockTransport) {
        mock.on_short_request(
            DEV_IDX,
            0x00,
            0x01,
            &[0x81, 0x00],
            &[PROFILE_FEATURE_IDX, 0x00, 0x00],
        );
    }

    #[test]
    fn read_profile_description_parses() {
        let mock = MockTransport::new();
        setup_profile_feature_lookup(&mock);

        // getDescription returns 4+ bytes, use long response.
        // memory_model=2, profiles=1, buttons=6, sectors=4
        mock.on_long_request(
            DEV_IDX,
            PROFILE_FEATURE_IDX,
            0x01, // function=0 << 4 | sw_id=1
            &[],
            &[
                0x02, 0x01, 0x06, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        let desc = read_profile_description(&mock, DEV_IDX).unwrap();
        assert_eq!(desc.memory_model, 2);
        assert_eq!(desc.profile_count, 1);
        assert_eq!(desc.button_count, 6);
        assert_eq!(desc.sector_count, 4);
    }

    #[test]
    fn set_onboard_mode_host() {
        let mock = MockTransport::new();
        setup_profile_feature_lookup(&mock);

        mock.on_short_request(
            DEV_IDX,
            PROFILE_FEATURE_IDX,
            0x11, // function=1 << 4 | sw_id=1
            &[0x01],
            &[0x01, 0x00, 0x00],
        );

        set_onboard_mode(&mock, DEV_IDX, OnboardMode::Host).unwrap();
    }

    #[test]
    fn set_onboard_mode_onboard() {
        let mock = MockTransport::new();
        setup_profile_feature_lookup(&mock);

        mock.on_short_request(
            DEV_IDX,
            PROFILE_FEATURE_IDX,
            0x11,
            &[0x02],
            &[0x02, 0x00, 0x00],
        );

        set_onboard_mode(&mock, DEV_IDX, OnboardMode::Onboard).unwrap();
    }

    #[test]
    fn get_current_profile_returns_page_offset() {
        let mock = MockTransport::new();
        setup_profile_feature_lookup(&mock);

        mock.on_short_request(
            DEV_IDX,
            PROFILE_FEATURE_IDX,
            0x21, // function=2 << 4 | sw_id=1
            &[],
            &[0x00, 0x00, 0x00], // page=0, offset=0
        );

        let (page, offset) = get_current_profile(&mock, DEV_IDX).unwrap();
        assert_eq!(page, 0);
        assert_eq!(offset, 0);
    }

    #[test]
    fn set_current_profile_sends_params() {
        let mock = MockTransport::new();
        setup_profile_feature_lookup(&mock);

        mock.on_short_request(
            DEV_IDX,
            PROFILE_FEATURE_IDX,
            0x31, // function=3 << 4 | sw_id=1
            &[0x00, 0x00],
            &[0x00, 0x00, 0x00],
        );

        set_current_profile(&mock, DEV_IDX, 0, 0).unwrap();
    }

    #[test]
    fn onboard_mode_from_byte() {
        assert_eq!(OnboardMode::from_byte(1), Some(OnboardMode::Host));
        assert_eq!(OnboardMode::from_byte(2), Some(OnboardMode::Onboard));
        assert_eq!(OnboardMode::from_byte(0), None);
        assert_eq!(OnboardMode::from_byte(3), None);
    }
}
