# FreeSpace User Guide 1.2

FreeSpace is a native Windows 10/11 utility that analyses disk usage, shows size trees, finds
duplicates, helps reclaim space safely, and works completely offline.

## Installation and launch

- Portable build: unpack the ZIP into any folder and run `freespace-gui.exe`. The archive contains
  no Explorer integration; settings, the journal, cache and quarantine live in the `data` folder
  next to the executable.
- Installer: run `FreeSpace_1.3.0_x64-setup.exe` (the name contains the version). The Tauri NSIS
  installer installs a compatible WebView2 offline and creates no context-menu entries. All
  application data lives in `data` next to the executable.
- A portable build without a bundled `webview2` folder uses the system WebView2; for a fully
  self-contained run, place an extracted fixed runtime into the `webview2` folder.
- `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` and temporary directories are never used. If the
  application folder is read-only, settings apply until the application closes and saving reports
  an error; the application never falls back to a system location.
- Windows 10 21H2+ or Windows 11, x64. Administrator rights are only needed for whole-volume MFT
  scans, protected system directories and machine-wide installation.

## First run

1. Open **Overview** — every volume, used and free space, the file system and the trend reconstructed
   from stored snapshots.
2. Press **Analyze this volume** to start a scan. Progress, the current path, speed, object count and
   cancellation are shown directly on the Overview and repeated in the status bar.
3. Switch to **Tree**: sizes, inline share bars, type icons and a size-details card on row hover.

## Main screens

There are 11 screens, grouped into six sidebar sections.

**Overview**

- **Overview** — volumes, used and free space, embedded scan progress and scan options (`auto`,
  `mft`, `walk`, exclusions).

**Analysis**

- **Tree** — size hierarchy with inline bars, type icons, sorting, name and owner filters, search,
  pagination and the size-details card on hover (logical, allocated and physical size, parent
  share, compression, links, ADS). Right-clicking a row or pressing `Shift+F10` opens a menu with
  "Open in Explorer", "Copy path", "Rename" after a preview and "Delete to Recycle Bin" after
  confirmation; quarantine and permanent deletion stay on their own screens.
- **Analysis** — tabs for largest files, file types and owners.
- **Duplicates** — byte-verified groups, keep rules and quarantine for extra copies.

**Cleanup**

- **Cleanup** — tabs for recommendations (temporary data, caches, logs, backups, stale files and
  empty folders, with the risk level as a word and an icon) and snapshots (create and compare).

**Operations**

- **Journal** — operation history with filters, undo and restore.
- **Shredder** — irreversible overwrite with preview, method choice and warnings.
- **Write monitor** — processes writing to disk, speed, thresholds and history.

**Tools**

- **Inspect** — tabs for metadata, archives, unlock and long paths.
- **File operations** — tabs for rename and permissions (attributes, owner, ACL).

**System**

- **Settings** — tabs for settings, elevation and about: language, theme, units, exclusions,
  thresholds, quarantine, context menu, elevation mode, version and the `data` folder.
- **Export** is a CSV/XML/HTML/XLSX action on the screens that produce data, not a screen.

## Interface and accessibility

- All 11 screens in 6 groups are reachable from the sidebar, the command palette (`Ctrl+K`) and deep
  links; the last screen is restored after a restart, and values stored by older versions open the
  nearest new screen.
- Dark, light and system themes switch without a restart, and Windows theme changes are followed
  live. DPI 100/125/150/200% and reduced motion are supported.
- Everything is keyboard-operable: skip link, arrow keys in the tree, `←/→` in tabs, `Enter` to open,
  `Esc` to go back, and focus returns after dialogs. Narrator receives headings, progress and errors
  through `aria-live`.
- RU/EN switches in the settings without losing state; numbers, dates and sizes use the formatting
  rules of the selected language.
- Status is never conveyed by colour alone: every state has text and an icon.

## Safety

- Deletion always goes through a preview and confirmation, and the deletion mode is explicit:
  Recycle Bin, FreeSpace quarantine or permanent deletion.
- Protected system paths require an extra confirmation and elevation.
- Every destructive operation is journaled; quarantined objects can be restored.
- Real user files are changed only on an explicit user command. Tests use temporary directories and
  test VHDs.

## Context menu and uninstall

- No FreeSpace entries exist after installation. Enable them in **Settings** or with
  `freespace config set context_menu_enabled true`, and disable them with the reverse command.
- Remove the entries without uninstalling: `freespace context-menu uninstall --all`.
- The Tauri NSIS uninstaller removes the executables, the `data` directory, the WebView2 profile and
  the menu entries for the current user. The portable build contains no `context-menu` command and
  creates no registry entries.

## Command line

The CLI mirrors the GUI and emits JSON. See [the CLI contract](../ru/cli.md) (Russian) for commands
and exit codes.

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
- **Settings are not saved**: check permissions on the `data` folder next to the executable or
  unpack the application into a writable directory. There is no fallback save to `%APPDATA%`,
  `%LOCALAPPDATA%` or `%ProgramData%`.
- **No context-menu entries**: enable `context_menu_enabled` in the settings; the portable build does
  not support the context menu.
