# Quickstart: Verifying FreeSpace

**Feature**: `001-freespace-disk-analyzer` | **Date**: 2026-11-15

> **2.0 note (2026-11-20)**: the scenarios below belong to 1.x. Quarantine, restore-from-quarantine
> and installer steps do not apply in 2.0; the current check is
> [003/quickstart.md](../../specs/003-cross-platform-portable/quickstart.md). In 2.0 deletion uses
> the Recycle Bin/XDG trash or permanent deletion, and the GUI has 9 screens.

A guide to building and end-to-end verification after implementation. The scenarios correspond to the user
stories and success criteria in [spec.md](./spec.md).

## Prerequisites

- Windows 10 21H2+ or Windows 11, x64.
- Rust stable 1.85+ (edition 2024), the `clippy` and `rustfmt` components.
- For administrative scenarios — a terminal running as administrator.
- A free test NTFS volume or VHD (≥ 2 GB recommended) for verifying MFT, permissions, and quarantine.

## Build and Run

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cd apps/freespace-desktop/ui; npm ci; npm run tauri dev
```

CLI:

```powershell
cargo run -p freespace-cli -- --help
cargo run -p freespace-cli -- drives --json
```

## Verification Scenarios

### 1. Scanning and tree (US1)

```powershell
cargo run -p freespace-cli -- scan C:\ --json
cargo run -p freespace-cli -- tree C:\ --depth 2 --json
```

Expected: valid JSON, the root's total size is consistent with the child nodes, inaccessible
directories are flagged, scan time for a volume up to 8 TB — no more than 60 s (on an SSD).

### 2. Treemap (US2)

Launch the GUI, open the map, and verify:

- rectangle areas are proportional to sizes;
- zoom and breadcrumbs work;
- tooltips contain the name, path, and share;
- coloring by type matches the legend;
- on a test volume with 1 million files, the UI remains responsive.

### 3. Unlock (US3)

Preparation: start a process that holds the file (for example, open the file in an editor or
use a test helper).

```powershell
cargo run -p freespace-cli -- unlock <file> --json
cargo run -p freespace-cli -- unlock <file> --soft --yes --json
```

Expected: a list of processes and handles; after a soft close the file is accessible; a journal entry.

### 4. Top files and types (US4)

```powershell
cargo run -p freespace-cli -- top C:\ --limit 100 --json
cargo run -p freespace-cli -- types C:\ --json
```

Expected: the 100 largest files; totals by extension match the tree.

### 5. Duplicates (US5)

```powershell
cargo run -p freespace-cli -- duplicates <test-dir> --algorithm sha256 --json
```

Expected: groups containing only byte-identical files; no false matches; one copy
always remains.

### 6. Shredder and delete-on-reboot (US6)

```powershell
cargo run -p freespace-cli -- shred <test-dir>\secret.txt --passes 3 --pattern random --yes --json
```

Expected: a warning about irreversibility; a journal entry; recovery with an independent
tool is impossible (HDD). On an SSD — a warning and TRIM.

### 7. Outdated files and empty folders (US7)

```powershell
cargo run -p freespace-cli -- cleanup suggest <test-dir> --older-than 180 --json
cargo run -p freespace-cli -- cleanup empty-dirs <test-dir> --json
```

Expected: only objects whose latest access/modification date is older than the threshold; empty
directories are listed in full.

### 8. Snapshots (US8)

```powershell
cargo run -p freespace-cli -- snapshot create C:\ --out before.fssnap --json
# ... changes on the disk ...
cargo run -p freespace-cli -- snapshot create C:\ --out after.fssnap --json
cargo run -p freespace-cli -- snapshot diff before.fssnap after.fssnap --json
```

Expected: the report contains all added/removed/changed objects and growth by directory.

### 9. Safe operations (US9)

Verify in the GUI and CLI:

- preview with the choice "Recycle Bin / Quarantine / Permanently";
- deletion to quarantine when space is insufficient — a warning and a choice;
- restoration from quarantine;
- a journal entry and undo where applicable.

### 10. Accurate space accounting (US10)

On a test set with hard links, junctions, compressed and sparse files, and ADS:

```powershell
cargo run -p freespace-cli -- scan <test-dir> --json
```

Expected: physical size with deduplication, logical — by the number of links; ADS are accounted for.

### 11. Long paths (US11)

Create a path longer than 260 characters (for example, using nested directories) and run:

```powershell
cargo run -p freespace-cli -- rename <long-path> --regex "a" --replacement "b" --yes --json
```

Expected: successful renaming without enabling `LongPathsEnabled`.

### 12. Bulk renaming (US12)

```powershell
cargo run -p freespace-cli -- rename <test-dir>\* --regex "IMG_(\d+)" --replacement "Photo_$1" --preview --json
```

Expected: preview, conflict detection, rollback after applying.

### 13. Attributes and permissions (US13)

```powershell
cargo run -p freespace-cli -- attrs <test-dir> --clear read-only --yes --json
cargo run -p freespace-cli -- acl <test-dir> --owner <account> --yes --json
```

Expected: a report of successful and failed objects; if permissions are insufficient — a request for elevation.

### 14. Metadata and owners (US14, US15)

```powershell
cargo run -p freespace-cli -- scan <media-dir> --json
cargo run -p freespace-cli -- export <media-dir> --format csv --out report.csv --include owners
```

Expected: correct EXIF/bitrate/resolution/PE version; totals by owner match the tree.

### 15. Archives (US16)

```powershell
cargo run -p freespace-cli -- archives <test-dir> --json
```

Expected: the contents of ZIP/7z/TAR/ISO without extraction; correct handling of passwords and corruption.

### 16. Export (US17)

```powershell
cargo run -p freespace-cli -- export C:\ --format csv --out tree.csv --json
cargo run -p freespace-cli -- export C:\ --format xml --out tree.xml --json
cargo run -p freespace-cli -- export C:\ --format html --out tree.html --json
cargo run -p freespace-cli -- export C:\ --format xlsx --out tree.xlsx --json
```

Expected: all files open in the target tools, and totals match.

### 17. CLI and scheduling (US18)

```powershell
cargo run -p freespace-cli -- scan C:\ --json | ConvertFrom-Json
cargo run -p freespace-cli -- schedule export default --out task.xml
```

Expected: valid JSON, documented exit codes, the XML task is accepted by Task Scheduler.

### 18. Context menu (US19)

```powershell
cargo run -p freespace-cli -- config get context_menu_enabled
cargo run -p freespace-cli -- config set context_menu_enabled true
```

Verify the menu items for a file, folder, and drive; then disable and clean both scopes:

```powershell
cargo run -p freespace-cli -- config set context_menu_enabled false
cargo run -p freespace-cli -- context-menu uninstall --all --yes
```

Expected: no entries exist after installation; enabling the setting adds them and disabling removes
them; `uninstall --all` is idempotent. The portable build
(`cargo build --release --no-default-features -p freespace-cli`) contains no
`context-menu` command.

### 19. Monitoring (US20)

```powershell
cargo run -p freespace-cli -- monitor --threshold 1073741824 --json
```

Expected: a stream of events with the process, path, and rate; a notification when the threshold is exceeded.

### 20. System directories (US21)

Launch the GUI as administrator, open `C:\System Volume Information`, and confirm access;
launch it without elevation — verify that inaccessibility is flagged correctly.

### 21. Dashboard, settings, localization (US22, US23)

```powershell
cargo run -p freespace-cli -- drives --json
cargo run -p freespace-cli -- config set language ru
cargo run -p freespace-cli -- config set language en
```

Expected: all volumes in the summary; full localization; settings, journal and quarantine are in
`<application folder>\data`, and `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` and temporary
directories are not created. Verify uninstall: after removal the `data` directory and menu entries
are absent.

## Release Readiness Criteria

- `cargo test --workspace` — green.
- `cargo clippy --workspace --all-targets -- -D warnings` — no warnings.
- All scenarios above are completed on a clean Windows 10 and Windows 11 machine.
- Administrative scenarios (`FREESPACE_ADMIN_TESTS=1`) passed on a test NTFS volume.
- Report checksums match the reference values.
