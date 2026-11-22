# Builds the Rewind setup .exe locally. Needs Rust + MSVC build tools installed.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust first: https://rustup.rs (and the MSVC C++ build tools)."
}
if (-not (Get-Command cargo-tauri -ErrorAction SilentlyContinue)) {
    Write-Host "Installing tauri-cli (one time, takes a few minutes)..." -ForegroundColor Cyan
    cargo install tauri-cli --version "^2" --locked
}

Write-Host "Building installer..." -ForegroundColor Cyan
Push-Location rewind-gui
cargo tauri build
Pop-Location

Write-Host "Building CLI..." -ForegroundColor Cyan
cargo build --release -p rewind-cli

New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item "target\release\bundle\nsis\*-setup.exe" dist\ -Force
Copy-Item "target\release\rewind.exe" "dist\rewind-cli-windows-x64.exe" -Force

Write-Host ""
Write-Host "Done. Files in .\dist :" -ForegroundColor Green
Get-ChildItem dist | ForEach-Object { Write-Host ("  " + $_.Name + "  (" + [math]::Round($_.Length / 1MB, 1) + " MB)") }
Start-Process explorer.exe (Join-Path $PSScriptRoot 'dist')
