# Security Policy

## Threat Model

Open G Hub writes directly to mouse hardware registers via HID++ 2.0 over USB HID. The primary risks are:

1. **Device bricking** — sending invalid commands could render the mouse unresponsive
2. **Unauthorized device modification** — malicious code could alter mouse behavior
3. **Privilege escalation** — the tool requires HID device access (udev rules or WinUSB driver)

## Safety Measures

### HID++ Feature Whitelist

Only known-safe HID++ feature IDs are allowed. Any request targeting an unlisted feature is **rejected before reaching the device**. The whitelist is defined in `crates/core/src/safety.rs`:

| Feature ID | Name | Access |
|-----------|------|--------|
| `0x0000` | ROOT | Read-only (feature lookup) |
| `0x0001` | FEATURE_SET | Read-only (feature enumeration) |
| `0x0005` | DEVICE_NAME | Read-only |
| `0x1000` | BATTERY_STATUS | Read-only |
| `0x1B04` | REPROG_CONTROLS_V4 | Read/Write (button remapping) |
| `0x2201` | ADJUSTABLE_DPI | Read/Write (DPI settings) |
| `0x8060` | REPORT_RATE | Read/Write (polling rate) |
| `0x8100` | ONBOARD_PROFILES | Read/Write (profile management) |

**Explicitly blocked** (not in whitelist):
- Firmware update / DFU features
- Raw memory read/write
- Manufacturing/debug features
- Any undocumented feature IDs

### Parameter Bounds Checking

All write parameters are validated against hardware-safe ranges before any HID communication:

- **DPI**: 100-25,600 (step 50)
- **Polling rate**: 125, 250, 500, or 1000 Hz only
- **Button index**: 0-5 only
- **Button actions**: CID-to-CID remapping only (no macro injection)

### What We Don't Do

- **No firmware operations**: Firmware read/write/update is completely out of scope
- **No macro support**: Onboard macro programming requires profile memory writes with higher bricking risk
- **No raw register access**: All communication goes through the typed feature API
- **No network access**: The application has no network capability
- **No telemetry**: Zero data collection or phone-home

### Firmware Checksum Validation

The G502 Lightspeed does not expose a firmware checksum validation feature via HID++ 2.0. The `DEVICE_FW_VERSION` feature (0x0003) provides firmware version strings but no integrity verification. This is a hardware limitation, not a software decision.

## Audit Logging

Enable structured trace logging to capture all HID++ transactions for post-hoc review:

```bash
RUST_LOG=trace open-g-hub-cli set-dpi 1600 2> audit.log
```

This logs:
- Every HID++ request (TX) with feature index, function ID, and parameter bytes
- Every HID++ response (RX) with full payload
- All safety validation checks (pass/fail)
- Error classification and retry decisions

For persistent audit logging, configure `tracing-subscriber` with a file appender (not yet built-in; can be added via `tracing-appender` crate).

## Reporting Vulnerabilities

If you discover a security issue (e.g., a way to bypass the safety layer, send unauthorized HID++ commands, or cause device damage), please:

1. **Do not open a public issue**
2. Email the maintainers directly (see repository contact info)
3. Include steps to reproduce and the HID++ commands involved
4. We will respond within 72 hours

## Bricking Recovery

If a mouse becomes unresponsive:

1. Unplug the USB cable / remove the receiver
2. Wait 10 seconds
3. Replug — most HID++ devices have hardware-level reset on power cycle
4. Factory reset: hold DPI button + left click while plugging in (model-specific, consult Logitech)
5. Contact Logitech support for hardware-level recovery
