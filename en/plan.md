# Implementation Plan: FreeSpace — Windows Disk Space Analyzer and Manager

**Branch**: `001-freespace-disk-analyzer` | **Date**: 2026-11-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-freespace-disk-analyzer/spec.md`

## Summary

FreeSpace is a native Windows application (Rust + egui/eframe) for analyzing and managing disk
space. At its core is a high-performance NTFS scanner based on direct MFT reads with
incremental updates through the USN journal and fallback traversal for other file systems.
A graphical interface, CLI, and system integrations run on top of the shared core. The application
combines visualization (tree, Treemap), duplicate search, safe cleanup with quarantine and
a journal, advanced file system work (links, long paths, NTFS permissions, metadata,
archives), and automation (reports, scheduling, context menu, monitoring).

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024), Windows x64

**Primary Dependencies**:
- UI: `eframe`/`egui` (native immediate-mode GUI), `egui_extras` (virtualized tables)
- System calls: `windows` (official Microsoft crate), `winreg`
- Concurrency: `rayon`, `crossbeam-channel`, `jwalk` (fallback traversal)
- Storage: `rusqlite` (bundled SQLite — journal, quarantine, settings), `serde` + `toml`/`json`
- Identifiers: `uuid` (v4 for operations and sessions, v5 for stable volume identifiers)
- Snapshots: `serde` + `ciborium` (CBOR) + `zstd`
- Hashing: `sha2`, `md-5`, `memmap2`
- Archives: `zip`, `sevenz-rust`, `tar` + `flate2`, `iso9660`; `unrar` — optional (RAR)
- Metadata: `kamadak-exif`, `lofty` (audio), `mp4parse` (video), `pelite` (PE versions)
- Reports: `csv`, `quick-xml`, `rust_xlsxwriter`, HTML templates
- CLI: `clap` v4
- Localization: `rust-i18n` (RU/EN, runtime switching)
- Search/filters: `globset`
- Monitoring: `ferrisetw` (ETW, provider `Microsoft-Windows-Kernel-File`) with fallback to
  `GetProcessIoCounters`

**Storage**: SQLite (operations journal, quarantine index, automation settings); snapshot files
`.fssnap` (CBOR + zstd, versioned schema); TOML/JSON configuration

**Testing**: `cargo test` (unit + integration), `assert_cmd`/`predicates` (CLI), `tempfile` and
`assert_fs` (file system fixtures), `criterion` (benchmarks), administrative tests on NTFS/VHD —
only behind an explicit environment flag

**Target Platform**: Windows 10 21H2+ and Windows 11, x64; NTFS as the primary file system

**Project Type**: desktop-app + CLI (Cargo workspace)

**Performance Goals**:
- Analysis of an NTFS volume up to 8 TB via MFT — no more than 60 s on a consumer SSD
- UI response to user actions — no more than 200 ms while scanning
- Treemap rendering — at least 30 frames/s in typical scenarios, smooth zoom
- Duplicate search on 1 TB — no more than 10 minutes (with a warm file cache)
- Incremental update via USN — no more than 5 s for 100 thousand changes

**Constraints**:
- Full offline operation, no telemetry or network requests
- Memory budget per scanning session of 10 million nodes — no more than 4 GB
- Support for paths up to 32,000 characters without the LongPathsEnabled system policy
- UAC elevation on demand; correct degradation without administrator permissions
- All destructive operations — only after confirmation and with journaling
- GUI and CLI use the shared core crates; feature parity is mandatory

**Scale/Scope**: up to 10 million files per volume, volumes up to 8 TB, 23 user stories, 95
functional requirements; 8 core/utility crates + GUI application

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Compliance | Rationale |
|-----------|------------|-----------|
| I. Safety of destructive operations | PASS | Single operations layer: preview, choice of deletion method, quarantine/Recycle Bin, journal, undo, system path protection |
| II. Accuracy of space accounting | PASS | `SizeMetrics` (logical/on disk/physical), hard link deduplication by FRN, ADS, sparse/compressed |
| III. Performance at large scale | PASS | MFT + USN, `rayon`, streaming result delivery, Treemap LOD, table virtualization |
| IV. Native Windows accuracy | PASS | `windows` crate, verbatim paths, Restart Manager, ACL API, TRIM, registry, ETW |
| V. Test-first approach | PASS | File system fixtures, integration tests, benchmarks, administrative tests behind a flag |
| VI. CLI and GUI parity | PASS | Shared core crates, CLI on `clap` with JSON output and documented return codes |
| VII. Privacy and offline | PASS | No network dependencies or telemetry; all data is local |

There are no violations requiring justification in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/001-freespace-disk-analyzer/
├── plan.md              # This file
├── research.md          # Phase 0: technical decisions and alternatives
├── data-model.md        # Phase 1: data model
├── quickstart.md        # Phase 1: validation scenarios
├── contracts/           # Phase 1: CLI, report and snapshot contracts
│   ├── cli.md
│   ├── report-formats.md
│   └── snapshot-format.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2: tasks (/speckit.tasks)
```

### Source Code (repository root)

```text
Cargo.toml                        # workspace, shared build profiles
rust-toolchain.toml               # pinning the stable toolchain
crates/
├── freespace-core/               # domain model, tree, size metrics, settings
│   ├── src/
│   └── tests/
├── freespace-platform/           # WinAPI wrappers: volumes, MFT/USN, locks, ACL, TRIM, registry, ETW
│   ├── src/
│   └── tests/
├── freespace-scan/               # MFT and tree traversal scanners, incremental sessions
│   ├── src/
│   └── tests/
├── freespace-ops/                # operations: deletion, quarantine, journal, shredder, rename, permissions
│   ├── src/
│   └── tests/
├── freespace-analysis/           # duplicates, cleanup, metadata, owners, archives
│   ├── src/
│   └── tests/
├── freespace-report/             # CSV/XML/HTML/Excel export, snapshots and comparison
│   ├── src/
│   └── tests/
└── freespace-cli/                # command line (clap), JSON output, return codes
    ├── src/
    └── tests/
apps/
└── freespace-gui/                # egui/eframe: tree, Treemap, feature screens
    ├── src/
    └── tests/
tests/                            # cross-crate integration scenarios (CLI + core)
benches/                          # shared scanning and Treemap benchmarks
docs/                             # user documentation (if needed)
ru/                               # Russian project documentation
en/                               # English project documentation
```

**Structure Decision**: Cargo workspace with separation by responsibility. `freespace-core` does
not depend on WinAPI and contains the domain model and algorithms, which allows them to be tested
without system calls. `freespace-platform` encapsulates all Windows API calls. Scanning,
operations, analysis, and reports are separate crates that depend on `core` (and `platform` when
necessary). GUI and CLI are thin frontends over the shared crates, which ensures feature parity.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |

There are no constitution violations; the structural complexity is justified by the requirements
for WinAPI isolation and CLI/GUI parity.
