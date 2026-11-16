# FreeSpace User Guide 1.0

FreeSpace is a native Windows 10/11 utility that analyses disk usage, shows trees and maps, finds
duplicates, helps reclaim space safely, and works completely offline.

## Installation and launch

- Portable mode: unpack the ZIP and place an empty `portable.flag` file next to
  `freespace-gui.exe`. Settings, the journal and quarantine stay in the application folder.
- Regular mode: install with `freespace-setup.exe`. Settings live in `%APPDATA%\FreeSpace` and the
  operation journal in `%LOCALAPPDATA%\FreeSpace`.
- Windows 10 21H2+ or Windows 11, x64. Administrator rights are only needed for whole-volume MFT
  scans and protected system directories.

## First run

1. Open **Drives** — the dashboard shows every volume, used and free space, the file system and the
   trend reconstructed from stored snapshots.
2. Press **Analyze this volume** to start a scan and open the tree.
3. Explore **Tree** and **Treemap**: sizes, shares, file types and breadcrumb navigation.

## Main screens

- **Scan** — volume or directory, method (`auto`, `mft`, `walk`) and exclusions.
- **Tree** — sorting, name and owner filters, expansion, exact sizes.
- **Treemap** — proportional rectangles, category colours and zoom.
- **Largest files** and **Types** — top objects and distribution by extension.
- **Duplicates** — byte-verified groups, keep rules and quarantine for extra copies.
- **Cleanup** — temporary data, caches, logs, backups, stale files and empty folders.
- **Snapshots** — save a volume state and compare two snapshots.
- **Size details** — logical, allocated and physical size, compression, links and ADS.
- **Long paths** — operations on paths longer than 260 characters without a system policy.
- **Metadata** and **Owners** — EXIF, media, PE versions and space by owner.
- **Archives** — inspect ZIP/7z/TAR/ISO/RAR without unpacking and export the list.
- **Export** — CSV, XML, HTML and XLSX with selectable sections.
- **Unlock** — find processes that hold a file and close or terminate them.
- **Shredder** — irreversible overwrite with an SSD warning.
- **Write monitor** — processes writing to disk, speed, thresholds and history.
- **Elevation** — current mode, protected directories and restart as administrator.
- **Settings** — language, theme, units, exclusions, thresholds, quarantine and portable mode.

## Safety

- Deletion always goes through a preview and confirmation, and the deletion mode is explicit:
  Recycle Bin, FreeSpace quarantine or permanent deletion.
- Protected system paths require an extra confirmation and elevation.
- Every destructive operation is journaled; quarantined objects can be restored.
- Real user files are changed only on an explicit user command. Tests use temporary directories and
  test VHDs.

## Command line

The CLI mirrors the GUI and emits JSON. See [cli.md](cli.md) for the contract and exit codes.

```powershell
freespace drives --json
freespace scan C:\ --json
freespace tree C:\ --depth 2 --json
freespace duplicates D:\Downloads --json
freespace monitor --threshold 1073741824 --json
freespace config set language en
```

Parallel runs are serialised by a lock; `--lock-timeout` controls the wait. Destructive commands
require `--yes` in non-interactive mode.

## Privacy

FreeSpace makes no network calls, sends no telemetry and requires no account. All data stays on the
machine. The regression test `tests/privacy.rs` verifies this at source and dependency level.

## Troubleshooting

- **Inaccessible directories**: limited mode marks them in the tree; open **Elevation** and restart
  the application as administrator.
- **MFT unavailable**: FAT32/exFAT uses the directory walk, and NTFS without rights falls back
  automatically.
- **Monitor without paths**: ETW needs administrator rights, so process counters and USN path hints
  are shown instead.
- **Settings are not saved**: check permissions on `%APPDATA%\FreeSpace` or enable portable mode with
  a `portable.flag` file next to the executable.
