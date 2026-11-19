# FreeSpace Constitution

## Core Principles

### I. Safety of Destructive Operations (NON-NEGOTIABLE)

Any operation that modifies or deletes user data (deletion, overwrite, rename, move,
permission and attribute changes) MUST be performed only after explicit user confirmation
with a preview of the affected objects and their total size. Deletion MUST default to the
Recycle Bin or a quarantine area; irreversible destruction (shredder) MUST be a separate,
deliberate action with an additional warning. Batch operations MUST support undo where
technically possible and MUST be journaled. Operations on system and protected directories
MUST require administrator rights and additional confirmation. The application MUST NOT
modify data outside the user-selected scope.

Rationale: the tool works with the only copies of user data; mistakes are irreversible, so
safety outranks convenience and speed.

### II. Accuracy of Disk Space Accounting

The application MUST distinguish logical file size from size actually occupied on disk.
Hard links MUST be counted once per physical file; junctions and symlinks MUST NOT be
followed unless explicitly requested by the user; compressed NTFS files, sparse files, and
alternate data streams (ADS) MUST be accounted according to actual space consumption.
Directory totals MUST agree with file-system data and MUST NOT overstate or understate
occupied space. Reports MUST clearly state which size type is shown (logical or on-disk).

Rationale: the product's primary value is a trustworthy answer to "where did the space go";
inaccurate accounting devalues every other feature.

### III. Performance at Scale

Scanning an NTFS volume MUST use direct MFT reading as the primary path and MUST analyze a
volume of up to 8 TB in no more than 60 seconds on a typical consumer SSD (target metric).
The user interface MUST remain responsive during scanning and any long-running operation:
work MUST run on background threads, lists and trees MUST be virtualized, and results MUST
appear progressively. Repeated scans MUST support incremental updates via the USN journal.
Every long-running operation MUST have a measurable time and memory budget recorded in the
plan.

Rationale: on multi-terabyte disks slow scanning makes the tool unusable, and a frozen UI is
perceived as a failure.

### IV. Windows-Native Fidelity

The application MUST be native to Windows 10 (21H2+) and Windows 11 x64 and MUST fully
support NTFS: long paths (beyond 260 characters) without enabling system policy, Unicode
names, NTFS permissions and owners, file attributes, and on-demand UAC elevation. For
non-NTFS volumes (FAT32, exFAT, ReFS) a correct fallback to directory-tree traversal MUST
exist and the applied scan method MUST be clearly indicated. The UI framework MUST NOT limit
the availability of core system capabilities. All system calls MUST be isolated in a
dedicated platform layer.

Rationale: the product solves tasks unavailable in plain Explorer; any loss of native
functionality destroys its value proposition.

### V. Test-First File-System Operations (NON-NEGOTIABLE)

Critical modules (scanning, hashing, shredder, deletion and moving, permissions, long paths,
links) MUST be covered by automated tests before changes are merged. Tests MUST use isolated
file-system fixtures (temporary directories, test VHD images) and MUST NOT touch real user
data. Scan results and reports MUST be deterministic: the same input MUST produce the same
output. New capabilities MUST follow the "test → fail → implement" cycle.

Rationale: file-system defects cause data loss and false reports; tests are the only defense
reproducible in CI.

### VI. CLI/GUI Parity

Every user capability available in the graphical interface MUST be available from the command
line with machine-readable output (JSON) and MUST support a non-interactive mode for Windows
Task Scheduler and CI. Report formats and exit codes MUST be documented and stable within a
major version. GUI and CLI MUST share the same core.

Rationale: automation is a key scenario for system administrators; divergence between GUI
and CLI behavior produces non-reproducible results.

### VII. Privacy and Offline Operation

The application MUST work fully without a network connection. Scanned data, metadata,
reports, and snapshots MUST NOT leave the user's machine. Telemetry MUST be absent by
default; any future diagnostics submission MUST be explicitly opt-in and documented.

Rationale: the tool sees the contents of the entire disk, including confidential data; user
trust is critical.

## Technology and Platform Constraints

- Implementation language — Rust (stable, edition 2024); UI — egui/eframe as the primary
  option; Tauri is allowed only with a justified need for web technologies and must not
  reduce native functionality.
- Target platforms for v1: Windows 10 21H2+ and Windows 11, x64. Cross-platform support is
  not a v1 goal.
- Primary file system — NTFS; other file systems are supported in fallback mode.
- Configuration and user data are stored locally; the snapshot format is versioned.
- Project documentation is maintained in two languages: Russian (canonical in `specs/<feature>/`
  and `docs/ru/`) and English (`docs/en/`); no separate `ru/` mirror is created.

## Development Workflow and Quality Gates

- Development follows the Spec Kit Spec-Driven Development process: constitution →
  specify → clarify → plan → checklist → tasks → implement → converge; artifacts live in
  `specs/<feature>/`.
- Every plan MUST include a Constitution Check section confirming compliance with the
  principles; deviations MUST be justified in Complexity Tracking.
- Code changes MUST pass `cargo fmt --check` and `cargo clippy -- -D warnings`; builds MUST
  be reproducible in CI.
- Commits MUST follow Conventional Commits; behavior changes MUST be reflected in the
  CHANGELOG.
- Real-disk validation MUST be performed manually against quickstart scenarios before a
  release.

## Governance

The constitution supersedes all other project practices. Amendments MUST be made as a
separate change with justification, a version and date update, and a Sync Impact Report.
Constitution versioning is semantic: MAJOR — removal or redefinition of principles, MINOR —
addition of a principle or material expansion, PATCH — wording clarifications. Every plan and
review MUST verify constitution compliance; non-compliance MUST block merging until resolved
or explicitly justified. The change author and the reviewer are accountable for compliance.

**Version**: 1.0.1 | **Ratified**: 2026-11-15 | **Last Amended**: 2026-11-18
