# Changelog

All notable changes to FreeSpace are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.3.0] — 2026-11-19

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

## [1.2.0] — 2026-11-16

### Added

- Native Tauri 2 + React interface with 24 screens, dark/light/system themes,
  100–200% DPI support, reduced-motion and high-contrast handling, full keyboard
  navigation, Narrator announcements and a command palette.
- Virtualized tree and data tables, a Canvas treemap with spatial keyboard
  navigation, synchronized selection and a details panel.
- Shared safe-operation flow for every destructive action: preview, explicit
  deletion mode, confirmation token, progress, journal entry, undo and restore.
- Tauri commands with a typed TypeScript client, a mock transport for tests, and
  golden contract fixtures for every command.
- Russian and English localization generated from `locales/*.toml`, `Intl`
  formatting and a missing-key/cyrillic regression test.
- Visual regression matrix (24 screens × 2 themes × 4 DPI × 2 languages), a
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

## [1.1.0] — 2026-11-16

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

## [1.0.0] — 2026-11-16

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
