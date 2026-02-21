# Architecture

## Overview

Open G Hub is a Cargo workspace with three crates:

```
open-g-hub/
  crates/
    core/   # Library — all protocol and device logic
    gui/    # Binary — iced desktop application
    cli/    # Binary — clap command-line tool
```

Both `gui` and `cli` depend on `core`. They contain no protocol logic — only UI/CLI glue.

## Core Crate (`open-g-hub-core`)

### Module Map

```
core/src/
  lib.rs              # Module declarations, VID/PID constants
  error.rs            # Error enum (thiserror)
  hidpp.rs            # HID++ 2.0 packet encode/decode
  transport.rs        # HidTransport trait + MockTransport
  device.rs           # Device discovery, MouseModel, DeviceInfo
  safety.rs           # Write parameter validation (bounds checking)
  dpi.rs              # DPI read/write (feature 0x2201)
  report_rate.rs      # Polling rate read/write (feature 0x8060)
  buttons.rs          # Button remapping (feature 0x1B04)
  onboard.rs          # Onboard profile management (feature 0x8100)
  comm.rs             # Error classification + retry logic
  profile.rs          # Logitech G Hub-compatible profile storage layer
  integration_tests.rs # Full-flow mock tests
```

### Layered Design

```
┌─────────────────────────────────────────────┐
│         Application (GUI / CLI)             │
├─────────────────────────────────────────────┤
│  Feature Modules                            │
│  dpi.rs  report_rate.rs  buttons.rs  etc.   │
├─────────────────────────────────────────────┤
│  Safety Layer (safety.rs)                   │
│  Validates all writes before device I/O     │
├─────────────────────────────────────────────┤
│  Communication (comm.rs)                    │
│  Error classification, retry logic          │
├─────────────────────────────────────────────┤
│  Transport (transport.rs)                   │
│  HidTransport trait — real HID or mock      │
├─────────────────────────────────────────────┤
│  Protocol (hidpp.rs)                        │
│  HID++ 2.0 packet encoding/decoding        │
├─────────────────────────────────────────────┤
│  Device Discovery (device.rs)               │
│  Enumerate HID, match VID/PID              │
└─────────────────────────────────────────────┘
```

### Key Abstractions

#### `HidTransport` Trait

```rust
pub trait HidTransport: Send {
    fn send_report(&self, data: &[u8]) -> Result<Vec<u8>>;
}
```

All protocol code accepts `&dyn HidTransport`, never a concrete HID handle. This enables:
- **Testing**: `MockTransport` maps request bytes to canned responses
- **Portability**: Real implementation wraps `hidapi::HidDevice`
- **Extensibility**: Future transports (Bluetooth, network) can be added without touching protocol code

#### Feature Index Lookup

HID++ 2.0 assigns each feature a runtime index. Before using any feature, you must query ROOT:

```
lookup_feature_index(transport, device_index, 0x2201)  // -> Ok(5)
```

This index (5) is then used as the `feature_index` in all subsequent requests for that feature.

#### Safety Validation

Every write function calls the safety module before sending data:

```
safety::validate_dpi(dpi_value)?;       // Bounds check [100, 25600], step 50
safety::validate_rate(rate)?;           // Must be 125/250/500/1000
safety::validate_button_index(idx)?;    // Must be 0-5
```

Validation happens client-side, before any HID communication. Invalid parameters are rejected with descriptive errors.

### Error Handling

```
Error (error.rs)         — Enum: Hid, DeviceNotFound, HidppProtocol, OutOfRange, etc.
  │
  ├─ ErrorClass (comm.rs) — Classification: Transient, Disconnected, PermissionDenied, Protocol
  │
  └─ send_with_retry()   — Retries transient errors up to 3 times
```

Error classification drives retry behavior:
- **Transient** (timeout, busy): retry up to 3 times
- **Disconnected**: stop, notify user
- **PermissionDenied**: stop, suggest Zadig/udev fix
- **Protocol/InvalidResponse**: stop, log details

### Profile Persistence

Profiles are targeted to Logitech G Hub storage on Windows:
- Windows: `%LOCALAPPDATA%\LGHUB\settings.db`

A profile captures DPI, polling rate, and button mappings.

## GUI Crate (`open-g-hub-gui`)

Built with [iced](https://github.com/iced-rs/iced) using the Elm architecture:

- **Model**: `OpenGHub` struct holds device info, DPI value, polling rate, button mappings
- **Messages**: `Message` enum (DeviceFound, DpiChanged, RateChanged, ButtonChanged, Apply, Save, etc.)
- **Update**: Pattern-matches on messages to update state
- **View**: Pure function rendering the current state as UI elements
- **Subscription**: 2-second interval polls for device connection/disconnection

The GUI never touches HID directly — all device communication goes through core functions.

## CLI Crate (`open-g-hub-cli`)

Built with [clap](https://github.com/clap-rs/clap) derive macros. Nine subcommands:

| Command | Core Function |
|---------|---------------|
| `list-devices` | `device::discover_devices()` |
| `get-dpi` | `dpi::read_dpi()` |
| `set-dpi <value>` | `safety::validate_dpi()` + `dpi::write_dpi()` |
| `get-rate` | `report_rate::read_rate()` |
| `set-rate <hz>` | `safety::validate_rate()` + `report_rate::write_rate()` |
| `get-buttons` | `buttons::read_button_mapping()` |
| `set-button <idx> <action>` | `safety::validate_button_index()` + `buttons::write_button_mapping()` |
| `save-profile` | `profile::save_profile()` |
| `load-profile` | `profile::load_profile()` |

## Data Flow: Setting DPI

```
User: "set-dpi 1600"
  │
  ├─ CLI parses argument (clap)
  │
  ├─ safety::validate_dpi(1600)
  │     └─ Ok(1600) — within [100, 25600], divisible by 50
  │
  ├─ device::discover_devices()
  │     └─ Enumerates HID, finds G502 at device_index 0xFF
  │
  ├─ transport: open HID device handle
  │
  ├─ dpi::write_dpi(&transport, 0xFF, 1600)
  │     ├─ lookup_feature_index(transport, 0xFF, 0x2201) -> index 5
  │     ├─ Encode HidppRequest { feature_index: 5, function: SET_DPI, params: [0x06, 0x40, ...] }
  │     ├─ transport.send_report(encoded_bytes)
  │     └─ Decode response, verify success
  │
  └─ Print "DPI set to 1600"
```

## Testing Strategy

- **Unit tests**: Each module has `#[cfg(test)]` tests using `MockTransport`
- **Integration tests**: `integration_tests.rs` runs full workflows (discover -> configure -> verify)
- **Concurrency tests**: Verify thread-safety of `MockTransport` under concurrent access
- **No hardware required**: All tests run against mock transport with pre-recorded HID++ packets
- **CI**: GitHub Actions runs tests on both `ubuntu-latest` and `windows-latest`
