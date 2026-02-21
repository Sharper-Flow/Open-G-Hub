# Open G Hub

Open-source cross-platform replacement for Logitech G Hub mouse configuration software.

Communicates directly with Logitech G mice via HID++ 2.0 protocol over USB HID. No cloud. No telemetry. No bloat.

## Features

- **DPI configuration** (100-25,600, step 50) via HID++ ADJUSTABLE_DPI (0x2201)
- **Polling rate** (125/250/500/1000 Hz) via HID++ REPORT_RATE (0x8060)
- **Button remapping** (6 programmable buttons) via HID++ REPROG_CONTROLS_V4 (0x1B04)
- **Onboard profile** management via HID++ ONBOARD_PROFILES (0x8100)
- **Profile persistence target**: Logitech G Hub storage contract (legacy `open-g-hub/profile.json` removed)
- **Safety validation** (all writes bounds-checked before reaching device)
- **Structured logging** (tracing crate, `RUST_LOG` env var)

## Supported Devices

| Device | VID | PID | Status |
|--------|-----|-----|--------|
| G502 Lightspeed | 0x046D | 0xC08D | Supported |
| G502 HERO | 0x046D | 0xC08B | Supported |

## Installation

### From Source

```bash
# Prerequisites: Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/user/open-g-hub.git
cd open-g-hub
cargo build --release

# Run CLI
./target/release/open-g-hub-cli list-devices

# Run GUI
./target/release/open-g-hub-gui
```

### Linux: udev Rules

Create `/etc/udev/rules.d/99-logitech-g502.rules`:

```
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c08d", MODE="0666"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c08b", MODE="0666"
```

Then reload: `sudo udevadm control --reload-rules && sudo udevadm trigger`

### Windows: Driver Setup

Windows requires a one-time Zadig/WinUSB driver installation for HID access.

**About Zadig:**
- Official website: [zadig.akeo.ie](https://zadig.akeo.ie/)
- Source code: [github.com/pbatard/zadig](https://github.com/pbatard/zadig)
- Open-source (GPLv3), widely trusted for USB driver installation
- Used by projects like [libusb](https://libusb.info/), [OpenOCD](https://openocd.org/), and many others

**Quick setup** (PowerShell as Administrator):

```powershell
.\scripts\install-winusb-driver.ps1
```

Or follow the manual steps in [TROUBLESHOOTING.md](TROUBLESHOOTING.md).

## Usage

### CLI

```bash
open-g-hub-cli list-devices         # Find connected mice
open-g-hub-cli set-dpi 1600         # Set DPI
open-g-hub-cli set-rate 1000        # Set polling rate (Hz)
open-g-hub-cli set-button 0 right   # Remap button 0 to right-click
open-g-hub-cli save-profile         # Save current settings
open-g-hub-cli load-profile         # Load saved settings
```

### GUI

Launch `open-g-hub-gui` for a graphical interface with:
- DPI slider (100-25,600)
- Polling rate dropdown
- Button mapping grid
- Device status polling (2s interval)
- Profile save/load

## Architecture

```
open-g-hub/
  crates/
    core/    # HID++ protocol, device discovery, safety validation
    gui/     # iced desktop app (Elm architecture)
    cli/     # clap command-line tool
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design.

## Development

```bash
cargo test              # Run all 75 tests
cargo clippy            # Lint
cargo fmt               # Format
RUST_LOG=trace cargo run -p open-g-hub-cli -- list-devices  # Debug logging
```

## License

GPL-2.0-or-later. Protocol knowledge from libratbag (MIT) and Solaar (GPLv2).
