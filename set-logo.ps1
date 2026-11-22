# Sets the Rewind logo (sidebar + window/exe icons) from a URL or a local image.
#   .\set-logo.ps1                      uses the default URL below
#   .\set-logo.ps1 .\my-logo.png        uses a local file
param([string]$Source = 'https://files.catbox.moe/zti00f.webp')
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$ext = [IO.Path]::GetExtension(($Source -split '\?')[0]); if (-not $ext) { $ext = '.webp' }
$dl = Join-Path $env:TEMP ('rewind-logo-src' + $ext)
if ($Source -match '^https?://') {
    Write-Host "Downloading logo..." -ForegroundColor Cyan
    Invoke-WebRequest $Source -OutFile $dl -UseBasicParsing
} else {
    Copy-Item $Source $dl -Force
}

# decode any image Windows understands (webp works on Win 11) -> PNG
Add-Type -AssemblyName PresentationCore, WindowsBase, System.Drawing
$tmpPng = Join-Path $env:TEMP 'rewind-logo-tmp.png'
$fs = [IO.File]::OpenRead($dl)
try {
    $dec = [Windows.Media.Imaging.BitmapDecoder]::Create($fs, 'PreservePixelFormat', 'OnLoad')
    $enc = New-Object Windows.Media.Imaging.PngBitmapEncoder
    $enc.Frames.Add([Windows.Media.Imaging.BitmapFrame]::Create($dec.Frames[0]))
    $out = [IO.File]::Create($tmpPng); $enc.Save($out); $out.Close()
} catch {
    throw "Windows could not decode this image. Convert it to PNG yourself and run: .\set-logo.ps1 .\logo.png"
} finally { $fs.Close() }

# pad to a transparent square (tauri icon needs square input)
$img = [System.Drawing.Image]::FromFile($tmpPng)
$side = [Math]::Max($img.Width, $img.Height)
$bmp = New-Object System.Drawing.Bitmap $side, $side, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.Clear([System.Drawing.Color]::Transparent)
$g.InterpolationMode = 'HighQualityBicubic'
$g.DrawImage($img, [int](($side - $img.Width) / 2), [int](($side - $img.Height) / 2), $img.Width, $img.Height)
$g.Dispose(); $img.Dispose()
$logo = Join-Path $PSScriptRoot 'rewind-gui\ui\logo.png'
$bmp.Save($logo, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
Write-Host "Sidebar logo written: rewind-gui\ui\logo.png" -ForegroundColor Green

# regenerate window / exe / installer icons
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust not installed yet - sidebar logo is set. Re-run this script after installing Rust to update the exe icon." -ForegroundColor Yellow
    exit 0
}
if (-not (Get-Command cargo-tauri -ErrorAction SilentlyContinue)) {
    Write-Host "Installing tauri-cli (one time)..." -ForegroundColor Cyan
    cargo install tauri-cli --version "^2" --locked
}
Push-Location rewind-gui
cargo tauri icon $logo -o icons
Pop-Location
Write-Host "Icons regenerated. Commit rewind-gui\ui\logo.png and rewind-gui\icons\ before releasing." -ForegroundColor Green
