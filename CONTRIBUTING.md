# Contributing to Open G Hub

## Development Setup

### Prerequisites

- **Rust toolchain** (stable, 1.75+): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Linux**: `libudev-dev` is NOT required (we use `hidapi` with `linux-native-basic-udev` feature)
- **Windows**: No special build deps. Runtime requires Zadig/WinUSB driver (see [TROUBLESHOOTING.md](TROUBLESHOOTING.md))

### Build & Test

```bash
git clone https://github.com/user/open-g-hub.git
cd open-g-hub

cargo build               # Debug build (all crates)
cargo test                # Run all 75+ tests
cargo clippy              # Lint (must pass with 0 warnings)
cargo fmt --check         # Format check
```

### Project Structure

```
open-g-hub/
  crates/
    core/    # Library: HID++ protocol, device discovery, safety validation
    gui/     # iced desktop application
    cli/     # clap command-line tool
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed crate design.

## HID++ 2.0 Protocol Primer

Open G Hub communicates with Logitech mice using the **HID++ 2.0** protocol over USB HID. Key concepts:

### Report Types

- **Short report** (7 bytes, ID `0x10`): header + 3 parameter bytes. Used for simple get/set commands.
- **Long report** (20 bytes, ID `0x11`): header + 16 parameter bytes. Used for bulk data (profiles, multi-field responses).

### Feature-Based Architecture

HID++ 2.0 uses a feature registry. Each capability has a **feature ID** (e.g., `0x2201` for DPI). At runtime, you query the **ROOT** feature (`0x0000`) to discover the **feature index** assigned to each feature ID on the connected device.

| Feature ID | Name | Purpose |
|-----------|------|---------|
| `0x0000` | ROOT | Feature index lookup |
| `0x0001` | FEATURE_SET | List all features |
| `0x2201` | ADJUSTABLE_DPI | DPI read/write |
| `0x8060` | REPORT_RATE | Polling rate read/write |
| `0x1B04` | REPROG_CONTROLS_V4 | Button remapping |
| `0x8100` | ONBOARD_PROFILES | Profile management |

### Request/Response Flow

1. Encode a `HidppRequest` with device index, feature index, function ID, and parameters
2. Send as a raw HID report via the `HidTransport` trait
3. Receive response bytes, decode into `HidppResponse`
4. Extract data from the response parameters

### Key Source Files

- `crates/core/src/hidpp.rs` — packet encoding/decoding
- `crates/core/src/transport.rs` — `HidTransport` trait and mock implementation
- `crates/core/src/dpi.rs`, `report_rate.rs`, `buttons.rs`, `onboard.rs` — feature implementations

## Testing

All protocol logic is tested via **mock transport** (`MockTransport` in `transport.rs`). This maps expected request bytes to canned response bytes, enabling full TDD without physical hardware.

### Running Tests

```bash
cargo test                                    # All tests
cargo test -p open-g-hub-core                 # Core crate only
cargo test -p open-g-hub-core -- dpi          # DPI tests only
cargo test -p open-g-hub-core -- integration  # Integration tests
RUST_LOG=trace cargo test                     # With debug output
```

### Writing Tests

1. Create a `MockTransport` instance
2. Register expected request/response pairs with `on_short_request` or `on_long_request`
3. Always register feature index lookups (ROOT feature) first
4. Call the function under test and assert on results

Example pattern:

```rust
#[test]
fn test_my_feature() {
    let mock = MockTransport::new();
    // Register feature lookup: feature 0x2201 -> index 0x05
    mock.on_short_request(0x00, 0x00, &[0x22, 0x01, 0x00], &[0x05, 0x00, 0x00]);
    // Register the actual command
    mock.on_short_request(0x05, 0x00, &[0x00, 0x00, 0x00], &[0x06, 0x40, 0x00]);

    let result = read_dpi(&mock, 0xFF).unwrap();
    assert_eq!(result, 1600);
}
```

## Code Style

- **Formatting**: `cargo fmt` (rustfmt defaults)
- **Linting**: `cargo clippy` must pass with zero warnings
- **Line length**: 100 chars (soft limit)
- **Naming**: `snake_case` for functions/variables, `PascalCase` for types
- **Errors**: Use `thiserror` derive macros. Return `Result<T>` (aliased to `crate::error::Result<T>`)
- **Logging**: Use `tracing` macros (`trace!`, `debug!`, `warn!`). Log all HID TX/RX at trace level
- **Safety**: All device writes MUST go through the safety module (`safety.rs`) before reaching HID

## Pull Request Guidelines

1. Fork and create a feature branch
2. Add tests for new functionality (mock transport for protocol work)
3. Ensure `cargo test && cargo clippy && cargo fmt --check` all pass
4. Keep commits atomic and well-described
5. Reference any relevant HID++ protocol documentation in commit messages

## Protocol References

- [libratbag](https://github.com/libratbag/libratbag) (MIT) — device profiles and protocol docs
- [Solaar](https://github.com/pwr-Solaar/Solaar) (GPLv2) — protocol implementation reference
- HID++ 2.0 specification (Logitech, not publicly released — reverse-engineered by community)
