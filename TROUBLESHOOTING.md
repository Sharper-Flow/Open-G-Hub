# Troubleshooting

## Windows: "Access Denied" or Device Not Found

### Problem

Windows `hidclass.sys` enforces exclusive access on HID mouse devices. The standard HID driver locks the device, preventing user-space tools (including Open G Hub) from sending HID++ commands. You'll see errors like:

- `Access denied`
- `Device not found` (device is present but not accessible)
- `PermissionDenied` error class in logs

### Solution: Zadig / WinUSB Driver (One-Time Setup)

This replaces the Windows HID driver with WinUSB for the Logitech receiver interface, allowing user-space HID access. **This requires administrator privileges once.** After setup, Open G Hub runs without admin.

**About Zadig:**
- Official website: [zadig.akeo.ie](https://zadig.akeo.ie/)
- Source code: [github.com/pbatard/zadig](https://github.com/pbatard/zadig) (open-source, GPLv3)
- Widely trusted by the open-source community for USB driver installation
- Used by projects like [libusb](https://libusb.info/), [OpenOCD](https://openocd.org/), and many embedded systems tools

#### Step-by-Step

1. **Download Zadig** from [zadig.akeo.ie](https://zadig.akeo.ie/)

2. **Plug in your G502 Lightspeed** (wired or wireless receiver)

3. **Run Zadig as Administrator**

4. **Menu**: Options > List All Devices

5. **Select the correct device** from the dropdown:
   - Look for `Logitech G502 Lightspeed` or the USB receiver
   - **VID**: `046D` — verify this matches
   - **PID**: `C08D` (Lightspeed) or `C08B` (HERO)
   - **WARNING**: Do NOT select the mouse HID interface itself (the one Windows uses for cursor movement). Select the HID++ communication interface, which is typically a secondary interface on the receiver.

6. **Set driver to WinUSB** (should be the default suggestion)

7. **Click "Replace Driver"** and wait for completion

8. **Restart Open G Hub** — the device should now be accessible

#### Reverting the Driver

If you want to restore the original Windows HID driver:

1. Open **Device Manager**
2. Find the Logitech device under "Universal Serial Bus devices"
3. Right-click > **Update driver** > **Search automatically**
4. Windows will reinstall the default HID driver

### Alternative: Run as Administrator

Running Open G Hub with admin privileges may bypass exclusive access on some systems, but this is **not recommended** for regular use. Zadig/WinUSB is the proper solution.

## Linux: Permission Denied

### Problem

Linux requires either root access or udev rules to access HID devices from user space.

### Solution: udev Rules

Create `/etc/udev/rules.d/99-logitech-g502.rules`:

```
# Logitech G502 Lightspeed
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c08d", MODE="0666"
# Logitech G502 HERO
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c08b", MODE="0666"
```

Reload udev rules:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

You may need to unplug and replug the mouse after reloading rules.

### Alternative: plugdev Group

Some distributions use a `plugdev` group for device access:

```
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c08d", MODE="0660", GROUP="plugdev"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="046d", ATTRS{idProduct}=="c08b", MODE="0660", GROUP="plugdev"
```

Ensure your user is in the `plugdev` group: `sudo usermod -aG plugdev $USER` (then log out/in).

## Device Not Found

### Symptoms

- `list-devices` returns empty
- GUI shows "No device connected"

### Checklist

1. **Is the mouse plugged in?** Check USB connection (wired) or receiver (wireless)
2. **Correct PID?** Run `lsusb | grep 046d` (Linux) or check Device Manager (Windows) to verify the product ID matches a supported device
3. **Driver conflict?** On Linux, check if `libratbag` / `ratbagd` is running — it may hold exclusive access. Stop it: `sudo systemctl stop ratbagd`
4. **HID interface?** The G502 Lightspeed exposes multiple HID interfaces. Open G Hub needs the HID++ interface, not the standard mouse interface
5. **Try debug logging**: `RUST_LOG=trace open-g-hub-cli list-devices` to see all enumerated HID devices

## Communication Errors

### Timeout

The device didn't respond within the expected time.

- **Cause**: USB bus congestion, device in sleep mode, or wrong interface
- **Fix**: Try again. If persistent, unplug/replug the mouse. Check `RUST_LOG=trace` output for the specific request that timed out

### Protocol Error

The device returned an HID++ error code.

- **Common codes**:
  - `0x01` — Unknown function (feature not supported on this device)
  - `0x02` — Wrong function (wrong function ID for this feature)
  - `0x05` — Invalid argument
  - `0x09` — Busy (device is processing another command)
- **Fix**: Busy errors are retried automatically (up to 3 times). Other errors indicate a protocol mismatch — please file a bug report with the full trace log

### Invalid Response

The response bytes don't match the expected format.

- **Cause**: Response from wrong device index, corrupted USB data, or unsupported device variant
- **Fix**: Enable trace logging and file a bug report with the raw TX/RX bytes

## Build Issues

### Linux: Missing `libudev-dev`

Open G Hub uses `hidapi` with the `linux-native-basic-udev` feature, which should NOT require `libudev-dev`. If you see linker errors mentioning `libudev`:

1. Ensure you're using the workspace `Cargo.toml` (not building individual crates)
2. Check that `hidapi` features are set correctly: `features = ["linux-native-basic-udev"]`
3. As a fallback, install `libudev-dev`: `sudo apt install libudev-dev`

### Windows: MSVC Toolchain

Ensure you have the MSVC build tools installed (comes with Visual Studio Build Tools or full Visual Studio). The `x86_64-pc-windows-msvc` target is required.

## Debug Logging

Set `RUST_LOG` to control log verbosity:

```bash
RUST_LOG=warn open-g-hub-cli list-devices     # Warnings only
RUST_LOG=info open-g-hub-cli list-devices     # Info + warnings
RUST_LOG=debug open-g-hub-cli list-devices    # Debug output
RUST_LOG=trace open-g-hub-cli list-devices    # Full HID TX/RX traces
```

For the GUI, set the env var before launching:

```bash
RUST_LOG=debug open-g-hub-gui
```

## Bricking Risk

**Open G Hub includes safety bounds on all device writes.** DPI, polling rate, and button index values are validated against known-safe ranges before any HID communication occurs.

However, as with any tool that writes to hardware:

- **Do not modify this software to bypass safety checks**
- **Macro/profile memory writes are not yet supported** — they carry higher risk and are deferred
- **Firmware updates are out of scope** — never attempt firmware operations
- **If the mouse becomes unresponsive**: unplug it, wait 10 seconds, replug. Factory reset: hold DPI button + left click while plugging in (consult Logitech support for your specific model)
