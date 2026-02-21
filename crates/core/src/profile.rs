//! Logitech G Hub profile compatibility layer.

use crate::device::{ButtonAction, PollingRate};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A saved mouse configuration profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile display name.
    pub name: String,
    /// DPI setting.
    pub dpi: u16,
    /// Polling rate.
    pub polling_rate: PollingRate,
    /// Button mappings (index = physical button, value = action).
    pub buttons: Vec<ButtonAction>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            dpi: 800,
            polling_rate: PollingRate::Hz1000,
            buttons: vec![
                ButtonAction::LeftClick,
                ButtonAction::RightClick,
                ButtonAction::MiddleClick,
                ButtonAction::Back,
                ButtonAction::Forward,
                ButtonAction::DpiCycleUp,
            ],
        }
    }
}

/// Logitech G Hub profile storage location.
///
/// Open G Hub no longer supports a separate `open-g-hub/profile.json` fallback.
/// Persistence must use Logitech G Hub's storage contract.
pub fn profile_path() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| Error::Profile("LOCALAPPDATA is not set".to_string()))?;
        Ok(PathBuf::from(local_app_data)
            .join("LGHUB")
            .join("settings.db"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err(Error::Profile(
            "Logitech G Hub-compatible profile storage is only available on Windows".to_string(),
        ))
    }
}

/// Save a profile to Logitech G Hub storage.
///
/// Legacy storage is removed; this now fails fast until direct G Hub format writes
/// are implemented.
pub fn save_profile(_profile: &Profile) -> Result<()> {
    let path = profile_path()?;
    Err(Error::Profile(format!(
        "legacy open-g-hub profile storage has been removed; direct Logitech G Hub format write not implemented yet (expected location: {})",
        path.display()
    )))
}

/// Load a profile from Logitech G Hub storage.
///
/// Legacy storage is removed; this now fails fast until direct G Hub format reads
/// are implemented.
pub fn load_profile() -> Result<Profile> {
    let path = profile_path()?;
    Err(Error::Profile(format!(
        "legacy open-g-hub profile storage has been removed; direct Logitech G Hub format read not implemented yet (expected location: {})",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::G502_BUTTON_COUNT;

    #[test]
    fn default_profile_has_correct_button_count() {
        let p = Profile::default();
        assert_eq!(p.buttons.len(), G502_BUTTON_COUNT);
    }

    #[test]
    fn profile_type_serialization_roundtrip() {
        let profile = Profile::default();
        let json = serde_json::to_string(&profile).expect("serialize profile");
        let deserialized: Profile = serde_json::from_str(&json).expect("deserialize profile");
        assert_eq!(deserialized.dpi, profile.dpi);
    }

    #[test]
    fn profile_path_non_windows_is_rejected() {
        #[cfg(not(target_os = "windows"))]
        {
            assert!(profile_path().is_err());
        }
    }
}
