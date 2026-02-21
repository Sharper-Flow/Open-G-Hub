#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Installs the WinUSB driver for Logitech G502 Lightspeed/HERO mice.

.DESCRIPTION
    Downloads Zadig's CLI tool (wdi-simple) and replaces the Windows HID
    driver with WinUSB for the Logitech G502 HID++ communication interface.
    This is a one-time setup that allows Open G Hub to communicate with the
    mouse without requiring administrator privileges at runtime.

.NOTES
    - Requires administrator privileges (will prompt for UAC if not elevated)
    - Only affects the HID++ interface, not the standard mouse input
    - Reversible via Device Manager (see TROUBLESHOOTING.md)
    - VID: 0x046D (Logitech), PIDs: 0xC08D (Lightspeed), 0xC08B (HERO)

.LINK
    https://zadig.akeo.ie/
    https://github.com/pbatard/libwdi
#>

param(
    [switch]$DryRun,
    [ValidateSet('C08D', 'C08B')]
    [string]$PID = 'C08D'
)

$ErrorActionPreference = 'Stop'

$VID = '046D'
$DeviceNames = @{
    'C08D' = 'Logitech G502 Lightspeed'
    'C08B' = 'Logitech G502 HERO'
}
$DeviceName = $DeviceNames[$PID]

Write-Host "Open G Hub - WinUSB Driver Installer" -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Device: $DeviceName (VID: 0x$VID, PID: 0x$PID)"
Write-Host ""
Write-Host "About Zadig:" -ForegroundColor White
Write-Host "  - Official site: https://zadig.akeo.ie/" -ForegroundColor Gray
Write-Host "  - Source code:   https://github.com/pbatard/zadig (GPLv3)" -ForegroundColor Gray
Write-Host "  - Widely trusted by open-source projects (libusb, OpenOCD, etc.)" -ForegroundColor Gray
Write-Host ""

# Check if running as admin
$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "ERROR: This script requires administrator privileges." -ForegroundColor Red
    Write-Host "Right-click PowerShell and select 'Run as Administrator', then try again."
    exit 1
}

# Warn user
Write-Host "WARNING: This will replace the Windows HID driver for your G502's" -ForegroundColor Yellow
Write-Host "         HID++ communication interface with WinUSB." -ForegroundColor Yellow
Write-Host ""
Write-Host "         Your mouse cursor will continue to work normally." -ForegroundColor Green
Write-Host "         This change can be reverted via Device Manager." -ForegroundColor Green
Write-Host ""

if (-not $DryRun) {
    $confirm = Read-Host "Continue? (y/N)"
    if ($confirm -ne 'y' -and $confirm -ne 'Y') {
        Write-Host "Cancelled."
        exit 0
    }
}

# Check if Zadig/wdi-simple is available
$zadigUrl = "https://github.com/pbatard/libwdi/releases/download/v1.5.0/zadig-2.8.exe"
$zadigPath = "$env:TEMP\zadig.exe"

if (-not (Test-Path $zadigPath)) {
    Write-Host "Downloading Zadig..." -ForegroundColor Cyan
    if ($DryRun) {
        Write-Host "[DRY RUN] Would download from $zadigUrl"
    } else {
        try {
            Invoke-WebRequest -Uri $zadigUrl -OutFile $zadigPath -UseBasicParsing
            Write-Host "Downloaded to $zadigPath"
        } catch {
            Write-Host "ERROR: Failed to download Zadig." -ForegroundColor Red
            Write-Host "Please download manually from https://zadig.akeo.ie/ and run it."
            Write-Host ""
            Write-Host "Manual steps:" -ForegroundColor Yellow
            Write-Host "  1. Open Zadig as Administrator"
            Write-Host "  2. Options > List All Devices"
            Write-Host "  3. Select '$DeviceName' (VID: $VID, PID: $PID)"
            Write-Host "  4. Set driver to WinUSB"
            Write-Host "  5. Click 'Replace Driver'"
            exit 1
        }
    }
}

Write-Host ""
Write-Host "Launching Zadig..." -ForegroundColor Cyan
Write-Host ""
Write-Host "In Zadig:" -ForegroundColor Yellow
Write-Host "  1. Go to Options > List All Devices" -ForegroundColor White
Write-Host "  2. Select '$DeviceName' from the dropdown" -ForegroundColor White
Write-Host "  3. Verify VID: $VID and PID: $PID" -ForegroundColor White
Write-Host "  4. Ensure target driver is 'WinUSB'" -ForegroundColor White
Write-Host "  5. Click 'Replace Driver'" -ForegroundColor White
Write-Host ""

if ($DryRun) {
    Write-Host "[DRY RUN] Would launch $zadigPath"
} else {
    Start-Process -FilePath $zadigPath -Wait
}

Write-Host ""
Write-Host "Done! If you replaced the driver successfully, Open G Hub should" -ForegroundColor Green
Write-Host "now be able to communicate with your $DeviceName." -ForegroundColor Green
Write-Host ""
Write-Host "To revert: Device Manager > find Logitech device > Update driver > Search automatically" -ForegroundColor Gray
