# Tasks: FreeSpace — Windows Disk Space Analyzer and Manager

**Input**: Design documents from `/specs/001-freespace-disk-analyzer/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/, quickstart.md

**Tests**: Test tasks are included for critical modules per the Constitution, principle V
(test-first approach to file system operations). For purely UI tasks, separate tests are
not provided.

**Organization**: Tasks are grouped by user story so that each can be implemented and
verified independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependencies)
- **[Story]**: user story (US1…US23)
- The description specifies the exact file path

## Path Conventions

- Workspace crates: `crates/<name>/src/`
- GUI application: `apps/freespace-gui/src/`
- Cross-crate tests: `tests/`
- Benchmarks: `benches/`
- Localization: `locales/`

---

## Phase 1: Setup (shared infrastructure)

**Purpose**: Initialize the workspace, tools and crate skeletons

- [x] T001 Create the Cargo workspace in `Cargo.toml` and pin the toolchain in `rust-toolchain.toml` (Rust 1.85+, edition 2024)
- [x] T002 [P] Configure workspace lints, build profiles and `.gitignore` in `Cargo.toml` and `.gitignore`
- [x] T003 [P] Create skeletons for the crates `crates/freespace-core`, `crates/freespace-platform`, `crates/freespace-scan`, `crates/freespace-ops`, `crates/freespace-analysis`, `crates/freespace-report`, `crates/freespace-cli` (Cargo.toml + src/lib.rs)
- [x] T004 [P] Create the GUI application skeleton `apps/freespace-gui` (eframe, Cargo.toml + src/main.rs)
- [x] T005 [P] Set up GitHub Actions CI for `fmt`, `clippy -D warnings`, `test` on windows-latest in `.github/workflows/ci.yml`
- [x] T006 [P] Create the localization files `locales/ru.toml` and `locales/en.toml` and wire up `rust-i18n`
- [x] T007 [P] Configure `tracing` logging and base error types in `crates/freespace-core/src/error.rs`
- [x] T008 [P] Create the `ru/`, `en/`, `docs/` directories and a basic `README.md` with build commands

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: Shared data model, platform layer and skeletons without which no story can start

**⚠️ CRITICAL**: No user story starts before this phase is complete

- [x] T009 Implement `SizeMetrics` (logical/on-disk/physical/ADS/savings) in `crates/freespace-core/src/size.rs`
- [x] T010 Implement the `FsNode` arena (SoA, `NodeId = u32`, names in a UTF-8 arena) in `crates/freespace-core/src/tree.rs`
- [x] T011 Implement `Volume`, `ScanSession` and status enums in `crates/freespace-core/src/model.rs`
- [x] T012 [P] Implement `Settings` (serde + TOML) and portable-mode storage in `crates/freespace-core/src/settings.rs`
- [x] T013 [P] Implement the path layer: verbatim paths, normalization, the 32,000-character limit in `crates/freespace-platform/src/paths.rs`
- [x] T014 [P] Implement WinAPI wrappers (RAII handle, error conversion) in `crates/freespace-platform/src/win.rs`
- [x] T015 [P] Implement volume enumeration (`GetLogicalDrives`, `GetVolumeInformationW`) in `crates/freespace-platform/src/volumes.rs`
- [x] T016 Implement the SQLite schema and migrations (journal, quarantine, hash cache) in `crates/freespace-ops/src/store.rs`
- [x] T017 Implement the `Operation`/`JournalEntry` model and state machine in `crates/freespace-ops/src/operation.rs`
- [x] T018 [P] Implement the operation preview and confirmation engine in `crates/freespace-ops/src/confirm.rs`
- [x] T019 [P] Implement quarantine (move, index, restore) in `crates/freespace-ops/src/quarantine.rs`
- [x] T020 [P] Implement the CLI skeleton: clap, global flags, JSON envelope, return codes in `crates/freespace-cli/src/main.rs`
- [x] T021 [P] Implement the GUI skeleton: window, navigation, background tasks and progress channel in `apps/freespace-gui/src/app.rs`
- [x] T022 [P] Create test FS fixtures and utilities in `tests/common/mod.rs`
- [x] T023 [P] Set up criterion benchmarks (scan, hashing, Treemap) in `benches/`
- [x] T024 Implement locale loading and runtime language switching in `crates/freespace-core/src/i18n.rs`
- [x] T025 Define the domain error codes `E_*` and their mapping to return codes in `crates/freespace-core/src/error.rs`

**Checkpoint**: the foundation is ready — stories can be implemented in parallel

---

## Phase 3: User Story 1 — Fast volume scan and size tree (P1) 🎯 MVP

**Goal**: volume analysis via MFT with fallback walk, tree construction, search, sorting and progress

**Independent Test**: scan a test volume, verify the totals against the reference, check search/sorting and the marking of inaccessible directories

- [x] T026 [P] [US1] Test MFT enumeration and totals on a reference volume (admin-gated) in `tests/scan_mft.rs`
- [x] T027 [P] [US1] Test the fallback walk and the marking of inaccessible directories in `tests/scan_walk.rs`
- [x] T028 [US1] Implement volume opening and `FSCTL_ENUM_USN_DATA` in `crates/freespace-platform/src/mft.rs`
- [x] T029 [US1] Implement `USN_RECORD_V2` parsing and tree construction in `crates/freespace-scan/src/mft_scan.rs`
- [x] T030 [US1] Implement incremental update via USN (`JournalID`, `NextUSN`) in `crates/freespace-scan/src/usn.rs`
- [x] T031 [US1] Implement the fallback walk (`jwalk` + `rayon`) and method detection in `crates/freespace-scan/src/walk.rs`
- [x] T032 [US1] Implement streaming delivery of results, progress and exclusion handling in `crates/freespace-scan/src/session.rs`
- [x] T033 [US1] Implement search/filters (`globset`) and sorting in `crates/freespace-core/src/query.rs`
- [x] T034 [US1] Implement the CLI commands `scan` and `tree` with JSON in `crates/freespace-cli/src/commands/scan.rs`
- [x] T035 [US1] Implement the virtualized tree in the GUI in `apps/freespace-gui/src/views/tree.rs`
- [x] T036 [US1] Integrate scan progress and cancellation in the GUI in `apps/freespace-gui/src/views/scan.rs`

**Checkpoint**: US1 is fully functional and can be demonstrated as the MVP

---

## Phase 4: User Story 2 — Interactive Treemap (P1)

**Goal**: a visual map with proportional areas, zoom, tooltips and coloring by type

**Independent Test**: verify area proportionality, zoom, tooltips and legend on a set with known sizes

- [x] T037 [P] [US2] Test squarified layout and area proportionality in `apps/freespace-gui/src/treemap/layout.rs`
- [x] T038 [US2] Implement the squarified treemap algorithm in `apps/freespace-gui/src/treemap/layout.rs`
- [x] T039 [US2] Implement rendering with levels of detail and a layout cache in `apps/freespace-gui/src/treemap/view.rs`
- [x] T040 [US2] Implement zoom, breadcrumbs and tooltips in `apps/freespace-gui/src/treemap/interaction.rs`
- [x] T041 [US2] Implement coloring by file type and the legend in `apps/freespace-gui/src/treemap/colors.rs`
- [x] T042 [P] [US2] Benchmark rendering on 1M nodes in `benches/treemap.rs`

**Checkpoint**: US2 works on US1 data independently of the other stories

---

## Phase 5: User Story 3 — Unlocking locked files (P1)

**Goal**: identify blocking processes, soft/hard lock release, delete on reboot

**Independent Test**: hold a file with a test process, find the lock, release it and verify the file is accessible

- [x] T043 [P] [US3] Test the lock list via Restart Manager in `tests/unlock.rs`
- [x] T044 [US3] Implement `RmStartSession`/`RmGetList` in `crates/freespace-platform/src/locks.rs`
- [x] T045 [US3] Implement soft/hard release and delete on reboot in `crates/freespace-ops/src/unlock.rs`
- [x] T046 [US3] Implement CLI `unlock` (list/soft/force) in `crates/freespace-cli/src/commands/unlock.rs`
- [x] T047 [US3] Implement the locks dialog in the GUI in `apps/freespace-gui/src/views/unlock.rs`

---

## Phase 6: User Story 4 — Top files and grouping by type (P1)

**Goal**: a flat top-N file list and aggregation of used space by extension/category

**Independent Test**: verify the top list at different N and the per-extension totals against the reference

- [x] T048 [P] [US4] Test top-N and type aggregation in `tests/top_types.rs`
- [x] T049 [US4] Implement top-N (partial sort) in `crates/freespace-analysis/src/top.rs`
- [x] T050 [US4] Implement grouping by extension and category in `crates/freespace-analysis/src/types.rs`
- [x] T051 [US4] Implement CLI `top` and `types` in `crates/freespace-cli/src/commands/top.rs`
- [x] T052 [US4] Implement the "Top" and "Types" tabs in the GUI in `apps/freespace-gui/src/views/top_types.rs`

**Checkpoint**: the MVP (US1–US4) is ready for demonstration

---

## Phase 7: User Story 5 — Duplicate search (P2)

**Goal**: groups of exact matches, copy selection rules and safe deletion

**Independent Test**: a set with known duplicates and "false" files of identical size — verify the absence of false matches and the preservation of the kept copy

- [x] T053 [P] [US5] Test the hashing pipeline and the absence of false matches in `tests/duplicates.rs`
- [x] T054 [US5] Implement two-stage hashing and byte-by-byte verification in `crates/freespace-analysis/src/hash.rs`
- [x] T055 [US5] Implement the hash cache in SQLite in `crates/freespace-analysis/src/hash_cache.rs`
- [x] T056 [US5] Implement duplicate grouping and keep rules in `crates/freespace-analysis/src/duplicates.rs`
- [x] T057 [US5] Implement CLI `duplicates` in `crates/freespace-cli/src/commands/duplicates.rs`
- [x] T058 [US5] Implement the duplicates screen in the GUI in `apps/freespace-gui/src/views/duplicates.rs`

---

## Phase 8: User Story 6 — Shredder and delete on reboot (P2)

**Goal**: irreversible overwrite, SSD/TRIM warnings and the system deletion queue

**Independent Test**: destroy a test set and verify that recovery with an independent tool is impossible

- [x] T059 [P] [US6] Test overwrite/deletion and SSD warnings in `tests/shred.rs`
- [x] T060 [US6] Implement the shredder (passes, patterns, write-through, rename/truncate/delete) in `crates/freespace-ops/src/shred.rs`
- [x] T061 [US6] Implement SSD and TRIM detection in `crates/freespace-platform/src/trim.rs`
- [x] T062 [US6] Implement CLI `shred` and the `--yes` check in `crates/freespace-cli/src/commands/shred.rs`
- [x] T063 [US6] Implement the shredder UI with warning and confirmation in `apps/freespace-gui/src/views/shred.rs`

---

## Phase 9: User Story 7 — Stale files, empty folders and recommendations (P2)

**Goal**: date filters, temporary data categories, empty directories and cleanup recommendations

**Independent Test**: a set with known dates and empty directories — verify filter accuracy and list completeness

- [ ] T064 [P] [US7] Test date filters and empty-directory search in `tests/cleanup.rs`
- [ ] T065 [US7] Implement the staleness filter based on the maximum of dates with a confidence flag in `crates/freespace-analysis/src/cleanup.rs`
- [ ] T066 [US7] Implement temp/cache/log/backup categories and recommendations in `crates/freespace-analysis/src/recommend.rs`
- [ ] T067 [US7] Implement empty-directory search in `crates/freespace-scan/src/empty_dirs.rs`
- [ ] T068 [US7] Implement CLI `cleanup suggest` and `cleanup empty-dirs` in `crates/freespace-cli/src/commands/cleanup.rs`
- [ ] T069 [US7] Implement the recommendations and empty folders screen in the GUI in `apps/freespace-gui/src/views/cleanup.rs`

---

## Phase 10: User Story 8 — Snapshots and comparison (P2)

**Goal**: saving `.fssnap` snapshots and comparing changes by directory and by file

**Independent Test**: two snapshots with known differences — verify the completeness and sorting of the report

- [ ] T070 [P] [US8] Test snapshot round-trip and comparison in `tests/snapshot.rs`
- [ ] T071 [US8] Implement the `.fssnap` format (CBOR + zstd, columns, checksum) in `crates/freespace-report/src/snapshot.rs`
- [ ] T072 [US8] Implement snapshot comparison and delta in `crates/freespace-report/src/diff.rs`
- [ ] T073 [US8] Implement CLI `snapshot create` and `snapshot diff` in `crates/freespace-cli/src/commands/snapshot.rs`
- [ ] T074 [US8] Implement the snapshots and comparison screen in the GUI in `apps/freespace-gui/src/views/snapshots.rs`

---

## Phase 11: User Story 9 — Safe operations, journal and undo (P2)

**Goal**: a single confirmation flow with deletion method selection, journal, quarantine and undo

**Independent Test**: perform a deletion, restore the object from quarantine, check the journal and undo

- [ ] T075 [P] [US9] Test deletion mode selection, quarantine and undo in `tests/safe_ops.rs`
- [ ] T076 [US9] Implement the confirmation dialog with the "Recycle Bin/Quarantine/Permanent" selection in `apps/freespace-gui/src/views/confirm.rs`
- [ ] T077 [US9] Implement deletion to the Recycle Bin via `IFileOperation` in `crates/freespace-platform/src/recycle.rs`
- [ ] T078 [US9] Implement free space checking and the insufficient-space scenario in `crates/freespace-ops/src/delete.rs`
- [ ] T079 [US9] Implement the operation journal, undo and restore in `crates/freespace-ops/src/undo.rs` and `apps/freespace-gui/src/views/journal.rs`
- [ ] T080 [US9] Implement CLI `delete --mode` with confirmation checks in `crates/freespace-cli/src/commands/delete.rs`
- [ ] T081 [US9] Implement system path protection, prevention of escaping the selected scope, and additional confirmation in `crates/freespace-ops/src/guarded.rs`

---

## Phase 12: User Story 10 — Accurate space accounting: links, compression, sparse, ADS (P3)

**Goal**: correct physical and logical size accounting for links, compression, sparseness and streams

**Independent Test**: a set with hard links, junctions, compressed/sparse files and ADS — verify against the reference

- [ ] T082 [P] [US10] Test deduplication of hard links, junctions, sparse/compressed and ADS in `tests/accounting.rs`
- [ ] T083 [US10] Implement reparse point and link detection in `crates/freespace-platform/src/reparse.rs`
- [ ] T084 [US10] Implement the exact size on disk (`GetCompressedFileSizeW`, allocated ranges) in `crates/freespace-platform/src/allocated.rs`
- [ ] T085 [US10] Implement physical size deduplication by FRN in `crates/freespace-analysis/src/accounting.rs`
- [ ] T086 [US10] Implement ADS enumeration (`FindFirstStreamW`) in `crates/freespace-platform/src/ads.rs`
- [ ] T087 [US10] Implement metric and ADS display in the GUI in `apps/freespace-gui/src/views/size_details.rs`
- [ ] T088 [US10] Implement CLI flags for ADS accounting and size mode in `crates/freespace-cli/src/commands/scan.rs`

---

## Phase 13: User Story 11 — Long path file manager (P3)

**Goal**: operations on paths longer than 260 characters without a system policy

**Independent Test**: create paths of 261/1000/32000 characters and perform rename/move/copy/delete

- [ ] T089 [P] [US11] Test operations on long paths and limit values in `tests/long_paths.rs`
- [ ] T090 [US11] Implement the long path file manager in `crates/freespace-ops/src/long_path_ops.rs`
- [ ] T091 [US11] Implement path limit validation and clear errors before modifying data in `crates/freespace-platform/src/paths.rs`
- [ ] T092 [US11] Implement the long path file manager UI in `apps/freespace-gui/src/views/long_paths.rs`

---

## Phase 14: User Story 12 — Bulk rename (P3)

**Goal**: RegEx/template rules, preview, conflict resolution and rollback

**Independent Test**: apply a rule to 1000 files, verify the preview, conflicts, result and rollback

- [ ] T093 [P] [US12] Test rename rules, conflicts and rollback in `tests/rename.rs`
- [ ] T094 [US12] Implement the rename rule engine in `crates/freespace-ops/src/rename.rs`
- [ ] T095 [US12] Implement preview, conflict detection and the rollback journal in `crates/freespace-ops/src/rename_plan.rs`
- [ ] T096 [US12] Implement CLI `rename` in `crates/freespace-cli/src/commands/rename.rs`
- [ ] T097 [US12] Implement the bulk rename UI in `apps/freespace-gui/src/views/rename.rs`

---

## Phase 15: User Story 13 — NTFS attributes and permissions (P3)

**Goal**: batch changes of attributes, owner and permissions with an error report

**Independent Test**: change attributes, owner and permissions on a test set, verify the report and privilege elevation

- [ ] T098 [P] [US13] Test attribute/owner/permission changes (admin-gated) in `tests/permissions.rs`
- [ ] T099 [US13] Implement setting and clearing attributes in `crates/freespace-ops/src/attributes.rs`
- [ ] T100 [US13] Implement ACL and owner handling (privileges, `SetNamedSecurityInfoW`) in `crates/freespace-platform/src/security.rs`
- [ ] T101 [US13] Implement batch permission changes and the error report in `crates/freespace-ops/src/acl.rs`
- [ ] T102 [US13] Implement CLI `attrs` and `acl` in `crates/freespace-cli/src/commands/attrs.rs`
- [ ] T103 [US13] Implement the permissions and attributes UI in `apps/freespace-gui/src/views/permissions.rs`

---

## Phase 16: User Story 14 — Metadata and advanced filtering (P3)

**Goal**: reading EXIF, audio/video and PE versions, filtering and sorting by metadata

**Independent Test**: prepare files with known metadata and verify display, filters and resilience to corruption

- [ ] T104 [P] [US14] Test reading EXIF/audio/video/PE and corrupted files in `tests/metadata.rs`
- [ ] T105 [US14] Implement EXIF reading in `crates/freespace-analysis/src/metadata/exif.rs`
- [ ] T106 [US14] Implement audio/video metadata reading in `crates/freespace-analysis/src/metadata/media.rs`
- [ ] T107 [US14] Implement PE version reading in `crates/freespace-analysis/src/metadata/pe.rs`
- [ ] T108 [US14] Implement background extraction and the metadata cache in `crates/freespace-analysis/src/metadata/mod.rs`
- [ ] T109 [US14] Implement metadata columns and filters in the GUI in `apps/freespace-gui/src/views/metadata.rs`
- [ ] T110 [US14] Implement CLI metadata filters in `crates/freespace-cli/src/commands/filter.rs` and the corresponding contract

---

## Phase 17: User Story 15 — Analysis by owners (P3)

**Goal**: a summary by owner and tree filtering by owner

**Independent Test**: a set with files owned by different owners — verify the totals, the filter and the "unknown" category

- [ ] T111 [P] [US15] Test aggregation by owners in `tests/owners.rs`
- [ ] T112 [US15] Implement SID interning and the owner summary in `crates/freespace-analysis/src/owners.rs`
- [ ] T113 [US15] Implement tree filtering by owner in `crates/freespace-core/src/query.rs`
- [ ] T114 [US15] Implement the owners report and UI in `apps/freespace-gui/src/views/owners.rs` and `crates/freespace-report/src/owners.rs`

---

## Phase 18: User Story 16 — Viewing archives without extraction (P3)

**Goal**: contents of ZIP/7z/TAR/ISO/RAR with sizes and list export

**Independent Test**: open archives of known formats, verify the list, sorting, passwords and corruption

- [ ] T115 [P] [US16] Test archive reading and password/corruption handling in `tests/archives.rs`
- [ ] T116 [US16] Implement ZIP/7z/TAR/ISO readers in `crates/freespace-analysis/src/archives/mod.rs`
- [ ] T117 [US16] Implement optional RAR with a library presence check in `crates/freespace-analysis/src/archives/rar.rs`
- [ ] T118 [US16] Implement resource limits and archive error handling in `crates/freespace-analysis/src/archives/limits.rs`
- [ ] T119 [US16] Implement CLI `archives` and the archive viewer UI in `crates/freespace-cli/src/commands/archives.rs` and `apps/freespace-gui/src/views/archives.rs`

---

## Phase 19: User Story 17 — Report export (P4)

**Goal**: export to CSV/XML/HTML/XLSX with content selection and streaming writes

**Independent Test**: export a single result to all formats and verify the totals and opening in the target tools

- [ ] T120 [P] [US17] Test CSV/XML/HTML/XLSX schemas and total consistency in `tests/export.rs`
- [ ] T121 [US17] Implement streaming CSV export in `crates/freespace-report/src/csv.rs`
- [ ] T122 [US17] Implement XML export (`quick-xml`) in `crates/freespace-report/src/xml.rs`
- [ ] T123 [US17] Implement HTML export with escaping in `crates/freespace-report/src/html.rs`
- [ ] T124 [US17] Implement XLSX export (`rust_xlsxwriter`) in `crates/freespace-report/src/xlsx.rs`
- [ ] T125 [US17] Implement CLI `export` and the export dialog in the GUI in `crates/freespace-cli/src/commands/export.rs` and `apps/freespace-gui/src/views/export.rs`

---

## Phase 20: User Story 18 — CLI and scheduled runs (P4)

**Goal**: full CLI parity, JSON schemas, locking of concurrent runs and Task Scheduler job export

**Independent Test**: run the main commands non-interactively, verify JSON, return codes and the XML job

- [ ] T126 [P] [US18] Test JSON schemas and return codes of all commands in `tests/cli_contract.rs`
- [ ] T127 [US18] Implement Task Scheduler job (XML) export in `crates/freespace-cli/src/commands/schedule.rs`
- [ ] T128 [US18] Implement locking and queueing of concurrent runs in `crates/freespace-cli/src/lock.rs`
- [ ] T129 [US18] Document the CLI contract and return codes in `docs/cli.md`
- [ ] T130 [US18] Implement automation profiles in `crates/freespace-ops/src/automation.rs`

---

## Phase 21: User Story 19 — Context menu integration (P4)

**Goal**: analysis and unlock entries in Explorer with install and uninstall

**Independent Test**: install the integration, verify the entries for file/folder/drive, uninstall the integration

- [ ] T131 [P] [US19] Test installation and removal of registry entries in HKCU in `tests/context_menu.rs`
- [ ] T132 [US19] Implement context menu install/uninstall (`winreg`) in `crates/freespace-platform/src/context_menu.rs`
- [ ] T133 [US19] Implement correct path passing (Unicode, spaces, long paths) in `crates/freespace-cli/src/main.rs`
- [ ] T134 [US19] Implement CLI `context-menu install/uninstall` in `crates/freespace-cli/src/commands/context_menu.rs`

---

## Phase 22: User Story 20 — Disk write monitoring (P4)

**Goal**: write events with process, path and speed, thresholds and history

**Independent Test**: run a test write of a known volume and verify the event, speed and notification

- [ ] T135 [P] [US20] Test the ETW parser and the fallback to I/O counters in `tests/monitor.rs`
- [ ] T136 [US20] Implement the `Microsoft-Windows-Kernel-File` ETW session (`ferrisetw`) in `crates/freespace-platform/src/etw.rs`
- [ ] T137 [US20] Implement the `GetProcessIoCounters` fallback and the USN tail in `crates/freespace-scan/src/monitor_fallback.rs`
- [ ] T138 [US20] Implement thresholds, process and path exclusions, history and notifications in `crates/freespace-analysis/src/monitor.rs`
- [ ] T139 [US20] Implement CLI `monitor` (JSON Lines) and the monitoring UI with notification-area support in `crates/freespace-cli/src/commands/monitor.rs` and `apps/freespace-gui/src/views/monitor.rs`

---

## Phase 23: User Story 21 — System directories and privilege elevation (P4)

**Goal**: UAC elevation on demand, access to protected directories and graceful degradation

**Independent Test**: verify operation without and with privileges, access to `System Volume Information`, and refusal of elevation

- [ ] T140 [P] [US21] Test privilege elevation and access to system directories (admin) in `tests/elevation.rs`
- [ ] T141 [US21] Implement UAC elevation on demand (`ShellExecute` runas) in `crates/freespace-platform/src/elevation.rs`
- [ ] T142 [US21] Implement access to protected directories and marking of inaccessibility in `crates/freespace-scan/src/access.rs`
- [ ] T143 [US21] Implement the privilege elevation request UI in `apps/freespace-gui/src/views/elevation.rs`

---

## Phase 24: User Story 22 — All-drives dashboard (P4)

**Goal**: a summary by volume, quick scan launch and dynamics across snapshots

**Independent Test**: verify the display of all volumes, the correctness of the numbers and navigation to analysis

- [ ] T144 [P] [US22] Test the volume summary and snapshot dynamics in `tests/dashboard.rs`
- [ ] T145 [US22] Implement types and collection of the volume summary in `crates/freespace-core/src/dashboard.rs` and `crates/freespace-platform/src/volumes.rs`
- [ ] T146 [US22] Implement the dashboard screen and quick scan launch in `apps/freespace-gui/src/views/dashboard.rs`
- [ ] T147 [US22] Implement CLI `drives` in `crates/freespace-cli/src/commands/drives.rs`

---

## Phase 25: User Story 23 — Settings, localization and portable mode (P4)

**Goal**: settings, complete RU/EN translations, portable data storage

**Independent Test**: switch language and theme, set exclusions, run the portable build and verify that nothing is written to the profile

- [ ] T148 [P] [US23] Test language switching, settings and portable mode in `tests/settings.rs`
- [ ] T149 [US23] Fill in the complete locales `locales/ru.toml` and `locales/en.toml`
- [ ] T150 [US23] Implement the settings screen in the GUI in `apps/freespace-gui/src/views/settings.rs`
- [ ] T151 [US23] Implement portable mode and the settings storage fallback in `crates/freespace-core/src/settings.rs`
- [ ] T152 [US23] Implement CLI `config get/set` in `crates/freespace-cli/src/commands/config.rs`
- [ ] T153 [US23] Implement a regression test ensuring no network calls or telemetry in `tests/privacy.rs`

---

## Phase 26: Polish & Cross-Cutting Concerns

**Purpose**: performance, documentation, packaging and final validation

- [ ] T154 [P] Optimize the MFT scan based on benchmark results in `crates/freespace-scan/src/mft_scan.rs`
- [ ] T155 [P] Optimize tree and arena memory in `crates/freespace-core/src/tree.rs`
- [ ] T156 [P] Run `cargo fmt --check` and `cargo clippy -D warnings`, eliminate warnings
- [ ] T157 [P] Prepare RU/EN user documentation in `docs/`, `ru/`, `en/`
- [ ] T158 [P] Prepare the portable ZIP and the Inno Setup installer in `installer/freespace.iss`
- [ ] T159 [P] Create `CHANGELOG.md` and pin version 1.0.0
- [ ] T160 Run all `quickstart.md` scenarios on Windows 10 and Windows 11 and record the results in `docs/release-validation.md`
- [ ] T161 Synchronize the documentation mirrors in `ru/` and `en/` and verify compliance with the constitution

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: no dependencies, starts immediately
- **Phase 2 (Foundational)**: depends on Phase 1 and blocks all stories
- **US1–US23**: depend on Phase 2; US1 is the data source for most stories
- **Polish (Phase 26)**: after the selected stories are complete

### User Story Dependencies

- **US1 (P1)**: after Phase 2 — does not depend on other stories
- **US2 (P1)**: after US1 (tree data)
- **US3 (P1)**: after Phase 2 (does not depend on US1)
- **US4 (P1)**: after US1
- **US5 (P2)**: after US1 and US9 (safe deletion); search is independent
- **US6 (P2)**: after US9 (confirmations and journal)
- **US7 (P2)**: after US1
- **US8 (P2)**: after US1
- **US9 (P2)**: after Phase 2; the base operations are available to other stories
- **US10 (P3)**: after US1
- **US11 (P3)**: after Phase 2 (path layer)
- **US12 (P3)**: after US9 (undo)
- **US13 (P3)**: after Phase 2; depends on US21 for privilege elevation
- **US14 (P3)**: after US1
- **US15 (P3)**: after US1
- **US16 (P3)**: after US1
- **US17 (P4)**: after US1; uses data from US4, US5, US15 as they become ready
- **US18 (P4)**: after US1 and US17 (export in the schedule)
- **US19 (P4)**: after Phase 2
- **US20 (P4)**: after Phase 2
- **US21 (P4)**: after Phase 2; blocks the administrative parts of US13
- **US22 (P4)**: after Phase 2 and US8 (dynamics)
- **US23 (P4)**: after Phase 2

### Within Each User Story

- Tests (if included) are written and fail before implementation
- Models → services → CLI/UI → integration
- A story is completed by independent verification

### Parallel Opportunities

- All `[P]` tasks in Phase 1 and Phase 2 run in parallel
- After Phase 2, the P1 stories (US1–US4) can proceed in parallel, taking into account the dependency of US2/US4 on US1
- P3 and P4 stories can be executed in parallel once their dependencies are ready
- Within stories, tasks with different files (models, tests, CLI, UI) are parallel

---

## Parallel Example: US1

```text
Task: "T026 [P] [US1] Test MFT enumeration and totals in tests/scan_mft.rs"
Task: "T027 [P] [US1] Test the fallback walk in tests/scan_walk.rs"
```

## Parallel Example: US5

```text
Task: "T053 [P] [US5] Test the hashing pipeline in tests/duplicates.rs"
Task: "T054 [US5] Two-stage hashing in crates/freespace-analysis/src/hash.rs"
Task: "T055 [US5] Hash cache in SQLite in crates/freespace-analysis/src/hash_cache.rs"
```

## Parallel Example: US17

```text
Task: "T121 [US17] CSV export in crates/freespace-report/src/csv.rs"
Task: "T122 [US17] XML export in crates/freespace-report/src/xml.rs"
Task: "T123 [US17] HTML export in crates/freespace-report/src/html.rs"
Task: "T124 [US17] XLSX export in crates/freespace-report/src/xlsx.rs"
```

## Parallel Example: P3 stories

```text
Task: "US10 — accounting for links, compression, sparse and ADS"
Task: "US11 — long path file manager"
Task: "US14 — metadata"
Task: "US16 — archive viewing"
```

---

## Implementation Strategy

### MVP First (P1)

1. Phase 1: Setup
2. Phase 2: Foundational
3. US1 → independent verification
4. US2, US3, US4 → independent verifications
5. **STOP and VALIDATE**: the MVP demonstrates scanning, tree, Treemap, unlocking, top and types

### Incremental Delivery

1. MVP (P1) → demonstration
2. P2: US5–US9 → cleanup, duplicates, shredder, snapshots, safe operations
3. P3: US10–US16 → advanced file system, metadata, archives
4. P4: US17–US23 → reports, CLI/schedule, context menu, monitoring, permissions, dashboard, settings
5. Polish → release 1.0.0

### Parallel Team Strategy

- After Phase 2: developer A — US1; developer B — US3; developer C — US18/US19 (CLI/integrations)
- After US1: US2, US4, US5, US8, US10, US14, US15, US16 are distributed in parallel
- US9 (safety) is implemented before US5/US6/US12 so that cleanup goes through the shared layer right away

---

## Notes

- `[P]` tasks — different files, no dependencies
- The `[Story]` label provides traceability to the specification
- Tests for critical modules are mandatory per the constitution; UI tests are optional
- Commits — Conventional Commits, after each task or logical group
- Stop at checkpoints for independent verification of stories
- Avoid tasks without concrete paths and dependencies that break story independence
