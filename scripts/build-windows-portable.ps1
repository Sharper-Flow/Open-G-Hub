param(
    [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

Write-Host "Building Open G Hub Windows portable executable..."
rustup target add $Target
cargo build --release --target $Target -p open-g-hub-gui

$dist = Join-Path $PSScriptRoot "..\dist"
New-Item -ItemType Directory -Path $dist -Force | Out-Null

$src = Join-Path $PSScriptRoot "..\target\$Target\release\open-g-hub-gui.exe"
$dst = Join-Path $dist "Open-G-Hub-Windows-Portable.exe"
Copy-Item $src $dst -Force

Write-Host "Created: $dst"
Write-Host "Note: Run scripts\\install-winusb-driver.bat once as Administrator before first use."
