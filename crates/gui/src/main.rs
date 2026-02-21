//! open-g-hub GUI: iced-based desktop application for mouse configuration.

use iced::widget::{button, column, container, pick_list, row, slider, text};
use iced::{Element, Length, Subscription, Task as IcedTask, Theme};
use std::time::{Duration, Instant};

use open_g_hub_core::device::{ButtonAction, PollingRate, G502_BUTTON_COUNT};
use open_g_hub_core::safety;

/// Device polling interval.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    iced::application("Open G Hub", App::update, App::view)
        .theme(|_| Theme::Dark)
        .subscription(App::subscription)
        .run_with(|| (App::new(), IcedTask::none()))
}

/// Application state.
struct App {
    /// Current DPI value.
    dpi: u16,
    /// Current polling rate.
    polling_rate: PollingRate,
    /// Button mappings.
    buttons: [ButtonAction; G502_BUTTON_COUNT],
    /// Device connection status.
    connected: bool,
    /// Status message.
    status: String,
    /// Last poll timestamp.
    last_poll: Instant,
    /// Auto-polling enabled.
    auto_poll: bool,
}

/// Messages that can update state.
#[derive(Debug, Clone)]
enum Message {
    DpiChanged(u16),
    PollingRateSelected(PollingRate),
    ButtonChanged(usize, ButtonAction),
    ApplySettings,
    RefreshDevice,
    PollTick,
    SaveProfile,
}

impl App {
    fn new() -> Self {
        let profile = open_g_hub_core::profile::load_profile().unwrap_or_default();
        let buttons: [ButtonAction; G502_BUTTON_COUNT] = {
            let mut arr = [ButtonAction::NoAction; G502_BUTTON_COUNT];
            for (i, btn) in profile.buttons.iter().enumerate().take(G502_BUTTON_COUNT) {
                arr[i] = *btn;
            }
            arr
        };

        Self {
            dpi: profile.dpi,
            polling_rate: profile.polling_rate,
            buttons,
            connected: false,
            status: "Scanning for devices...".into(),
            last_poll: Instant::now(),
            auto_poll: true,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.auto_poll {
            iced::time::every(POLL_INTERVAL).map(|_| Message::PollTick)
        } else {
            Subscription::none()
        }
    }

    fn poll_device(&mut self) {
        match open_g_hub_core::device::discover_devices() {
            Ok(devices) if !devices.is_empty() => {
                let was_disconnected = !self.connected;
                self.connected = true;
                let name = devices[0].model.name();
                if was_disconnected {
                    self.status = format!("Connected: {name}");
                }
                // Status only updates on state change to avoid flicker
            }
            Ok(_) => {
                let was_connected = self.connected;
                self.connected = false;
                if was_connected {
                    self.status = "Device disconnected.".into();
                } else {
                    self.status = "No Logitech G mice found.".into();
                }
            }
            Err(e) => {
                self.connected = false;
                self.status = format!("Scan error: {e}");
            }
        }
        self.last_poll = Instant::now();
    }

    fn update(&mut self, message: Message) -> IcedTask<Message> {
        match message {
            Message::DpiChanged(val) => {
                if let Ok(validated) = safety::validate_dpi(val) {
                    self.dpi = validated;
                }
            }
            Message::PollingRateSelected(rate) => {
                self.polling_rate = rate;
            }
            Message::ButtonChanged(idx, action) => {
                if idx < G502_BUTTON_COUNT {
                    self.buttons[idx] = action;
                }
            }
            Message::ApplySettings => {
                self.status = format!(
                    "Settings ready: DPI={}, Rate={}Hz (device write pending)",
                    self.dpi,
                    self.polling_rate.as_hz()
                );
            }
            Message::RefreshDevice => {
                self.poll_device();
            }
            Message::PollTick => {
                self.poll_device();
            }
            Message::SaveProfile => {
                let profile = open_g_hub_core::profile::Profile {
                    name: "Default".into(),
                    dpi: self.dpi,
                    polling_rate: self.polling_rate,
                    buttons: self.buttons.to_vec(),
                };
                match open_g_hub_core::profile::save_profile(&profile) {
                    Ok(()) => self.status = "Profile saved.".into(),
                    Err(e) => self.status = format!("Save error: {e}"),
                }
            }
        }
        IcedTask::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let title = text("Open G Hub").size(28);
        let status = text(&self.status).size(14);

        // Device status with indicator
        let status_icon = if self.connected { "[OK]" } else { "[--]" };
        let status_text = if self.connected {
            "Connected"
        } else {
            "Disconnected"
        };
        let connection = text(format!("{status_icon} {status_text}")).size(16);

        let refresh_btn = button("Refresh").on_press(Message::RefreshDevice);

        // DPI slider
        let dpi_label = text(format!("DPI: {}", self.dpi)).size(16);
        let dpi_slider = slider(
            (safety::DPI_MIN as f64)..=(safety::DPI_MAX as f64),
            self.dpi as f64,
            |val| Message::DpiChanged(val as u16),
        )
        .step(safety::DPI_STEP as f64);

        // Polling rate picker
        let rate_label = text("Polling Rate:").size(16);
        let rate_options: Vec<PollingRate> = PollingRate::ALL.to_vec();
        let rate_picker = pick_list(rate_options, Some(self.polling_rate), |rate| {
            Message::PollingRateSelected(rate)
        });

        // Button mappings
        let button_section = text("Button Mappings:").size(16);
        let button_labels = [
            "Left (0)",
            "Right (1)",
            "Middle (2)",
            "Back (3)",
            "Forward (4)",
            "DPI (5)",
        ];
        let button_rows: Vec<Element<Message>> = (0..G502_BUTTON_COUNT)
            .map(|i| {
                let label = text(button_labels[i]).size(14);
                let actions: Vec<ButtonAction> = ButtonAction::ALL.to_vec();
                let picker = pick_list(actions, Some(self.buttons[i]), move |action| {
                    Message::ButtonChanged(i, action)
                });
                row![label, picker].spacing(10).into()
            })
            .collect();

        let apply_btn = button("Apply Settings").on_press(Message::ApplySettings);
        let save_btn = button("Save Profile").on_press(Message::SaveProfile);

        let mut content = column![
            title,
            row![connection, refresh_btn].spacing(10),
            status,
            text("").size(8),
            dpi_label,
            dpi_slider,
            row![rate_label, rate_picker].spacing(10),
            text("").size(8),
            button_section,
        ]
        .spacing(8)
        .padding(20);

        for btn_row in button_rows {
            content = content.push(btn_row);
        }

        content = content.push(text("").size(8));
        content = content.push(row![apply_btn, save_btn].spacing(10));

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
