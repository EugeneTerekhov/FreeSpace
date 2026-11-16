# Changelog

All notable changes to FreeSpace are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
