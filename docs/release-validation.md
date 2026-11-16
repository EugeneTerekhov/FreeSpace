# Release validation — FreeSpace 1.1.0

**Date**: 2026-11-16
**Environment**: Windows 11 23H2 (build 22631), x64; Rust 1.93.0; cargo 1.93.0.
**Status**: automated scenarios passed on Windows 11. Windows 10 21H2+ hardware was not available in
this environment, so the manual checks that require it remain open (see the last section).

## Automated gates

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| `cargo test --workspace` | passed (all unit and integration suites) |
| `cargo test --workspace` with `FREESPACE_ADMIN_TESTS=1` | not run in this session (requires an elevated runner and a test VHD) |

## Quickstart scenarios

Scenarios were executed against temporary fixtures and the real machine where it is read-only.
Commands use `cargo run -p freespace-cli --` in development and the release executables in the
portable ZIP.

| # | Scenario | Result |
| --- | --- | --- |
| 1 | Scan and tree (US1) | passed on a temporary fixture; MFT fallback covered by `scan_mft` and `scan_walk` |
| 2 | Treemap (US2) | interactive check on Windows 11; layout, zoom and category colours verified |
| 3 | Unlock (US3) | passed via `tests/unlock.rs` (temporary file, no real process termination) |
| 4 | Top files and types (US4) | passed via `tests/top_types.rs` |
| 5 | Duplicates (US5) | passed via `tests/duplicates.rs` |
| 6 | Shredder (US6) | passed via `tests/shred.rs` on a temporary directory |
| 7 | Stale files and empty folders (US7) | passed via `tests/cleanup.rs` |
| 8 | Snapshots (US8) | passed via `tests/snapshot.rs` and `tests/dashboard.rs` |
| 9 | Safe operations (US9) | passed via `tests/safe_ops.rs` (quarantine, journal, confirmation) |
| 10 | Exact accounting (US10) | passed via `tests/accounting.rs` (sparse, compressed, links, ADS) |
| 11 | Long paths (US11) | passed via `tests/long_paths.rs` (>260 characters, verbatim prefixes) |
| 12 | Bulk rename (US12) | passed via `tests/rename.rs` (preview, conflicts, rollback) |
| 13 | Attributes and ACL (US13) | passed via `tests/permissions.rs`; owner change is admin-gated |
| 14 | Metadata and owners (US14, US15) | passed via `tests/metadata.rs` and `tests/owners.rs` |
| 15 | Archives (US16) | passed via `tests/archives.rs` (ZIP, 7z, TAR, ISO, damaged files) |
| 16 | Export (US17) | passed via `tests/export.rs`; all four formats validated |
| 17 | CLI and scheduling (US18) | passed via `tests/cli_contract.rs` (envelopes, codes, UTF-16 XML) |
| 18 | Context menu (US19) | passed via `tests/context_menu.rs` under an isolated `HKCU` subtree; default-off setting covered by `tests/settings.rs` |
| 19 | Monitoring (US20) | passed via `tests/monitor.rs` and `cargo run -p freespace-cli -- monitor --iterations 1 --interval 0 --no-etw --json` |
| 20 | Protected directories (US21) | elevation state and protected-area probing covered; admin access check is opt-in |
| 21 | Dashboard, settings, localization (US22, US23) | passed via `tests/dashboard.rs`, `tests/settings.rs`, `tests/privacy.rs` |
| 22 | Portable build without context menu | `cargo check --workspace --all-targets --no-default-features` passed; command absent from the stripped binary |
| 23 | Install/uninstall cleanup | installer removes leftover entries and writes the setting as disabled; uninstaller removes `{app}\data` and calls `context-menu uninstall` for both scopes |

## Manual checks performed

- Application-local storage: settings, journal and quarantine were created in `<exe_dir>\data`;
  no `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` or temporary files were created.
- Read-only application folder: settings remained in memory and the save reported an error without
  falling back to a system directory.
- Language switch RU ↔ EN at runtime without losing the scan or settings state.
- Light/dark/system themes redraw the map and tables correctly.
- Explorer integration is absent after a fresh install; enabling `context_menu_enabled` adds the
  entries and disabling it removes them.
- `freespace context-menu uninstall --all` is idempotent and removes both scopes.
- `freespace drives --json` and `freespace config set language ru|en` round-trip.

## Open items

- Windows 10 21H2+ run of the quickstart scenarios (hardware not available here).
- Administrative suite (`FREESPACE_ADMIN_TESTS=1`) on a dedicated NTFS test VHD.
- ETW monitoring with a real administrator session on a machine with active writers.
- A native notification-area icon: monitoring runs in the background while the application is open,
  and notifications are shown in-app; a shell tray icon is planned for the next release.
- Inno Setup compile and code signing on a release machine.
