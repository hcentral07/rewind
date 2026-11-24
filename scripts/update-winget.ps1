<#
Fills the winget manifests for a released version.
  .\scripts\update-winget.ps1 -Version 0.5.0
Downloads the installer from the GitHub release, computes its SHA-256 and writes
winget-out\manifests\h\hcentral07\Rewind\<version>\ ready for `winget validate` and a PR to microsoft/winget-pkgs.
#>
param(
  [Parameter(Mandatory = $true)][string]$Version,
  [string]$Repo = "hcentral07/rewind"
)
$ErrorActionPreference = "Stop"
$Version = $Version.TrimStart("v")
$name = "Rewind_${Version}_x64-setup.exe"
$url = "https://github.com/$Repo/releases/download/v$Version/$name"

$tmp = Join-Path $env:TEMP $name
Write-Host "Downloading $url"
Invoke-WebRequest -Uri $url -OutFile $tmp
$hash = (Get-FileHash $tmp -Algorithm SHA256).Hash.ToUpper()
Write-Host "SHA-256: $hash"

$out = Join-Path "winget-out" "manifests\h\hcentral07\Rewind\$Version"
New-Item -ItemType Directory -Force $out | Out-Null
$date = (Get-Date).ToString("yyyy-MM-dd")

foreach ($f in Get-ChildItem "packaging\winget\*.yaml") {
  $t = Get-Content $f.FullName -Raw
  $t = $t -replace "PackageVersion: .*", "PackageVersion: $Version"
  $t = $t -replace "download/v[0-9][0-9.]*/Rewind_[0-9][0-9.]*_x64-setup\.exe", "download/v$Version/$name"
  $t = $t -replace "tag/v[0-9][0-9.]*", "tag/v$Version"
  $t = $t -replace "InstallerSha256: [0-9A-Fa-f]{64}", "InstallerSha256: $hash"
  $t = $t -replace "ReleaseDate: .*", "ReleaseDate: $date"
  Set-Content -Path (Join-Path $out $f.Name) -Value $t -NoNewline -Encoding utf8
}
Write-Host "Wrote $out"
Write-Host "Next: winget validate --manifest $out"
Write-Host "      winget install --manifest $out   (needs: winget settings --enable LocalManifestFiles)"
Write-Host "      then open a PR to microsoft/winget-pkgs, or run: wingetcreate submit $out"
