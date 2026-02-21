//! open-g-hub GUI: iced-based desktop application for mouse configuration.

use iced::widget::{
    button, column, container, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{Element, Length, Subscription, Task as IcedTask, Theme};
use std::array;
use std::time::{Duration, Instant};

use open_g_hub_core::device::{ButtonAction, PollingRate, G502_BUTTON_COUNT};
use open_g_hub_core::safety;
use open_g_hub_core::transport::HidTransport;

/// Device polling interval.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const DEVICE_INDEX_CANDIDATES: [u8; 2] = [0xFF, 0x01];

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    iced::application("Open G Hub", App::update, App::view)
        .theme(|_| Theme::Dark)
        .subscription(App::subscription)
        .run_with(|| (App::new(), IcedTask::none()))
}

struct GuiHidTransport {
    device: hidapi::HidDevice,
}

impl GuiHidTransport {
    fn open_first_supported() -> Result<Self, String> {
        let devices = open_g_hub_core::device::discover_devices().map_err(|e| e.to_string())?;
        let first = devices
            .first()
            .ok_or_else(|| "No supported Logitech G device found".to_string())?;

        let api = hidapi::HidApi::new().map_err(|e| format!("hidapi init: {e}"))?;
        let device = api.open(first.vid, first.pid).map_err(|e| {
            format!(
                "open HID device (VID=0x{:04X} PID=0x{:04X}): {e}",
                first.vid, first.pid
            )
        })?;

        Ok(Self { device })
    }
}

impl HidTransport for GuiHidTransport {
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

/// Application state.
struct App {
    dpi: u16,
    polling_rate: PollingRate,
    buttons: [ButtonAction; G502_BUTTON_COUNT],
    custom_cids: [String; G502_BUTTON_COUNT],
    connected: bool,
    status: String,
    last_poll: Instant,
    auto_poll: bool,
}

#[derive(Debug, Clone)]
enum Message {
    DpiChanged(u16),
    PollingRateSelected(PollingRate),
    ButtonChanged(usize, ButtonAction),
    CustomCidChanged(usize, String),
    ApplyCustomCid(usize),
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
            custom_cids: array::from_fn(|_| String::new()),
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
            Message::CustomCidChanged(idx, value) => {
                if idx < G502_BUTTON_COUNT {
                    self.custom_cids[idx] = value;
                }
            }
            Message::ApplyCustomCid(idx) => {
                if idx >= G502_BUTTON_COUNT {
                    self.status = "Invalid button index".into();
                    return IcedTask::none();
                }

                let Some(cid) = parse_custom_cid(&self.custom_cids[idx]) else {
                    self.status = format!(
                        "Invalid custom CID for button {}. Use hex like 0053 or 0x0053.",
                        idx
                    );
                    return IcedTask::none();
                };

                match GuiHidTransport::open_first_supported() {
                    Ok(transport) => {
                        match with_device_index(|dev_idx| {
                            open_g_hub_core::buttons::write_button_mapping_cid(
                                &transport, dev_idx, idx, cid,
                            )
                        }) {
                            Ok(()) => {
                                self.status =
                                    format!("Applied custom CID 0x{cid:04X} to button {}", idx);
                            }
                            Err(e) => self.status = format!("Custom keybind error: {e}"),
                        }
                    }
                    Err(e) => {
                        self.status = format!("Connection error: {e}");
                    }
                }
            }
            Message::ApplySettings => match GuiHidTransport::open_first_supported() {
                Ok(transport) => {
                    let result = with_device_index(|dev_idx| {
                        open_g_hub_core::dpi::write_dpi(&transport, dev_idx, self.dpi)?;
                        open_g_hub_core::report_rate::write_report_rate(
                            &transport,
                            dev_idx,
                            self.polling_rate,
                        )?;
                        for (idx, action) in self.buttons.iter().enumerate() {
                            open_g_hub_core::buttons::write_button_mapping(
                                &transport, dev_idx, idx, *action,
                            )?;
                        }
                        Ok(())
                    });

                    match result {
                        Ok(()) => {
                            self.status = format!(
                                "Applied: DPI {}, {}Hz, {} button mappings",
                                self.dpi,
                                self.polling_rate.as_hz(),
                                G502_BUTTON_COUNT
                            );
                        }
                        Err(e) => self.status = format!("Apply error: {e}"),
                    }
                }
                Err(e) => {
                    self.status = format!("Connection error: {e}");
                }
            },
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
        let status_icon = if self.connected { "[OK]" } else { "[--]" };
        let status_text = if self.connected {
            "Device connected"
        } else {
            "Device disconnected"
        };

        let header = column![
            text("Open G Hub").size(34),
            text("Configure Logitech mouse DPI, polling, and button mappings").size(16),
        ]
        .spacing(4);

        let device_card = container(
            column![
                text("Device").size(20),
                row![
                    text(format!("{status_icon} {status_text}")).size(16),
                    button("Refresh").on_press(Message::RefreshDevice),
                ]
                .spacing(14),
                text(&self.status).size(14),
            ]
            .spacing(8),
        )
        .padding(14)
        .width(Length::Fill);

        let rate_options: Vec<PollingRate> = PollingRate::ALL.to_vec();
        let performance_card = container(
            column![
                text("Performance").size(20),
                text(format!("DPI: {}", self.dpi)).size(16),
                slider(
                    (safety::DPI_MIN as f64)..=(safety::DPI_MAX as f64),
                    self.dpi as f64,
                    |val| Message::DpiChanged(val as u16),
                )
                .step(safety::DPI_STEP as f64),
                row![
                    text("Polling Rate").size(16),
                    pick_list(
                        rate_options,
                        Some(self.polling_rate),
                        Message::PollingRateSelected
                    ),
                ]
                .spacing(10),
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::Fill);

        let button_labels = [
            "Left (0)",
            "Right (1)",
            "Middle (2)",
            "Back (3)",
            "Forward (4)",
            "DPI (5)",
        ];

        let button_rows: Vec<Element<'_, Message>> = (0..G502_BUTTON_COUNT)
            .map(|i| {
                let actions: Vec<ButtonAction> = ButtonAction::ALL.to_vec();
                let row = column![
                    row![
                        text(button_labels[i]).size(15).width(Length::Fixed(105.0)),
                        pick_list(actions, Some(self.buttons[i]), move |action| {
                            Message::ButtonChanged(i, action)
                        })
                        .width(Length::Fill),
                    ]
                    .spacing(10),
                    row![
                        text_input("Custom CID (hex)", &self.custom_cids[i])
                            .on_input(move |v| Message::CustomCidChanged(i, v))
                            .width(Length::Fill),
                        button("Set Custom Keybind").on_press(Message::ApplyCustomCid(i)),
                    ]
                    .spacing(10),
                ]
                .spacing(6);

                container(row).padding(8).width(Length::Fill).into()
            })
            .collect();

        let mut button_col = column![
            text("Button Mappings").size(20),
            text("Use presets or set a custom HID++ CID keybinding per button").size(14),
        ]
        .spacing(8);

        for row in button_rows {
            button_col = button_col.push(row);
        }

        let button_card = container(scrollable(button_col).height(Length::Fixed(300.0)))
            .padding(14)
            .width(Length::Fill);

        let actions = row![
            button("Apply Settings").on_press(Message::ApplySettings),
            button("Save Profile").on_press(Message::SaveProfile),
            text(format!(
                "Last poll: {}s ago",
                self.last_poll.elapsed().as_secs()
            )),
        ]
        .spacing(12);

        let content = column![header, device_card, performance_card, button_card, actions]
            .spacing(14)
            .padding(20)
            .max_width(980);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into()
    }
}

fn parse_custom_cid(input: &str) -> Option<u16> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let raw = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    u16::from_str_radix(raw, 16).ok()
}
