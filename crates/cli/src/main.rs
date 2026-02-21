//! open-g-hub CLI: command-line mouse configuration tool.

use anyhow::Result;
use clap::{Parser, Subcommand};
use open_g_hub_core::transport::HidTransport;

const DEVICE_INDEX_CANDIDATES: [u8; 2] = [0xFF, 0x01];

struct CliHidTransport {
    device: hidapi::HidDevice,
}

impl CliHidTransport {
    fn open_first_supported() -> Result<Self> {
        let devices = open_g_hub_core::device::discover_devices()?;
        let first = devices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No supported Logitech G device found"))?;

        let api = hidapi::HidApi::new().map_err(|e| anyhow::anyhow!("hidapi init: {e}"))?;
        let device = api.open(first.vid, first.pid).map_err(|e| {
            anyhow::anyhow!(
                "open HID device (VID=0x{:04X} PID=0x{:04X}): {e}",
                first.vid,
                first.pid
            )
        })?;

        Ok(Self { device })
    }
}

impl HidTransport for CliHidTransport {
    fn send_report(&self, data: &[u8]) -> open_g_hub_core::error::Result<Vec<u8>> {
        self.device
            .write(data)
            .map_err(|e| open_g_hub_core::error::Error::Hid(format!("write: {e}")))?;

        let mut response = [0u8; 64];
        let n = self
            .device
            .read_timeout(&mut response, 1000)
            .map_err(|e| open_g_hub_core::error::Error::Hid(format!("read_timeout: {e}")))?;

        if n == 0 {
            return Err(open_g_hub_core::error::Error::Timeout(
                "hid_read timed out after 1000ms".to_string(),
            ));
        }

        Ok(response[..n].to_vec())
    }
}

fn with_device_index<T>(
    mut op: impl FnMut(u8) -> open_g_hub_core::error::Result<T>,
) -> open_g_hub_core::error::Result<T> {
    let mut last_error = None;
    for idx in DEVICE_INDEX_CANDIDATES {
        match op(idx) {
            Ok(v) => return Ok(v),
            Err(e) => last_error = Some(e),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        open_g_hub_core::error::Error::DeviceNotFound("no usable device index".to_string())
    }))
}

#[derive(Parser)]
#[command(
    name = "open-g-hub",
    version,
    about = "Open-source Logitech mouse configuration"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List connected Logitech G mice.
    ListDevices,
    /// Get current DPI setting.
    GetDpi,
    /// Set DPI value (100-25600, rounded to nearest 50).
    SetDpi {
        /// DPI value to set.
        value: u16,
    },
    /// Get current polling rate.
    GetRate,
    /// Set polling rate (125, 250, 500, or 1000 Hz).
    SetRate {
        /// Polling rate in Hz.
        value: u16,
    },
    /// Show current button mappings.
    GetButtons,
    /// Remap a button.
    SetButton {
        /// Button index (0-5).
        index: usize,
        /// Action: left, right, middle, back, forward, dpi-up, dpi-down, none.
        action: String,
    },
    /// Save current settings to a profile.
    SaveProfile,
    /// Load and apply a saved profile.
    LoadProfile,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::ListDevices => {
            let devices = open_g_hub_core::device::discover_devices()?;
            if devices.is_empty() {
                println!("No Logitech G mice found.");
                println!("Ensure your mouse is connected and drivers are set up.");
            } else {
                for dev in &devices {
                    println!(
                        "{} (VID: 0x{:04X}, PID: 0x{:04X}, path: {})",
                        dev.model.name(),
                        dev.vid,
                        dev.pid,
                        dev.path
                    );
                }
            }
        }
        Commands::GetDpi => {
            let transport = CliHidTransport::open_first_supported()?;
            let dpi = with_device_index(|idx| open_g_hub_core::dpi::read_dpi(&transport, idx))?;
            println!("Current DPI: {dpi}");
        }
        Commands::SetDpi { value } => {
            let transport = CliHidTransport::open_first_supported()?;
            let validated =
                with_device_index(|idx| open_g_hub_core::dpi::write_dpi(&transport, idx, value))?;
            println!("DPI set to {validated}");
        }
        Commands::GetRate => {
            let transport = CliHidTransport::open_first_supported()?;
            let rate = with_device_index(|idx| {
                open_g_hub_core::report_rate::read_report_rate(&transport, idx)
            })?;
            println!("Current polling rate: {} Hz", rate.as_hz());
        }
        Commands::SetRate { value } => {
            let validated = open_g_hub_core::safety::validate_polling_rate(value)?;
            let transport = CliHidTransport::open_first_supported()?;
            with_device_index(|idx| {
                open_g_hub_core::report_rate::write_report_rate(&transport, idx, validated)
            })?;
            println!("Polling rate set to {} Hz", validated.as_hz());
        }
        Commands::GetButtons => {
            let transport = CliHidTransport::open_first_supported()?;
            for index in 0..open_g_hub_core::device::G502_BUTTON_COUNT {
                let action = with_device_index(|idx| {
                    open_g_hub_core::buttons::read_button_mapping(&transport, idx, index)
                })?;
                println!("Button {index}: {}", action.label());
            }
        }
        Commands::SetButton { index, action } => {
            open_g_hub_core::safety::validate_button_index(index)?;
            let parsed_action = open_g_hub_core::device::ButtonAction::from_name(&action)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Unknown button action '{}'. Valid actions: left, right, middle, back, forward, dpi-up, dpi-down, none",
                        action
                    )
                })?;
            let transport = CliHidTransport::open_first_supported()?;
            with_device_index(|idx| {
                open_g_hub_core::buttons::write_button_mapping(
                    &transport,
                    idx,
                    index,
                    parsed_action,
                )
            })?;
            println!("Set button {index} to '{}'", parsed_action.label());
        }
        Commands::SaveProfile => {
            let profile = open_g_hub_core::profile::Profile::default();
            open_g_hub_core::profile::save_profile(&profile)?;
            println!(
                "Profile saved to {:?}",
                open_g_hub_core::profile::profile_path()?
            );
        }
        Commands::LoadProfile => {
            let profile = open_g_hub_core::profile::load_profile()?;
            println!("Loaded profile: {}", profile.name);
            println!("  DPI: {}", profile.dpi);
            println!("  Polling rate: {} Hz", profile.polling_rate.as_hz());
            for (i, btn) in profile.buttons.iter().enumerate() {
                println!("  Button {i}: {}", btn.label());
            }
        }
    }

    Ok(())
}
