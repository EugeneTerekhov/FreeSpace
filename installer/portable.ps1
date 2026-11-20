<#
.SYNOPSIS
Builds the portable Windows archive of FreeSpace.

.DESCRIPTION
Produces `dist/freespace-portable-<version>-windows-x64.zip` with the GUI
(`freespace-gui.exe`), the CLI (`freespace.exe`), the documentation, the
`CHANGELOG.md` and an application-owned `data` directory.

The version is read from `[workspace.package].version` in the root Cargo.toml;
the script contains no version literal. The frontend under
`apps/freespace-desktop/ui` is built when `ui/dist` is missing, because the
Tauri release build embeds it into the executable.

The build uses `--no-default-features` on purpose: the portable package ships
without the optional `context-menu` feature, so it never registers Explorer
entries, and it never writes to the registry. Deleting the unpacked folder
removes every trace.

.PARAMETER WebView2Runtime
Optional path to an extracted fixed WebView2 runtime. Its contents are copied
into `webview2/` next to the executables, which the application picks up
automatically. Without it the machine needs the Evergreen WebView2 runtime.

.PARAMETER SkipBuild
Reuse the binaries already present in `target/release` instead of running
cargo.

.PARAMETER VersionOnly
Print the workspace version and exit. Used by validation and by the release
workflow to prove the version parsing without starting a build.

.EXAMPLE
pwsh -File installer/portable.ps1

.EXAMPLE
pwsh -File installer/portable.ps1 -WebView2Runtime C:\WebView2\FixedRuntime
#>
[CmdletBinding()]
param(
    [string]$WebView2Runtime = '',

    [switch]$SkipBuild,

    [switch]$VersionOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$ui = Join-Path $root 'apps\freespace-desktop\ui'

# Reads `[workspace.package].version` from Cargo.toml. The table is located
# from its header so a dependency version can never be picked up by mistake.
function Get-WorkspaceVersion {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CargoTomlPath
    )

    $inWorkspacePackage = $false
    foreach ($line in Get-Content -LiteralPath $CargoTomlPath) {
        if ($line -match '^\s*\[(?<section>[^\]]+)\]') {
            $inWorkspacePackage = $Matches['section'] -eq 'workspace.package'
            continue
        }
        if ($inWorkspacePackage -and $line -match '^\s*version\s*=\s*"(?<version>[^"]+)"') {
            return $Matches['version']
        }
    }
    throw "Unable to read [workspace.package].version from $CargoTomlPath"
}

function Invoke-External {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)]
        [string]$WorkingDirectory
    )

    Push-Location -LiteralPath $WorkingDirectory
    try {
        Write-Host "> $FilePath $($Arguments -join ' ')"
        & $FilePath @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "$FilePath $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }
}

function Write-Utf8File {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Content
    )

    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

# Copies the first existing guide. Both languages are shipped when present; a
# single legacy `docs/user-guide.md` is used as a fallback.
function Copy-UserGuides {
    param([Parameter(Mandatory = $true)][string]$Destination)

    $copied = 0
    foreach ($language in @('ru', 'en')) {
        $guide = Join-Path $root "docs\$language\user-guide.md"
        if (Test-Path -LiteralPath $guide) {
            $target = Join-Path $Destination "docs\$language"
            New-Item -ItemType Directory -Path $target -Force | Out-Null
            Copy-Item -LiteralPath $guide -Destination (Join-Path $target 'user-guide.md')
            $copied++
        }
    }

    if ($copied -eq 0) {
        $legacy = Join-Path $root 'docs\user-guide.md'
        if (-not (Test-Path -LiteralPath $legacy)) {
            throw 'No user guide found (docs/ru, docs/en or docs/user-guide.md)'
        }
        $target = Join-Path $Destination 'docs'
        New-Item -ItemType Directory -Path $target -Force | Out-Null
        Copy-Item -LiteralPath $legacy -Destination (Join-Path $target 'user-guide.md')
        $copied = 1
    }

    Write-Host "Staged $copied user guide(s)."
}

$version = Get-WorkspaceVersion -CargoTomlPath (Join-Path $root 'Cargo.toml')
if ($version -notmatch '^\d+\.\d+\.\d+') {
    throw "Unsupported workspace version '$version'"
}

if ($VersionOnly) {
    Write-Output $version
    return
}

$stage = Join-Path $dist 'freespace-portable'
$zip = Join-Path $dist "freespace-portable-$version-windows-x64.zip"

if (-not $SkipBuild) {
    if (-not (Test-Path -LiteralPath (Join-Path $ui 'dist\index.html'))) {
        Write-Host 'Frontend assets are missing; building them with npm.'
        if (-not (Test-Path -LiteralPath (Join-Path $ui 'node_modules'))) {
            Invoke-External -FilePath 'npm' -Arguments @('ci') -WorkingDirectory $ui
        }
        Invoke-External -FilePath 'npm' -Arguments @('run', 'build') -WorkingDirectory $ui
    }

    # `--no-default-features` disables the optional `context-menu` feature, so
    # the portable build registers nothing in Explorer.
    Invoke-External -FilePath 'cargo' -Arguments @(
        'build', '--release', '--no-default-features',
        '-p', 'freespace-cli', '-p', 'freespace-desktop'
    ) -WorkingDirectory $root
}

$gui = Join-Path $root 'target\release\freespace-gui.exe'
$cli = Join-Path $root 'target\release\freespace.exe'
foreach ($binary in @($gui, $cli)) {
    if (-not (Test-Path -LiteralPath $binary)) {
        throw "Missing build artifact $binary"
    }
}

if (Test-Path -LiteralPath $stage) {
    Remove-Item -LiteralPath $stage -Recurse -Force
}
New-Item -ItemType Directory -Path $stage -Force | Out-Null

# The desktop package keeps the historical binary name, so shortcuts and
# scripts that point at `freespace-gui.exe` continue to work.
Copy-Item -LiteralPath $gui -Destination $stage
Copy-Item -LiteralPath $cli -Destination $stage
Copy-Item -LiteralPath (Join-Path $root 'CHANGELOG.md') -Destination $stage

Copy-UserGuides -Destination $stage

if ($WebView2Runtime -ne '') {
    $source = (Resolve-Path -LiteralPath $WebView2Runtime).Path
    if (-not (Test-Path (Join-Path $source 'msedgewebview2.exe'))) {
        throw "WebView2 runtime not found in $source (msedgewebview2.exe is missing)"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $stage 'webview2') -Recurse
}
else {
    Write-Warning 'No fixed WebView2 runtime bundled; the machine needs the Evergreen runtime.'
}

$data = Join-Path $stage 'data'
New-Item -ItemType Directory -Path $data -Force | Out-Null
$dataReadme = @'
FreeSpace portable data
=======================

Everything the application writes lives in this folder, next to the
executables:

  settings.toml    - user settings
  freespace.db     - journal, snapshots and the hash cache
  webview2\        - the WebView2 profile

FreeSpace never writes to Program Files, %APPDATA%, the registry or any other
system location. Delete the folder you unpacked the archive into and every
trace of the application is gone. Nothing is installed and nothing is left
behind; there is no uninstaller to run.

Keep this folder writable. If the application is started from a read-only
location, settings live in memory for the session only; there is no fallback
to a system folder.

WebView2
--------

The GUI uses Microsoft Edge WebView2. Without a bundled `webview2` folder next
to the executables, the Evergreen WebView2 runtime that ships with Windows 10
and Windows 11 is used. To make the archive self-contained, extract a fixed
WebView2 runtime into `webview2\`; the application detects it automatically.
FreeSpace never downloads anything: the application is fully offline.
'@
Write-Utf8File -Path (Join-Path $data 'README.txt') -Content $dataReadme

$archiveReadme = @"
FreeSpace $version - portable Windows x64
==========================================

FreeSpace is a disk space analyzer and cleanup tool. This archive is
self-contained apart from the Microsoft Edge WebView2 runtime used by the GUI.

Contents
--------

  freespace-gui.exe  - graphical interface
  freespace.exe      - command line interface (every GUI capability has a CLI
                       equivalent with --json output)
  data\README.txt    - what the application stores and where
  docs\ru, docs\en   - user guide
  CHANGELOG.md       - release history
  webview2\          - optional fixed WebView2 runtime, when bundled

Getting started
---------------

1. Extract the archive into a folder you can write to, for example
   C:\Tools\FreeSpace. Do not run the executables from inside the ZIP.
2. Start the GUI with freespace-gui.exe, or the CLI from a terminal:

       freespace.exe --json capabilities
       freespace.exe scan E:\ --json

3. The application creates its state in data\ next to the executables.

Portable model
--------------

Nothing is installed. The application does not write to Program Files,
%APPDATA%, %LOCALAPPDATA% or the registry, and it does not create Explorer
context-menu entries. All state stays in the data folder next to the
executables, so moving the folder moves the application and deleting it
removes every trace.

WebView2
--------

The GUI needs Microsoft Edge WebView2. Windows 10 and Windows 11 normally ship
the Evergreen runtime. When this archive contains a webview2 folder, that
fixed runtime is used instead; no installation or registry change is needed.
FreeSpace never downloads anything.

Builds are unsigned, so Windows SmartScreen may show a warning on first start.
"@
Write-Utf8File -Path (Join-Path $stage 'README.txt') -Content $archiveReadme

if (Test-Path -LiteralPath $zip) {
    Remove-Item -LiteralPath $zip -Force
}
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal

Write-Host "Portable archive written to $zip"
