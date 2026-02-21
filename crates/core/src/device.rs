//! Device model: discovery, connection, and feature access.

use crate::error::{Error, Result};
use crate::{pids, LOGITECH_VID};
use tracing::{debug, info};

/// Supported Logitech mouse models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseModel {
    G502Lightspeed,
    G502Hero,
}

impl MouseModel {
    /// Look up model from USB product ID.
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            pids::G502_LIGHTSPEED => Some(Self::G502Lightspeed),
            pids::G502_HERO => Some(Self::G502Hero),
            _ => None,
        }
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::G502Lightspeed => "Logitech G502 Lightspeed",
            Self::G502Hero => "Logitech G502 HERO",
        }
    }

    /// USB Product ID.
    pub fn pid(&self) -> u16 {
        match self {
            Self::G502Lightspeed => pids::G502_LIGHTSPEED,
            Self::G502Hero => pids::G502_HERO,
        }
    }
}

/// Information about a discovered Logitech device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub model: MouseModel,
    pub vid: u16,
    pub pid: u16,
    pub path: String,
    pub serial: Option<String>,
}

/// Discover all connected Logitech G mice.
///
/// Enumerates USB HID devices and returns info for any recognized models.
pub fn discover_devices() -> Result<Vec<DeviceInfo>> {
    debug!("Starting HID device enumeration");
    let api = hidapi::HidApi::new().map_err(|e| Error::Hid(e.to_string()))?;

    let mut devices = Vec::new();
    for info in api.device_list() {
        if info.vendor_id() != LOGITECH_VID {
            continue;
        }

        if let Some(model) = MouseModel::from_pid(info.product_id()) {
            info!(
                model = model.name(),
                vid = format_args!("0x{:04X}", info.vendor_id()),
                pid = format_args!("0x{:04X}", info.product_id()),
                path = %info.path().to_string_lossy(),
                "Found Logitech device"
            );
            devices.push(DeviceInfo {
                model,
                vid: info.vendor_id(),
                pid: info.product_id(),
                path: info.path().to_string_lossy().into_owned(),
                serial: info.serial_number().map(|s| s.to_string()),
            });
        }
    }

    debug!(count = devices.len(), "Device enumeration complete");
    Ok(devices)
}

/// Polling rate options supported by G502 mice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u16)]
pub enum PollingRate {
    Hz125 = 125,
    Hz250 = 250,
    Hz500 = 500,
    Hz1000 = 1000,
}

impl PollingRate {
    /// Convert from raw Hz value.
    pub fn from_hz(hz: u16) -> Option<Self> {
        match hz {
            125 => Some(Self::Hz125),
            250 => Some(Self::Hz250),
            500 => Some(Self::Hz500),
            1000 => Some(Self::Hz1000),
            _ => None,
        }
    }

    /// Get the Hz value.
    pub fn as_hz(&self) -> u16 {
        *self as u16
    }

    /// All supported rates.
    pub const ALL: &'static [PollingRate] = &[
        PollingRate::Hz125,
        PollingRate::Hz250,
        PollingRate::Hz500,
        PollingRate::Hz1000,
    ];
}

impl std::fmt::Display for PollingRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} Hz", self.as_hz())
    }
}

/// Standard mouse button actions for remapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ButtonAction {
    LeftClick,
    RightClick,
    MiddleClick,
    Back,
    Forward,
    DpiCycleUp,
    DpiCycleDown,
    NoAction,
}

impl ButtonAction {
    /// All available actions.
    pub const ALL: &'static [ButtonAction] = &[
        ButtonAction::LeftClick,
        ButtonAction::RightClick,
        ButtonAction::MiddleClick,
        ButtonAction::Back,
        ButtonAction::Forward,
        ButtonAction::DpiCycleUp,
        ButtonAction::DpiCycleDown,
        ButtonAction::NoAction,
    ];

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::LeftClick => "Left Click",
            Self::RightClick => "Right Click",
            Self::MiddleClick => "Middle Click",
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::DpiCycleUp => "DPI Cycle Up",
            Self::DpiCycleDown => "DPI Cycle Down",
            Self::NoAction => "No Action",
        }
    }

    /// Parse a button action from a CLI-friendly string.
    ///
    /// Accepts common name variants (case-insensitive):
    /// - "left", "left-click" → LeftClick
    /// - "right", "right-click" → RightClick
    /// - "middle", "middle-click" → MiddleClick
    /// - "back" → Back
    /// - "forward" → Forward
    /// - "dpi-up", "dpi-cycle-up" → DpiCycleUp
    /// - "dpi-down", "dpi-cycle-down" → DpiCycleDown
    /// - "none", "no-action", "disabled" → NoAction
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "left" | "left-click" | "leftclick" => Some(Self::LeftClick),
            "right" | "right-click" | "rightclick" => Some(Self::RightClick),
            "middle" | "middle-click" | "middleclick" => Some(Self::MiddleClick),
            "back" => Some(Self::Back),
            "forward" => Some(Self::Forward),
            "dpi-up" | "dpi-cycle-up" | "dpiup" => Some(Self::DpiCycleUp),
            "dpi-down" | "dpi-cycle-down" | "dpidown" => Some(Self::DpiCycleDown),
            "none" | "no-action" | "noaction" | "disabled" => Some(Self::NoAction),
            _ => None,
        }
    }
}

impl std::fmt::Display for ButtonAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Number of programmable buttons on G502.
pub const G502_BUTTON_COUNT: usize = 6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_model_from_known_pid() {
        assert_eq!(
            MouseModel::from_pid(0xC08D),
            Some(MouseModel::G502Lightspeed)
        );
        assert_eq!(MouseModel::from_pid(0xC08B), Some(MouseModel::G502Hero));
    }

    #[test]
    fn mouse_model_from_unknown_pid() {
        assert_eq!(MouseModel::from_pid(0x1234), None);
    }

    #[test]
    fn polling_rate_roundtrip() {
        for rate in PollingRate::ALL {
            assert_eq!(PollingRate::from_hz(rate.as_hz()), Some(*rate));
        }
    }

    #[test]
    fn polling_rate_rejects_invalid() {
        assert_eq!(PollingRate::from_hz(200), None);
        assert_eq!(PollingRate::from_hz(0), None);
    }

    #[test]
    fn button_action_labels_non_empty() {
        for action in ButtonAction::ALL {
            assert!(!action.label().is_empty());
        }
    }

    #[test]
    fn button_action_from_name_accepts_variants() {
        assert_eq!(
            ButtonAction::from_name("left"),
            Some(ButtonAction::LeftClick)
        );
        assert_eq!(
            ButtonAction::from_name("Left-Click"),
            Some(ButtonAction::LeftClick)
        );
        assert_eq!(
            ButtonAction::from_name("RIGHT"),
            Some(ButtonAction::RightClick)
        );
        assert_eq!(
            ButtonAction::from_name("middle"),
            Some(ButtonAction::MiddleClick)
        );
        assert_eq!(ButtonAction::from_name("back"), Some(ButtonAction::Back));
        assert_eq!(
            ButtonAction::from_name("forward"),
            Some(ButtonAction::Forward)
        );
        assert_eq!(
            ButtonAction::from_name("dpi-up"),
            Some(ButtonAction::DpiCycleUp)
        );
        assert_eq!(
            ButtonAction::from_name("dpi-down"),
            Some(ButtonAction::DpiCycleDown)
        );
        assert_eq!(
            ButtonAction::from_name("none"),
            Some(ButtonAction::NoAction)
        );
        assert_eq!(
            ButtonAction::from_name("disabled"),
            Some(ButtonAction::NoAction)
        );
    }

    #[test]
    fn button_action_from_name_rejects_unknown() {
        assert_eq!(ButtonAction::from_name("shoot"), None);
        assert_eq!(ButtonAction::from_name(""), None);
        assert_eq!(ButtonAction::from_name("macro1"), None);
    }
}
