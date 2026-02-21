# Release & Packaging Strategy

## Distribution Channels

### 1. GitHub Releases (Primary)

Pre-built binaries for each platform, attached to GitHub Release tags.

| Platform | Artifact | Notes |
|----------|----------|-------|
| Linux x86_64 | `open-g-hub-cli-linux-x86_64`, `open-g-hub-gui-linux-x86_64` | Statically linked where possible |
| Windows x86_64 | `open-g-hub-cli-windows-x86_64.exe`, `open-g-hub-gui-windows-x86_64.exe` | MSVC build |

**CI handles this**: The GitHub Actions workflow already builds release binaries for both platforms and uploads them as artifacts. To create a release:

```bash
git tag v0.1.0
git push origin v0.1.0
# CI builds and uploads artifacts
# Create GitHub Release from the tag, attach artifacts
```

### 2. Cargo Install (Rust Users)

```bash
cargo install open-g-hub-cli
cargo install open-g-hub-gui
```

Requires publishing to crates.io. Deferred until API stabilizes.

### 3. System Packages (Future)

| Format | Tool | Target |
|--------|------|--------|
| `.deb` (Debian/Ubuntu) | `cargo-deb` | Includes udev rules at `/etc/udev/rules.d/` |
| `.rpm` (Fedora/RHEL) | `cargo-generate-rpm` | Includes udev rules |
| MSI (Windows) | `wix` / `cargo-wix` | Includes Zadig in bundle, optional driver install |
| Flatpak | `flatpak-builder` | Sandboxed, requires HID device portal access |
| AUR (Arch Linux) | PKGBUILD | Community-maintained |

System packages are deferred to post-1.0. The standalone binary approach covers initial release.

## Install Paths

| Platform | CLI Binary | GUI Binary | Config Dir | udev Rules |
|----------|-----------|-----------|-----------|-----------|
| Linux | `~/.cargo/bin/` or `/usr/local/bin/` | `~/.cargo/bin/` or `/usr/local/bin/` | N/A (G Hub compatibility target is Windows) | `/etc/udev/rules.d/99-logitech-g502.rules` |
| Windows | `%USERPROFILE%\.cargo\bin\` or any PATH dir | Same | `%LOCALAPPDATA%\LGHUB\settings.db` | N/A (WinUSB driver via Zadig) |

## Windows Zadig Bundle

For Windows releases, we include:
- `open-g-hub-cli.exe` and `open-g-hub-gui.exe`
- `scripts/install-winusb-driver.ps1` — PowerShell helper that downloads and launches Zadig
- `README.txt` — quick start instructions

Future MSI installer could run the Zadig driver setup as part of installation.

## Versioning

- **SemVer**: `MAJOR.MINOR.PATCH`
- **Pre-1.0**: Breaking changes expected. HID++ protocol interface may change.
- **1.0 criteria**: Stable CLI/GUI interface, tested on real hardware, Windows + Linux confirmed working

## Release Checklist

1. All tests pass: `cargo test`
2. No clippy warnings: `cargo clippy -- -D warnings`
3. Format check: `cargo fmt --check`
4. Version bumped in all `Cargo.toml` files
5. CHANGELOG updated (when created)
6. Tag created: `git tag vX.Y.Z`
7. CI builds complete for both platforms
8. GitHub Release created with binaries attached
9. README install instructions verified
