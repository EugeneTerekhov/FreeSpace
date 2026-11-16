# Portable FreeSpace 1.0 build script.
#
# Usage:
#   pwsh -File installer/portable.ps1
#
# Produces dist/freespace-portable-1.0.0.zip with both executables, the marker
# file that switches storage to the application folder, and the user guide.

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist 'freespace-portable'
$zip = Join-Path $dist 'freespace-portable-1.0.0.zip'

cargo build --workspace --release
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null

Copy-Item (Join-Path $root 'target\release\freespace-gui.exe') $stage
Copy-Item (Join-Path $root 'target\release\freespace.exe') $stage
Copy-Item (Join-Path $root 'docs\user-guide.md') $stage
Copy-Item (Join-Path $root 'CHANGELOG.md') $stage
New-Item -ItemType File -Path (Join-Path $stage 'portable.flag') | Out-Null

if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

Write-Host "Portable archive written to $zip"
