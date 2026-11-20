# FreeSpace User Guide 2.0

FreeSpace is a portable cross-platform utility (Windows 10/11 x64 and Linux x86_64) that analyses
disk usage, shows size trees, finds duplicates, helps reclaim space safely, and works completely
offline.

## Platforms and distribution

| Platform | Artifact | Scanning | Trash |
|----------|----------|----------|-------|
| Windows 10 21H2+ / Windows 11 x64 | `freespace-portable-<version>-windows-x64.zip` | MFT, USN, directory walk | Windows Recycle Bin |
| Linux x86_64 | `freespace-portable-<version>-linux-x86_64.tar.gz` | directory walk | XDG trash |

- There are no installers: no NSIS, MSI, deb/rpm, AppImage, Flatpak or Snap. Unpack the application
  into any folder and run it in place.
- Builds are unsigned. Windows SmartScreen may warn about an unknown publisher; this is expected.
  Verify the `SHA256SUMS.txt` checksum from the release.
- Settings, the journal, the hash cache, the WebView2/WebKit profile and the CLI lock live in the
  `data` folder next to the executables. `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` and
  temporary directories are never used.
- The portable build creates no registry entries. Uninstalling means deleting the unpacked folder
  together with `data`.
- If the application folder is read-only, settings apply until the application closes and saving
  reports an error; the application never falls back to a system location.

## Launch

- **Windows**: unpack the ZIP into any folder and run `freespace-gui.exe` (GUI) or `freespace.exe`
  (CLI). Without a bundled `webview2` folder the system Evergreen WebView2 is used; for a fully
  self-contained run, place an extracted fixed runtime into the `webview2` folder.
- **Linux**: unpack the archive and run `./run.sh`, `./freespace-gui` or `./freespace`. WebKitGTK
  4.1 and GTK3 must be installed from the distribution repository; no installer or `.desktop` file
  is created. "Portable" means no installation, not the absence of system libraries.
- Administrator rights are only needed for whole-volume MFT scans, protected system directories and
  elevation in Windows. On Linux no elevation is required for ordinary user directories.

## First run

1. Open **Overview** — every volume, used and free space, the file system and the trend reconstructed
   from stored snapshots.
2. Press **Analyze this volume** to start a scan. Progress, the current path, speed, object count and
   cancellation are shown directly on the Overview and repeated in the status bar. Linux always uses
   the directory walk; on Windows the method is chosen automatically (`auto`, `mft`, `walk`).
3. Switch to **Tree**: sizes, inline share bars, type icons and a size-details card on row hover.

## Main screens

There are 9 screens, grouped into six sidebar sections.

**Overview**

- **Overview** — volumes, used and free space, embedded scan progress and scan options (`auto`,
  `mft`, `walk`, exclusions).

**Analysis**

- **Tree** — size hierarchy with inline bars, type icons, sorting, name and owner filters, search,
  pagination and the size-details card on hover (logical, allocated and physical size, parent
  share, compression, links, ADS). Right-clicking a row or pressing `Shift+F10` opens a menu with
  "Open in Explorer", "Copy path", "Rename" after a preview and "Delete to Recycle Bin" after
  confirmation. Permanent deletion is available through the operation preview on analysis screens;
  the application quarantine no longer exists.
- **Analysis** — tabs for largest files, file types and owners.
- **Duplicates** — byte-verified groups, keep rules and deletion of extra copies to the Recycle Bin
  or permanently after a preview.

**Cleanup**

- **Cleanup** — tabs for recommendations (temporary data, caches, logs, backups, stale files and
  empty folders, with the risk level as a word and an icon) and snapshots (create and compare).

**Operations**

- **Journal** — operation history with filters; undo is available only for renames, and deletion is
  restored through the operating system Recycle Bin.
- **Write monitor** — processes writing to disk, speed, thresholds and history.

**Tools**

- **Inspect** — tabs for metadata, archives, unlock and long paths. Bulk rename, permissions,
  attributes and the shredder are CLI-only. On Linux tabs that require NTFS/ACL/ADS/Restart Manager
  are hidden or marked unavailable.

**System**

- **Settings** — tabs for settings and about: language, theme, units, exclusions, thresholds,
  context menu, elevation mode, version and the `data` folder.
- **Export** is a CSV/XML/HTML/XLSX action on the screens that produce data, not a screen.

## Platform differences

| Capability | Windows | Linux |
|------------|---------|-------|
| Scanning | MFT, USN increment, directory walk | directory walk only |
| Deletion | Windows Recycle Bin, permanent | XDG trash, permanent |
| Shredder | CLI only | `E_UNSUPPORTED` |
| ACL, attributes, ADS | CLI and inspection (Windows) | unavailable |
| File unlock | Restart Manager | `E_UNSUPPORTED` |
| Explorer context menu | Windows build without integration | none |
| Long paths | verbatim prefixes | `PATH_MAX` limit (4096) |
| Name case | case-insensitive | case-sensitive |

The exact capability set of the current build is printed by `freespace capabilities --json`. The
GUI hides or disables unavailable actions, and the CLI returns `E_UNSUPPORTED` with exit code 8.

## Interface and accessibility

- All 9 screens in 6 groups are reachable from the sidebar, the command palette (`Ctrl+K`) and deep
  links; the last screen is restored after a restart, and values stored by older versions (`shred`,
  `fileops`, `rename`, `permissions`) open the nearest new screen.
- Dark, light and system themes switch without a restart, and system theme changes are followed
  live. DPI 100/125/150/200% and reduced motion are supported.
- Everything is keyboard-operable: skip link, arrow keys in the tree, `←/→` in tabs, `Enter` to open,
  `Esc` to go back, and focus returns after dialogs. Screen readers receive headings, progress and
  errors through `aria-live`.
- RU/EN switches in the settings without losing state; numbers, dates and sizes use the formatting
  rules of the selected language.
- Status is never conveyed by colour alone: every state has text and an icon.

## Safety and deletion modes

- Deletion always goes through a preview and confirmation, and the deletion mode is explicit:
  Recycle Bin (Windows) / XDG trash (Linux) or permanent deletion.
- **Trash** moves objects to the system trash: Explorer restores them on Windows, and a file manager
  restores them on Linux according to freedesktop.org. If trash is unavailable on a volume, the
  application reports an error and offers permanent deletion with a separate confirmation; it never
  silently switches to permanent.
- **Permanent** removes objects irreversibly and always warns that the action cannot be undone.
- Protected system paths require an extra confirmation and elevation.
- Every destructive operation is journaled. Only a rename can be undone; objects in the trash are
  restored with the operating system tools. The application quarantine and its restore no longer
  exist.
- Real user files are changed only on an explicit user command. Tests use temporary directories and
  test VHDs.

## Context menu and uninstalling

- The tree row context menu (open, copy path, rename, delete) works inside the application and needs
  no Explorer integration.
- No FreeSpace entries exist in the Explorer context menu after unpacking. The portable build is
  compiled without the `context-menu` feature, so it has no `freespace context-menu` command.
- The portable build creates no registry entries and has no installer; uninstalling means deleting
  the unpacked folder together with `data`.

## CLI-only features

The following operations were removed from the GUI but remain available from the command line:

- `freespace shred` — file overwriting, Windows only; returns `E_UNSUPPORTED` on Linux;
- `freespace acl` — NTFS owner and permissions, Windows only;
- `freespace attrs` — file attributes, Windows only;
- `freespace rename` — bulk rename by regular expression;
- `freespace unlock` — lock release, Windows only.

Contextual rename of a single file stays in the GUI tree with a preview and confirmation.

## Command line

The CLI mirrors the GUI and emits JSON. See [the CLI contract](cli.md) for commands, `capabilities`
fields and exit codes.

```powershell
freespace capabilities --json
freespace drives --json
freespace scan C:\ --json
freespace scan /home --method walk --json
freespace tree C:\ --depth 2 --json
freespace duplicates D:\Downloads --json
freespace delete D:\Downloads\old.bin --mode recycle --yes --json
freespace monitor --threshold 1073741824 --json
freespace config set language en
```

Parallel runs are serialised by a lock; `--lock-timeout` controls the wait. Destructive commands
require `--yes` in non-interactive mode.

## Migrating from 1.x

- On the first 2.0 run the journal database is upgraded to schema 2: the quarantine table is
  dropped and old journal entries are preserved and remain readable.
- Old entries with the `quarantine` and `restore` kinds are shown as audit records only: there is no
  restore button, and no new quarantine entries are created.
- The `quarantine_retention_days` key in an old `settings.toml` is ignored; all other settings load
  without an error.
- The old `data/quarantine` folder from 1.x is no longer used and is not deleted automatically;
  inspect it and remove it manually when it is no longer needed.

## Privacy

FreeSpace makes no network calls, sends no telemetry and requires no account. All data stays on the
machine. The regression test `tests/privacy.rs` verifies this at source and dependency level.

## Troubleshooting

- **Inaccessible directories**: limited mode marks them in the tree; on Windows restart the
  application as administrator to open protected directories.
- **MFT unavailable**: FAT32/exFAT uses the directory walk, and NTFS without rights falls back
  automatically. MFT is never used on Linux.
- **Monitor without paths**: on Windows without administrator rights ETW is unavailable, so process
  counters and USN path hints are shown instead. Linux uses `/proc` counters.
- **Settings are not saved**: check permissions on the `data` folder next to the executable or
  unpack the application into a writable directory. There is no fallback save to `%APPDATA%`,
  `%LOCALAPPDATA%` or `%ProgramData%`.
- **Trash is unavailable**: XDG trash may be unavailable on a network or removable volume; choose
  permanent deletion with a separate confirmation, or copy the data to a local volume.
- **SmartScreen warns**: the build is unsigned; compare the SHA-256 with `SHA256SUMS.txt` and run
  the file from the unpacked archive.
- **No Explorer context-menu entries**: the portable build contains no Explorer integration; use the
  in-application tree context menu instead.
