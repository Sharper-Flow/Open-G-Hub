//! Button remapping via HID++ 2.0 REPROG_CONTROLS_V4 feature (0x1B04).
//!
//! HID++ 2.0 REPROG_CONTROLS_V4 functions:
//!   - Function 0: getCount → params[0] = number of reprogrammable controls
//!   - Function 1: getControlInfo(index) → CID (control ID), task ID, flags
//!   - Function 2: getControlReporting(CID) → current remap for a control
//!   - Function 3: setControlReporting(CID, flags, remap) → remap a control
//!
//! Control IDs (CIDs) are 16-bit identifiers for each physical button.
//! Each CID can be remapped to another CID's action.

use crate::device::ButtonAction;
use crate::error::{Error, Result};
use crate::hidpp::{self, HidppRequest};
use crate::safety;
use crate::transport::{hidpp_request, lookup_feature_index, HidTransport};

/// Well-known HID++ Control IDs (CIDs) for G502 buttons.
pub mod cids {
    /// Left click.
    pub const LEFT_CLICK: u16 = 0x0050;
    /// Right click.
    pub const RIGHT_CLICK: u16 = 0x0051;
    /// Middle click.
    pub const MIDDLE_CLICK: u16 = 0x0052;
    /// Back (thumb button).
    pub const BACK: u16 = 0x0053;
    /// Forward (thumb button).
    pub const FORWARD: u16 = 0x0056;
    /// DPI cycle up (sniper/G-shift area).
    pub const DPI_UP: u16 = 0x004D;
    /// DPI cycle down.
    pub const DPI_DOWN: u16 = 0x004E;
    /// No action / disabled.
    pub const NO_ACTION: u16 = 0x0000;
}

/// Convert a ButtonAction to its HID++ Control ID.
pub fn action_to_cid(action: ButtonAction) -> u16 {
    match action {
        ButtonAction::LeftClick => cids::LEFT_CLICK,
        ButtonAction::RightClick => cids::RIGHT_CLICK,
        ButtonAction::MiddleClick => cids::MIDDLE_CLICK,
        ButtonAction::Back => cids::BACK,
        ButtonAction::Forward => cids::FORWARD,
        ButtonAction::DpiCycleUp => cids::DPI_UP,
        ButtonAction::DpiCycleDown => cids::DPI_DOWN,
        ButtonAction::NoAction => cids::NO_ACTION,
    }
}

/// Convert a HID++ Control ID to a ButtonAction.
pub fn cid_to_action(cid: u16) -> ButtonAction {
    match cid {
        cids::LEFT_CLICK => ButtonAction::LeftClick,
        cids::RIGHT_CLICK => ButtonAction::RightClick,
        cids::MIDDLE_CLICK => ButtonAction::MiddleClick,
        cids::BACK => ButtonAction::Back,
        cids::FORWARD => ButtonAction::Forward,
        cids::DPI_UP => ButtonAction::DpiCycleUp,
        cids::DPI_DOWN => ButtonAction::DpiCycleDown,
        _ => ButtonAction::NoAction,
    }
}

/// Information about a single reprogrammable control.
#[derive(Debug, Clone)]
pub struct ControlInfo {
    /// Control ID — unique identifier for this button.
    pub cid: u16,
    /// Task ID — the default action for this control.
    pub task_id: u16,
    /// Flags (virtual, persist, divert, reprog).
    pub flags: u8,
}

/// Read the number of reprogrammable controls.
pub fn read_control_count(transport: &dyn HidTransport, device_index: u8) -> Result<u8> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::REPROG_CONTROLS_V4)?;

    // getCount: function 0
    let req = HidppRequest::new(device_index, feature_idx, 0x00, vec![]);
    let resp = hidpp_request(transport, &req)?;

    Ok(resp.params[0])
}

/// Read info for a control at the given index.
pub fn read_control_info(
    transport: &dyn HidTransport,
    device_index: u8,
    index: u8,
) -> Result<ControlInfo> {
    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::REPROG_CONTROLS_V4)?;

    // getControlInfo: function 1, params[0]=index
    let req = HidppRequest::new(device_index, feature_idx, 0x01, vec![index]);
    let resp = hidpp_request(transport, &req)?;

    // Response: params[0..1] = CID, params[2..3] = task_id, params[4] = flags
    if resp.params.len() < 5 {
        return Err(Error::HidppProtocol {
            feature: hidpp::features::REPROG_CONTROLS_V4,
            code: 0xFE,
        });
    }

    Ok(ControlInfo {
        cid: ((resp.params[0] as u16) << 8) | (resp.params[1] as u16),
        task_id: ((resp.params[2] as u16) << 8) | (resp.params[3] as u16),
        flags: resp.params[4],
    })
}

/// Read the current remapping for a specific button by index.
///
/// Returns the ButtonAction that the button at `button_index` is currently mapped to.
pub fn read_button_mapping(
    transport: &dyn HidTransport,
    device_index: u8,
    button_index: usize,
) -> Result<ButtonAction> {
    safety::validate_button_index(button_index)?;

    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::REPROG_CONTROLS_V4)?;

    // First get the CID for this button index
    let info =
        read_control_info_with_feature(transport, device_index, feature_idx, button_index as u8)?;

    // getControlReporting: function 2, params[0..1]=CID
    let req = HidppRequest::new(
        device_index,
        feature_idx,
        0x02,
        vec![(info.cid >> 8) as u8, (info.cid & 0xFF) as u8],
    );
    let resp = hidpp_request(transport, &req)?;

    // Response: params[0..1]=CID, params[2]=flags, params[3..4]=remap CID
    let remap_cid = if resp.params.len() >= 5 {
        ((resp.params[3] as u16) << 8) | (resp.params[4] as u16)
    } else {
        info.cid // no remap, returns self
    };

    Ok(cid_to_action(remap_cid))
}

/// Write a button remapping.
pub fn write_button_mapping(
    transport: &dyn HidTransport,
    device_index: u8,
    button_index: usize,
    action: ButtonAction,
) -> Result<()> {
    safety::validate_button_index(button_index)?;

    let feature_idx =
        lookup_feature_index(transport, device_index, hidpp::features::REPROG_CONTROLS_V4)?;

    // Get the CID for this button index
    let info =
        read_control_info_with_feature(transport, device_index, feature_idx, button_index as u8)?;

    let remap_cid = action_to_cid(action);

    // setControlReporting: function 3
    // params: CID[0..1], flags(0x10=remap), remap_CID[0..1]
    let req = HidppRequest::new(
        device_index,
        feature_idx,
        0x03,
        vec![
            (info.cid >> 8) as u8,
            (info.cid & 0xFF) as u8,
            0x10, // remap flag
            (remap_cid >> 8) as u8,
            (remap_cid & 0xFF) as u8,
        ],
    );
    let _resp = hidpp_request(transport, &req)?;

    Ok(())
}

/// Internal: read control info when feature index is already known.
fn read_control_info_with_feature(
    transport: &dyn HidTransport,
    device_index: u8,
    feature_idx: u8,
    index: u8,
) -> Result<ControlInfo> {
    let req = HidppRequest::new(device_index, feature_idx, 0x01, vec![index]);
    let resp = hidpp_request(transport, &req)?;

    if resp.params.len() < 5 {
        return Err(Error::HidppProtocol {
            feature: hidpp::features::REPROG_CONTROLS_V4,
            code: 0xFE,
        });
    }

    Ok(ControlInfo {
        cid: ((resp.params[0] as u16) << 8) | (resp.params[1] as u16),
        task_id: ((resp.params[2] as u16) << 8) | (resp.params[3] as u16),
        flags: resp.params[4],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::MockTransport;

    const DEV_IDX: u8 = 0x01;
    const BTN_FEATURE_IDX: u8 = 0x09;

    fn setup_button_feature_lookup(mock: &MockTransport) {
        // ROOT lookup: REPROG_CONTROLS_V4 (0x1B04) → feature index 0x09
        mock.on_short_request(
            DEV_IDX,
            0x00,
            0x01,
            &[0x1B, 0x04],
            &[BTN_FEATURE_IDX, 0x00, 0x00],
        );
    }

    #[test]
    fn read_control_count_returns_value() {
        let mock = MockTransport::new();
        setup_button_feature_lookup(&mock);

        mock.on_short_request(
            DEV_IDX,
            BTN_FEATURE_IDX,
            0x01, // function=0 << 4 | sw_id=1
            &[],
            &[0x06, 0x00, 0x00], // 6 controls
        );

        let count = read_control_count(&mock, DEV_IDX).unwrap();
        assert_eq!(count, 6);
    }

    #[test]
    fn read_control_info_parses_response() {
        let mock = MockTransport::new();
        setup_button_feature_lookup(&mock);

        // getControlInfo for index 0: CID=0x0050(left click), task=0x0038, flags=0x01
        mock.on_long_request(
            DEV_IDX,
            BTN_FEATURE_IDX,
            0x11, // function=1 << 4 | sw_id=1
            &[0x00],
            &[
                0x00, 0x50, 0x00, 0x38, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        let info = read_control_info(&mock, DEV_IDX, 0).unwrap();
        assert_eq!(info.cid, 0x0050);
        assert_eq!(info.task_id, 0x0038);
        assert_eq!(info.flags, 0x01);
    }

    #[test]
    fn action_cid_roundtrip() {
        for action in ButtonAction::ALL {
            if *action == ButtonAction::NoAction {
                continue; // NoAction maps to 0x0000, which maps back to NoAction
            }
            let cid = action_to_cid(*action);
            let back = cid_to_action(cid);
            assert_eq!(back, *action, "roundtrip failed for {:?}", action);
        }
    }

    #[test]
    fn no_action_maps_to_zero() {
        assert_eq!(action_to_cid(ButtonAction::NoAction), 0x0000);
        assert_eq!(cid_to_action(0x0000), ButtonAction::NoAction);
    }

    #[test]
    fn unknown_cid_maps_to_no_action() {
        assert_eq!(cid_to_action(0xFFFF), ButtonAction::NoAction);
    }

    #[test]
    fn write_button_rejects_invalid_index() {
        let mock = MockTransport::new();
        let result = write_button_mapping(&mock, DEV_IDX, 10, ButtonAction::LeftClick);
        assert!(result.is_err());
    }

    #[test]
    fn read_button_mapping_returns_action() {
        let mock = MockTransport::new();
        setup_button_feature_lookup(&mock);

        // getControlInfo for index 0: CID=0x0050 (left click)
        mock.on_long_request(
            DEV_IDX,
            BTN_FEATURE_IDX,
            0x11,
            &[0x00],
            &[
                0x00, 0x50, 0x00, 0x38, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        // getControlReporting for CID 0x0050: remapped to 0x0051 (right click)
        mock.on_long_request(
            DEV_IDX,
            BTN_FEATURE_IDX,
            0x21, // function=2 << 4 | sw_id=1
            &[0x00, 0x50],
            &[
                0x00, 0x50, 0x00, 0x00, 0x51, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        let action = read_button_mapping(&mock, DEV_IDX, 0).unwrap();
        assert_eq!(action, ButtonAction::RightClick);
    }

    #[test]
    fn write_button_mapping_sends_remap() {
        let mock = MockTransport::new();
        setup_button_feature_lookup(&mock);

        // getControlInfo for index 1: CID=0x0051 (right click)
        mock.on_long_request(
            DEV_IDX,
            BTN_FEATURE_IDX,
            0x11,
            &[0x01],
            &[
                0x00, 0x51, 0x00, 0x39, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        // setControlReporting: remap CID 0x0051 to CID 0x0053 (back)
        mock.on_long_request(
            DEV_IDX,
            BTN_FEATURE_IDX,
            0x31, // function=3 << 4 | sw_id=1
            &[0x00, 0x51, 0x10, 0x00, 0x53],
            &[
                0x00, 0x51, 0x10, 0x00, 0x53, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ],
        );

        write_button_mapping(&mock, DEV_IDX, 1, ButtonAction::Back).unwrap();
    }
}
