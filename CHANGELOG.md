# Changelog

All notable changes to FreeSpace are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- The portable archives now embed the frontend instead of pointing at the development server. A plain
  `cargo build --release` without the `custom-protocol` feature produced a GUI that showed
  `ERR_CONNECTION_REFUSED` for `http://localhost:1420`; `installer/portable.ps1` and
  `installer/portable.sh` enable the feature and a regression test guards it.


## [2.0.0] тАФ 2026-11-20

### Added

- Linux x86_64 portable backend: directory-walk scanning, volume discovery from the mount table and
  `statvfs`, `st_blocks` allocation accounting with sparse-file handling, XDG trash, `/proc` write
  monitoring, `flock` instance locking and `xdg-open` reveal. MFT, USN, ETW, ACL, ADS, Restart
  Manager, shredding and Explorer integration are reported as unavailable.
- `freespace capabilities [--json]` and the GUI bootstrap capability descriptor. The payload exposes
  `platform`, `scan_methods`, `trash`, `permanent`, `shred`, `acl`, `attrs`, `ads`, `unlock`,
  `context_menu`, `monitor`, `elevation`, `long_paths`, `case_sensitive` and `max_path`.
- Portable-only distribution: `freespace-portable-<version>-windows-x64.zip` and
  `freespace-portable-<version>-linux-x86_64.tar.gz`, with `SHA256SUMS.txt` and no installers.
- Manual tagged release workflow, Linux CI job, platform-specific test gating and version-consistency
  checks across Cargo, Tauri and npm metadata.

### Changed

- The GUI is reduced to 9 screens in 6 groups: Overview, Tree, Analysis, Duplicates, Cleanup,
  Journal, Monitor, Inspect and Settings. Saved `shred`, `fileops`, `rename` and `permissions`
  identifiers resolve to the nearest live screen.
- Deletion offers exactly two modes: system trash (Recycle Bin on Windows, XDG trash on Linux) and
  permanent deletion. Both keep preview, explicit confirmation and journaling.
- Journal undo applies only to renames. Deletions are restored through the operating system trash;
  old `quarantine` and `restore` journal entries are read-only audit records.
- Bulk rename, permissions and attributes are CLI-only; contextual rename stays in the tree.
- The shredder remains only in the Windows CLI. On Linux the `shred`, `acl`, `attrs`, `unlock` and
  `context-menu` commands return `E_UNSUPPORTED` with exit code `8`.
- Portable builds store settings, the journal, caches, the WebView profile and the CLI lock in
  `data` next to the executables and create no registry entries.
- Version 2.0.0 marks the removal of the quarantine and the change of supported platforms.

### Removed

- Application quarantine: code, database table (`SCHEMA_VERSION` 2 with `DROP TABLE quarantine`),
  settings key `quarantine_retention_days`, DTO fields, GUI selection, CLI mode `quarantine`, the
  restore flow and the quarantine counter in the journal.
- GUI screens for the shredder and file operations (including permissions and rename screens), their
  routes, command-palette entries, hotkeys, i18n keys and Tauri commands. Explorer integration
  remains an optional Windows build feature; the shipped portable archive has no such command.
- NSIS installer configuration, `installer/nsis/hooks.nsh` and installer artifacts. The distribution
  contains only portable archives.
- Code signing and notarisation; releases are unsigned and the SmartScreen warning is documented.

### Fixed

- Database migration to schema 2 keeps every old journal entry readable while dropping the
  quarantine table and its indexes.
- An older `settings.toml` containing `quarantine_retention_days` still loads; the key is ignored.
- A `data/quarantine` folder from 1.x is neither used nor deleted automatically; the user guide
  explains manual cleanup.
- Unsupported platform features now fail with a stable `E_UNSUPPORTED` instead of an unclear error,
  and `freespace delete --mode quarantine` fails with `E_INVALID_ARGUMENT` instead of falling back to
  permanent deletion.
- Rename conflict checks respect case-sensitive Linux file systems, and paths beyond `PATH_MAX` are
  rejected before any data changes.

## [1.3.0] тАФ 2026-11-19

### Added

- Tree row context menu on right click and `Shift+F10`: open the object in
  Explorer, copy its full path, rename it after a preview, or move it to the
  Recycle Bin with an explicit confirmation. The confirmation is a compact
  dialog with the object name, type, size and modification date. The context
  menu never offers quarantine or permanent deletion; those stay on their own
  screens.
- `reveal_in_explorer` Tauri command backed by `freespace_platform::reveal`,
  which opens a directory as a folder and selects a file inside its parent.

### Changed

- The interface is reduced to 11 screens in 6 sidebar groups. The Treemap
  and the standalone scanning and export screens are gone: scanning progress is
  shown inline, and export is a global action.
- The frontend now uses React 19 and `@ui-sdk`; the visual regression suite and
  its baselines were retired.
- Documentation moved to `docs/ru` (Russian) and `docs/en` (English); one-off
  release, audit and performance reports and unused quality checklists were
  removed.

### Fixed

- Deleting an object that sits directly under a volume root (for example
  `E:\file.txt`) no longer fails the scanned-area check: a root with a trailing
  separator now matches its direct children. The same boundary check is used by
  the operation preview and the Explorer command.
- A successful recycle now removes the row from the tree immediately and
  decreases the loaded ancestor totals, including operations that the shell
  reports as finished only after the move. Deleted paths stay hidden after
  leaving and re-entering the tree until the next scan. The result of the
  Recycle Bin call follows the file-system state, so a shell error after a
  successful move no longer leaves the row in the tree.

## [1.2.0] тАФ 2026-11-16

### Added

- Native Tauri 2 + React interface with 24 screens, dark/light/system themes,
  100тАУ200% DPI support, reduced-motion and high-contrast handling, full keyboard
  navigation, Narrator announcements and a command palette.
- Virtualized tree and data tables, a Canvas treemap with spatial keyboard
  navigation, synchronized selection and a details panel.
- Shared safe-operation flow for every destructive action: preview, explicit
  deletion mode, confirmation token, progress, journal entry, undo and restore.
- Tauri commands with a typed TypeScript client, a mock transport for tests, and
  golden contract fixtures for every command.
- Russian and English localization generated from `locales/*.toml`, `Intl`
  formatting and a missing-key/cyrillic regression test.
- Visual regression matrix (24 screens ├Ч 2 themes ├Ч 4 DPI ├Ч 2 languages), a
  pull-request subset, axe accessibility matrix and storage/offline checks.
- NSIS installer hooks that keep Explorer entries disabled, create `data` and
  delete it on uninstall, plus an optional fixed WebView2 runtime for portable
  builds.

### Changed

- The desktop client is built from `apps/freespace-desktop` (Tauri) instead of
  the egui application; the CLI stays byte-compatible and keeps its JSON
  envelopes.
- The installer is Tauri NSIS with the offline WebView2 installer and a minimum
  runtime version; Inno Setup is withdrawn from the distribution.
- WebView2 profiles, settings, the journal, quarantine, snapshots and caches live
  only in `<exe_dir>\data`.

### Removed

- The egui/eframe client, its side-by-side UI and the Inno Setup script.

## [1.1.0] тАФ 2026-11-16

### Added

- `context_menu_enabled` setting (disabled by default) and a portable build
  produced with `--no-default-features` that contains no Explorer integration.
- `freespace context-menu uninstall --all` for idempotent cleanup of both user
  and machine registry scopes.
- Regression tests that production sources never resolve `%APPDATA%`,
  `%LOCALAPPDATA%`, `%ProgramData%` or temporary directories.

### Changed

- All persistent data now lives in `<exe_dir>\data`; settings, journal,
  quarantine, hash cache and the CLI lock no longer use the user profile.
- The Inno Setup installer registers no context-menu entries and the uninstaller
  removes `{app}\data` plus both registry scopes.
- The CLI binary is built as `freespace.exe`, matching the installer and portable
  archive layout.

### Removed

- `portable_mode` setting and `portable.flag` marker; storage is always
  application-local.

## [1.0.0] тАФ 2026-11-16

### Added

- Volume scanning through NTFS MFT enumeration with an automatic directory-walk
  fallback, USN incremental updates and access-denied reporting.
- Arena-backed size tree, treemap, largest-file and file-type reports, duplicate
  detection with a persistent hash cache.
- Safe operations layer: preview, explicit deletion mode, confirmation, journal,
  quarantine, undo, Recycle Bin and system-path protection.
- Snapshots, snapshot comparison, exact logical/allocated/physical accounting,
  long-path operations, bulk rename, attributes, ownership and ACL editing.
- Metadata extraction (EXIF, audio, video, PE), owner reports, archive inspection
  (ZIP, 7z, TAR, ISO, optional RAR) and CSV/XML/HTML/XLSX export.
- Full CLI parity with JSON envelopes and documented exit codes, a run lock and
  Task Scheduler profile export.
- Explorer context-menu installation in `HKCU` or `HKLM`, with Unicode-safe path
  quoting.
- Real-time write monitoring through ETW with a process-counter and USN fallback,
  thresholds, exclusions, history and notifications.
- On-demand UAC elevation and protected-directory reporting.
- Drive dashboard with usage summary and snapshot trends.
- Runtime RU/EN localization, settings screen, portable mode and `config get/set`.
- Documentation for users, the CLI contract, release validation, the Inno Setup
  installer and the portable ZIP build.

### Security

- The application makes no network calls and sends no telemetry; a regression test
  enforces the offline-only contract.
