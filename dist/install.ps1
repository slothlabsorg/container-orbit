# orbit installer — Windows (PowerShell).
#
#   irm https://raw.githubusercontent.com/slothlabsorg/container-orbit/main/dist/install.ps1 | iex
#
# Downloads the latest release binary and installs orbit.exe to
# %LOCALAPPDATA%\orbit\bin, adding it to your user PATH.

$ErrorActionPreference = "Stop"
$Repo = "slothlabsorg/container-orbit"
$Version = if ($env:ORBIT_VERSION) { $env:ORBIT_VERSION } else { "latest" }

$arch = (Get-CimInstance Win32_Processor).Architecture
# 9 = x64, 12 = ARM64
$target = if ($arch -eq 12) { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
$asset  = "orbit-$target.zip"

if ($Version -eq "latest") {
  $url = "https://github.com/$Repo/releases/latest/download/$asset"
} else {
  $url = "https://github.com/$Repo/releases/download/$Version/$asset"
}

$dir = Join-Path $env:LOCALAPPDATA "orbit\bin"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$tmp = Join-Path $env:TEMP $asset

Write-Host "> Downloading $asset..." -ForegroundColor Cyan
Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing
Expand-Archive -Path $tmp -DestinationPath $dir -Force
Remove-Item $tmp -Force

# Add to the user PATH if not already there.
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$dir*") {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$dir", "User")
  Write-Host "! Added $dir to your PATH — open a new terminal to pick it up." -ForegroundColor Yellow
}

Write-Host "OK installed orbit to $dir\orbit.exe" -ForegroundColor Green
Write-Host ""
Write-Host "Next: run  orbit setup" -ForegroundColor Green
