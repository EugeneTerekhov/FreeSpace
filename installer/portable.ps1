# Portable FreeSpace 1.3.0 build script.
#
# Usage:
#   pwsh -File installer/portable.ps1
#   pwsh -File installer/portable.ps1 -WebView2Runtime C:\WebView2\FixedRuntime
#
# Produces dist/freespace-portable-1.3.0.zip with the GUI and CLI built without
# the context-menu feature, the application-owned `data` directory, the user
# guide and, when `-WebView2Runtime` is given, an extracted fixed WebView2
# runtime in `webview2/`. No registry entries are created by this build.
#
# The fixed runtime is optional: without it the target machine must have the
# Evergreen WebView2 runtime, which every supported Windows 10/11 build ships.

param(
    [string]$WebView2Runtime = ''
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$stage = Join-Path $dist 'freespace-portable'
$zip = Join-Path $dist 'freespace-portable-1.3.0.zip'

# `--no-default-features` disables the optional `context-menu` feature. The
# portable build therefore has no `context-menu` command and no Explorer items.
cargo build --release --no-default-features -p freespace-cli -p freespace-desktop
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null

# The desktop package keeps the historical binary name, so shortcuts and
# scripts that point at `freespace-gui.exe` continue to work.
Copy-Item (Join-Path $root 'target\release\freespace-gui.exe') $stage
Copy-Item (Join-Path $root 'target\release\freespace.exe') $stage
Copy-Item (Join-Path $root 'docs\user-guide.md') $stage
Copy-Item (Join-Path $root 'CHANGELOG.md') $stage

if ($WebView2Runtime -ne '') {
    $source = (Resolve-Path -LiteralPath $WebView2Runtime).Path
    if (-not (Test-Path (Join-Path $source 'msedgewebview2.exe'))) {
        throw "WebView2 runtime not found in $source (msedgewebview2.exe is missing)"
    }
    Copy-Item $source (Join-Path $stage 'webview2') -Recurse
} else {
    Write-Warning 'No fixed WebView2 runtime bundled; the machine needs the Evergreen runtime.'
}

$data = Join-Path $stage 'data'
New-Item -ItemType Directory -Path $data | Out-Null
@'
FreeSpace stores settings.toml, freespace.db, the journal, the hash cache, the
quarantine and its WebView2 profile (data\webview2) in this folder. Delete the
application folder to remove every trace from the machine.

If the `webview2` folder next to the executables is missing, install the
Microsoft Edge WebView2 Evergreen runtime once; the application never downloads
anything itself.
'@ | Set-Content -Path (Join-Path $data 'README.txt') -Encoding UTF8

if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

Write-Host "Portable archive written to $zip"

