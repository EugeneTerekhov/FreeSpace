# Portable FreeSpace 1.0 build script.
#
# Usage:
#   pwsh -File installer/portable.ps1
#
# Produces dist/freespace-portable-1.1.0.zip with both executables built without
# the context-menu feature, the application-owned `data` directory and the user
# guide. No registry entries are created by this build.

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist 'freespace-portable'
$zip = Join-Path $dist 'freespace-portable-1.1.0.zip'

# `--no-default-features` disables the optional `context-menu` feature. The
# portable build therefore has no `context-menu` command and no Explorer items.
cargo build --release --no-default-features -p freespace-cli -p freespace-gui
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null

Copy-Item (Join-Path $root 'target\release\freespace-gui.exe') $stage
Copy-Item (Join-Path $root 'target\release\freespace.exe') $stage
Copy-Item (Join-Path $root 'docs\user-guide.md') $stage
Copy-Item (Join-Path $root 'CHANGELOG.md') $stage

$data = Join-Path $stage 'data'
New-Item -ItemType Directory -Path $data | Out-Null
@'
FreeSpace stores settings.toml, freespace.db, the journal, the hash cache and
the quarantine in this folder. Delete the application folder to remove every
trace from the machine.
'@ | Set-Content -Path (Join-Path $data 'README.txt') -Encoding UTF8

if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

Write-Host "Portable archive written to $zip"
