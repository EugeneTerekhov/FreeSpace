# Research: FreeSpace — technical decisions

**Feature**: `001-freespace-disk-analyzer` | **Date**: 2026-11-15

> **2.0 note (2026-11-20)**: the 1.x decisions about egui/eframe, Inno Setup and quarantine are
> historical. Tauri 2 + React (ADR 0001), portable archives without an installer and the system
> trash instead of quarantine are current; see
> [003/plan.md](../../specs/003-cross-platform-portable/plan.md).

All key technical unknowns from the plan are resolved below in the format "decision — rationale —
alternatives".

## 1. UI framework

- **Decision**: `egui`/`eframe` with `egui_extras` for tables and custom Treemap rendering.
- **Rationale**: native immediate-mode UI without a WebView, full control over rectangle
  rendering and virtualization, simple integration with background threads via channels,
  minimal overhead, a single binary.
- **Alternatives considered**: Tauri (WebView, HTML/JS frontend — harder to ensure
  Treemap performance on millions of nodes, extra latency and distribution size);
  Slint (smaller ecosystem for custom rendering); WinUI 3 (heavy integration with Rust, C++/C#).

## 2. Fast NTFS scanning

- **Decision**: open the volume via `CreateFileW("\\.\X:")` and enumerate MFT records via
  `FSCTL_ENUM_USN_DATA` (`USN_RECORD_V2`), with deduplication by File Reference Number (FRN).
  If benchmarks do not reach the 8 TB / 60 s goal, an optimized raw
  MFT record parser is enabled behind a feature flag.
- **Rationale**: documented and stable interface, provides name, parent, FRN, attributes,
  sizes, and timestamps in a single pass; deduplication by FRN correctly accounts for hard links.
- **Alternatives considered**: custom `$MFT` parser (maximum speed, but high
  complexity and risk of incompatibility); `FindFirstFileExW` (slow on millions of files);
  third-party crates (`ntfs`, `usn-journal`) — used as reference implementations, but
  the critical path is written on the `windows` crate.

## 3. Fallback scanning of non-NTFS and operation without permissions

- **Decision**: parallel tree traversal on `jwalk` + `rayon` with bounded parallelism and
  access error handling; the scan method is stored in the session and shown in the UI/report.
- **Rationale**: `jwalk` efficiently uses multiple cores and correctly handles long
  paths; predictable behavior without administrative permissions.
- **Alternatives considered**: `walkdir` (simpler but slower); custom walker (more code
  and tests).

## 4. Incremental update via USN

- **Decision**: saving `USN_JOURNAL_DATA_V2` (JournalID, NextUSN) in the session; on repeated
  scanning — `FSCTL_READ_USN_JOURNAL` and applying records to the tree. When JournalID changes or
  the journal overflows — a full rescan with notification.
- **Rationale**: the only reliable mechanism for NTFS delta changes; dramatically speeds up repeated
  analysis and monitoring.
- **Alternatives considered**: repeated full traversal (slow); `ReadDirectoryChangesW`
  (works only in real time, not for repeated analysis).

## 5. Tree storage model

- **Decision**: structure-of-arrays (SoA) in an arena: `NodeId = u32`, parent/child/sibling indices,
  names in a shared UTF-8 arena with offsets, `SizeMetrics` (logical/on disk/physical),
  FRN, attributes, timestamps. Estimate: ~80–100 bytes per node, up to 4 GB for 10 million nodes.
- **Rationale**: no per-node allocations, cache-friendliness, fast traversal and sorting;
  fits within the memory budget.
- **Alternatives considered**: `Rc<RefCell<Node>>` (fragmentation and overhead); flat
  path strings (memory-expensive); external DB (slow queries during interactive navigation).

## 6. Treemap

- **Decision**: squarified treemap algorithm (Bruls et al.), custom rendering in egui,
  levels of detail: nodes below the pixel threshold are aggregated; layout cache per zoom level.
- **Rationale**: optimal rectangle readability, predictable performance,
  no external dependencies.
- **Alternatives considered**: slice-and-dice (poor proportions); cushion treemap (more complex);
  ready-made WebGL libraries (incompatible with egui).

## 7. Duplicate search

- **Decision**: pipeline "size → partial hash (first and last 64 KiB) → full
  SHA-256/MD5 hash → byte-by-byte comparison". Hash cache in SQLite: the `hash_cache`
  table keyed by `(VolumeSerial, FRN, Size, MTime)` and the `analysis_hash_cache` table
  keyed by `(Path, Size, MTime)` for files without an FRN; `rayon` parallelism; large
  files are streamed.
- **Rationale**: eliminates false matches, minimizes disk reads, speeds up repeated
  checks.
- **Alternatives considered**: comparison by name/size only (false matches); full
  hash only (slow); BLAKE3 (faster, but users expect MD5/SHA-256).

## 8. Shredder

- **Decision**: overwriting with `FILE_FLAG_WRITE_THROUGH`, "zeros" and "random data"
  patterns (cryptographically secure PRNG), configurable number of passes; after overwriting —
  renaming to a random name, truncation and deletion; on SSD — `FSCTL_FILE_LEVEL_TRIM` and a warning about
  wear leveling with explicit confirmation.
- **Rationale**: meets the specification requirements and honestly reflects SSD limitations;
  requires no driver.
- **Alternatives considered**: a single pass of zeros (faster, weaker); crypto-erase (not applicable to
  arbitrary files); external utilities (violate self-containment).

## 9. Detecting locks and unlocking

- **Decision**: Restart Manager API (`RmStartSession`/`RmRegisterResources`/`RmGetList`) for the
  process list; "soft" release — `RmShutdown` (graceful close), "hard" — terminating
  the process after confirmation. For driver-level locks — `MoveFileExW` with
  `MOVEFILE_DELAY_UNTIL_REBOOT`.
- **Rationale**: official mechanism without a kernel driver; closing other processes' handles from
  user-mode is impossible, and terminating the process is a transparent and safe alternative.
- **Alternatives considered**: unlocker driver (complexity, signing, risks); enumerating
  handles via `NtQuerySystemInformation` (informative, but does not allow closing another process's
  handle).

## 10. Long paths

- **Decision**: all internal operations — via verbatim paths (`\\?\` and `\\?\UNC\`), a custom
  normalization layer; checking the 32,000-character limit before the operation; full paths in the journal.
- **Rationale**: independent of the `LongPathsEnabled` policy; covers requirements FR-051…FR-053.
- **Alternatives considered**: requiring the policy to be enabled (unacceptable for the user);
  `dunce` normalization (useful, but insufficient for guarantees).

## 11. NTFS permissions and owners

- **Decision**: `GetNamedSecurityInfoW`/`SetNamedSecurityInfoW`, `LookupAccountSidW`,
  enabling the `SeRestorePrivilege`, `SeTakeOwnershipPrivilege`, `SeSecurityPrivilege` privileges
  via `AdjustTokenPrivileges`; ACE preview before applying.
- **Rationale**: standard Security APIs; correct handling of inheritance and auditing.
- **Alternatives considered**: `icacls`/PowerShell (external processes, slow, unsafe);
  direct descriptor writing without privileges (impossible).

## 12. Links, compression, sparse and ADS

- **Decision**: hard links — deduplication by FRN; junction/symlink — detection via
  `FILE_ATTRIBUTE_REPARSE_POINT` and `FSCTL_GET_REPARSE_POINT`; compression/sparse — `GetCompressedFileSizeW`
  and `FSCTL_QUERY_ALLOCATED_RANGES` for the exact size on disk (for other files —
  rounding to the cluster); ADS — `FindFirstStreamW`/`FindNextStreamW` on demand and in
  deep analysis mode.
- **Rationale**: accuracy where it matters, without mass expensive queries to every file.
- **Alternatives considered**: `GetCompressedFileSizeW` for all files (too slow);
  ignoring ADS (understated sizes).

## 13. Snapshot format

- **Decision**: `.fssnap` — CBOR (`ciborium`) with a schema version, columnar node structure,
  `zstd` compression; volume metadata, scan method, settings; comparison — streaming merge of
  sorted paths.
- **Rationale**: compactness, serialization speed, versioning and streaming comparison without
  loading the entire file into memory.
- **Alternatives considered**: JSON (large size, slow); SQLite file (more difficult to update
  schema versions); protobuf (requires code generation, redundant).

## 14. Reading archives

- **Decision**: ZIP — `zip`; 7z — `sevenz-rust`; TAR/GZ — `tar` + `flate2`; ISO — `iso9660`;
  RAR — optional `unrar` with an external `unrar.dll` and an explicit label in the interface.
- **Rationale**: coverage of common formats without external processes; RAR — due to
  licensing restrictions, only optionally.
- **Alternatives considered**: invoking the 7-Zip CLI (dependency on installed software, slow).

## 15. Metadata

- **Decision**: `kamadak-exif` for EXIF; `lofty` for audio (bitrate, duration); `mp4parse`
  and Matroska header parsing for video (resolution, codec, duration); `pelite` for
  `VS_VERSIONINFO` of executable files; all reads — in the background and with a timeout.
- **Rationale**: mature crates, no external binaries, resilience to corrupted files.
- **Alternatives considered**: `ffprobe` (external dependency); `exiftool` (external process).

## 16. Disk write monitoring

- **Decision**: real-time ETW session `Microsoft-Windows-Kernel-File` via `ferrisetw`
  (write events with PID and path); without permissions — fallback to polling
  `GetProcessIoCounters` (per-process throughput without paths) and the USN journal tail (paths without PID).
- **Rationale**: ETW provides precise "process → path → amount" attribution; the fallback preserves
  functionality without administrative permissions.
- **Alternatives considered**: USN only (no PID); minifilter driver (redundant, signing);
  WMI (`Win32_PerfFormattedData_PerfProc_Process` — without paths).

## 17. Local storage

- **Decision**: SQLite (`rusqlite`, bundled) for the operations journal, quarantine index and hash
  cache; settings — TOML; everything lives in `<exe_dir>\data`. `%APPDATA%`, `%LOCALAPPDATA%`,
  `%ProgramData%` and temporary directories are not used. Without write access settings stay in
  memory and the operation reports an error. Schema migrations are built-in and versioned.
- **Rationale**: journal transactionality, crash recovery, simple migrations and predictable
  removal of every trace with the application folder; avoiding the profile removes hidden records
  and simplifies portability.
- **Alternatives considered**: `%APPDATA%`/`%LOCALAPPDATA%` (forbidden by the full opt-out
  requirement); `%ProgramData%` (needs rights and is a system location); JSON journal files (risk
  of corruption); external DBMS (redundant).

## 18. Context menu integration

- **Decision**: registry entries `HKCU\Software\Classes\*\shell`, `Directory\shell`, `Drive\shell`
  (and `HKLM` when elevated) via `winreg`, with `MUIVerb`, `Icon` and a launch command. Entries are
  disabled by default and controlled by `context_menu_enabled`; the portable build compiles without
  the `context-menu` feature and without the `winreg` dependency.
- **Rationale**: works on Windows 10 and 11 (in Windows 11 — in the extended "Show
  more options" menu), requires no COM server or installer; disabled by default avoids unexpected
  registry writes.
- **Alternatives considered**: `IExplorerCommand` COM + MSIX (native Windows 11 menu, but
  significantly more complex); always-on integration (violates the "empty by default"
  requirement); third-party shells (unacceptable).

## 19. Localization and settings

- **Decision**: `rust-i18n` with `locales/ru.toml` and `locales/en.toml` files, language switching
  at runtime without restart; settings — `serde` + TOML, hot application where possible.
- **Rationale**: simple format, pluralization support, no heavy dependencies.
- **Alternatives considered**: Fluent (more powerful, but redundant for two languages); gettext (more complex).

## 20. Testing file operations

- **Decision**: unit tests on temporary directories (`tempfile`/`assert_fs`); CLI integration tests
  via `assert_cmd`; MFT/permissions/locks tests — only with `FREESPACE_ADMIN_TESTS=1` on
  a separate test NTFS volume (VHD); `criterion` benchmarks for scanning, hashing and
  Treemap.
- **Rationale**: safety of real data, reproducibility in CI (windows-latest provides
  an administrator), measurability of the performance budget.
- **Alternatives considered**: tests on the developer's primary disk (dangerous); WinAPI mocks
  (do not verify real file system semantics).

## 21. Packaging and the portable build

- **Decision**: the portable ZIP is built with `--no-default-features` (no context-menu component)
  and contains a `data` directory; the Inno Setup installer registers no menu entries and on
  uninstall calls `context-menu uninstall` for both scopes and removes `{app}\data`. Code signing
  is a separate release task.
- **Rationale**: portability is important for system administrators; avoiding AppData and disabling
  the menu by default make behaviour predictable and uninstall complete.
- **Alternatives considered**: `portable.flag` (no longer needed, the storage mode is singular);
  installing to `%LOCALAPPDATA%\Programs` (forbidden by the requirement); MSIX (restricts registry
  integrations); Chocolatey/Winget (additional channels, do not replace ZIP).

## 22. Report export

- **Decision**: CSV — `csv`; XML — `quick-xml`; HTML — a custom template with escaping;
  Excel — `rust_xlsxwriter` (XLSX); streaming writes for large reports; documented
  schemas in `contracts/`.
- **Rationale**: native crates without external dependencies, correct encodings, support for
  large volumes.
- **Alternatives considered**: XLS generation (legacy format); HTML via a template engine
  (redundant).

## 23. Identifiers and supporting dependencies

- **Decision**: operations, sessions and quarantine items are identified by `uuid` v4; a volume
  identifier is derived deterministically (UUID v5) from the normalized mount point. The `uuid`
  crate (`v4`, `v5`, `serde`) is added to `freespace-core`; other modules use the
  `freespace_core::Uuid` re-export.
- **Rationale**: stable identifiers are required by the journal, quarantine and undo; UUID v5 keeps
  volume references valid between runs; the re-export prevents `uuid` versions from diverging across
  crates.
- **Alternatives considered**: a hand-written string generator (no uniqueness or format
  guarantees); the volume serial number as an identifier (changes when the volume is reformatted and
  does not cover mount points).

## 24. Integration-test layout

- **Decision**: cross-crate tests and shared filesystem fixtures live in a separate
  `freespace-tests` package (`tests/Cargo.toml`), because the root manifest is virtual and the
  `tests/` directory would otherwise not be compiled.
- **Rationale**: `tests/common/mod.rs` is reused by every scenario, and `cargo test --workspace`
  runs it in CI.
- **Alternatives considered**: duplicating fixtures per crate (drift); tests on real disks
  (forbidden by the constitution).

## 25. Startup splash and application icon

- **Decision**: the visual identity is fixed in the ready-made files
  `apps/freespace-desktop/src-tauri/icons/icon.png` (256×256), `icon.ico` (16–256 px) and
  `apps/freespace-desktop/ui/public/splash.png`. Tauri embeds the ICO through `tauri-build`, and
  the splash is loaded from `public`.
- **Rationale**: the splash covers the settings load, and the same artwork identifies the window,
  the taskbar, Explorer and the installer. Preparing the sizes up front avoids decoding the
  original image on every start and keeps the memory budget.
- **Dependencies**: no extra runtime crates.
- **Alternatives considered**: separate `.png` files next to the `.exe` (breaks the single-file
  portable build).