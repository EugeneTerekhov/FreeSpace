# Implementation Plan: Multi-volume snapshot

**Branch**: `004-multi-volume-snapshot` | **Date**: 2026-11-20 | **Spec**: [spec.md](./spec.md)

## Summary

One scan accepts one to 32 roots and creates **one immutable snapshot** with several roots. The
scanning core is reused as-is: each root keeps its own method (MFT or walk), but nodes are appended
to **one `FsTree` arena**, so merging does not double memory. The snapshot stores the volume list and
one session per root; the tree shows every root as a top-level row and analysis works over the whole
arena.

The GUI gets multi-select and an explicit snapshot-composition indicator. The CLI gets several
positional paths for `scan` and `tree`. The scan request, task, progress events and contracts change
together; the IPC version is bumped.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024), TypeScript 5.6+, React 19, Node.js 20.19+.

**Primary Dependencies**: no new dependencies. Reuses `freespace-core`, `freespace-scan`,
`freespace-platform`, `freespace-app`, `freespace-cli`, `tauri` 2, `clap`, `zustand`.

**Storage**: SQLite journal and settings are unchanged. `.fssnap` and report formats are **not
changed** by this feature: they record the primary (first) volume and list every root. Extending the
formats is a separate future feature.

**Testing**: `cargo test --workspace`, contract tests in `tests/contract/*`, CLI contract in
`tests/cli_contract.rs`, UI unit tests (`vitest`), e2e/a11y without regressions.

**Target Platform**: Windows 10 21H2+/11 x64 and Linux x86_64 (portable 2.0).

**Project Type**: Rust workspace + Tauri/React desktop app + CLI.

**Performance Goals**:

- One volume: the current 8 TB / 60 s budget on a consumer SSD.
- Two volumes totalling 8 TB: 120 s or less (sequential scanning, no parallel I/O).
- Memory: 4 GB or less for 10 million nodes across all roots.
- UI response: 200 ms or less for the tree root page.
- Progress: at least 10 events per second and a monotonic overall percentage.

**Constraints**:

- One current snapshot; a new scan merges its volumes into the current snapshot only after at least
  one root succeeds.
- Roots are scanned sequentially and deterministically.
- Duplicate and overlapping roots are rejected before the scan starts.
- All platform calls stay in `freespace-platform`; no network calls or telemetry.
- Destructive operations are unchanged; the journal records the target volume instead of the first
  snapshot volume.

**Scale/Scope**: up to 32 roots, up to 10 million nodes, 9 GUI screens, CLI + GUI, RU/EN.

## Constitution Check

| Principle | How it is satisfied |
|---|---|
| I. Safety of destructive operations | Operations are unchanged; preview, confirmation and journal remain. The journal starts recording the target volume (FR-016). |
| II. Accounting accuracy | One node is appended to the arena exactly once; overlapping roots are rejected, so nothing is counted twice. Root sizes are summed into the snapshot total. |
| III. Performance | Roots are scanned sequentially into one arena without an intermediate merge; budgets are 8 TB/60 s per volume, 4 GB/10 million nodes in total, UI 200 ms. |
| IV. Native platform accuracy | Each root uses its own method (MFT/walk) and capabilities; a mixed method set is shown explicitly. |
| V. Test-first approach | New tests cover the core (`scan_many`), validation, the CLI contract and the UI before implementation; fixtures are temporary directories. |
| VI. CLI and GUI parity | `scan` and `tree` accept several paths and emit JSON with `volumes`, `roots` and `total`; the GUI uses the same core. |
| VII. Privacy and offline operation | No new network calls; data never leaves the machine. |

No violations; Complexity Tracking is empty.

## 1. Architecture decisions

### 1.1 One snapshot, many roots

`TreeSnapshot` becomes a snapshot of a root set:

- `volumes: Vec<Volume>` — unique volumes in root order;
- `sessions: Vec<ScanSession>` — one session per successful root;
- `volume()` and `session()` are kept for compatibility and return the first element;
- `volumes()` and `sessions()` are new accessors;
- `volume_for_path(&Path)` returns the volume that owns a path (longest matching mount point) for the
  journal and task metadata.

`TreeSnapshot::new` remains for a single volume; `TreeSnapshot::new_multi` is added.

### 1.2 Scanning into a shared arena

In `freespace-scan`:

- `scan()` is split into `scan_into(request, tree, observer) -> RootScanOutcome` and final
  aggregation.
- `scan_into` scans one root, appends it as a new arena root, counts only that root's nodes and does
  **not** call `recompute_totals`, `compact` or `validate`.
- `scan_many(requests, observer) -> ScanManyResult` creates one `FsTree`, calls `scan_into` in order,
  isolates root errors, stops on cancellation and runs `recompute_totals`, `validate` and `compact`
  once.
- An observer wrapper maps a root-local fraction to the overall percentage
  `(index + local_fraction) / roots_count` and forwards events. A final `Finished` event reports 1.0.
- When every root fails, the first error is returned; when at least one succeeds, the result carries
  `failures`.

### 1.3 Request and task

- DTO `ScanRequest` gets `paths: Vec<String>` instead of `path`.
- `Task` gets `volume_ids: Vec<String>`; `volume_id` stays the first element for compatibility.
- `ScanSummary` in the completion event gets `volumes` and `methods`; `method` is the single method
  or `mixed`.
- `ScanSessionSummary` remains one record per root; `scan_sessions` returns all of them.

### 1.4 Validation

- 1 to 32 paths, each absolute and non-empty.
- Duplicate roots are rejected by the platform path key.
- Overlapping roots are rejected after canonicalisation: no root is an ancestor of another.
- Validation runs before volumes are reserved and the worker starts.

### 1.5 Volume reservation

- `AppState` gets `reserve_scans(task_id, volume_ids, cancel)` and `release_scans(volume_ids)`.
- If any volume is busy the task does not start (`E_BUSY`); there is no partial reservation.
- The task stores `volume_ids`; all reservations are released when it finishes.

### 1.6 Progress, cancellation and errors

- Progress is sequential: volume `i` of `N`, with a monotonic overall percentage.
- Cancellation between roots does not start the remaining roots; collected data is kept.
- A root error adds a `root: cause` warning; the task becomes `partial`.
- When no root succeeds, no snapshot is installed and the task becomes `failed`.

### 1.7 GUI

- `snapshotStore` stores `volumes: VolumeSummary[]` and derives `volume` (the first) for
  compatibility.
- After `scan-completed` the frontend requests `snapshot_volumes(generation)`.
- Overview: a volume is selected by clicking its card, and the "Analyze" button starts the selected
  set. A card shows the scanning marker when its volume is in `task.volume_ids`.
- Tree: the total size is the sum of every root row; the context names the volume count; single-volume
  texts are unchanged.
- Other screens use the first volume only where a default path is needed; data comes from the whole
  snapshot.

### 1.8 CLI

- `freespace scan <PATH>...` and `freespace tree <PATH>...` accept one or several paths.
- `scan` calls the same `scan_many`; JSON contains `volumes`, `roots`, `methods`, `total`, `issues` and
  `node_count`; `--snapshot-out` writes the primary volume and every root.
- One path keeps the previous shape and adds the new arrays.

## 2. Changed components

| Layer | Files |
|---|---|
| Scan core | `crates/freespace-core/src/tree.rs`, `crates/freespace-scan/src/session.rs`, `crates/freespace-scan/src/lib.rs` |
| Application | `crates/freespace-app/src/{scan.rs,validation.rs,tree.rs,dto/mod.rs}` |
| Tauri | `apps/freespace-desktop/src-tauri/src/{state.rs,commands/scan.rs,commands/volumes.rs,commands/operations.rs,commands/analysis.rs,commands/filesystem.rs,security.rs,lib.rs,events.rs}` |
| UI | `ui/src/api/{dto.ts,client.ts,transport.ts,mockTransport.ts}`, `ui/src/stores/snapshotStore.ts`, `ui/src/App.tsx`, `ui/src/screens/{Overview.tsx,Tree.tsx}`, styles and i18n |
| CLI | `crates/freespace-cli/src/{cli.rs,lib.rs,commands/scan.rs}` |
| Tests | scan/validation unit tests, `tests/cli_contract.rs`, `tests/contract/*`, UI unit/e2e |
| Documentation | `specs/004-multi-volume-snapshot/*`, `docs/en/004-multi-volume-snapshot/*`, user guides, `CHANGELOG.md` |

## 3. Performance and memory

- One arena for the whole snapshot; roots are appended sequentially and no intermediate trees are
  created.
- Peak memory equals the final arena plus one root's temporary structures (MFT records, maps), as it
  does today for one volume.
- `recompute_totals`, `validate` and `compact` run once after the last root.
- Progress is weighted by the number of roots, not bytes: cheap and monotonic; the current volume and
  path are shown explicitly.
- The UI does not load every root at once: `tree_root` returns a page of roots, as before.

## 4. Compatibility and migrations

- The IPC version is bumped; contract fixtures and tests are updated.
- `volumes_current` is kept and returns the first volume; the new `snapshot_volumes` returns all.
- Old clients with `path` are unsupported after the IPC bump; this is an internal application
  protocol.
- `.fssnap` and reports stay at version 1: primary volume plus every root. This limitation is recorded
  in the spec and quickstart.
- Settings and journal data do not migrate.

## 5. Risks

| Risk | Mitigation |
|---|---|
| Double counting with overlapping roots | Pre-scan validation plus a test for `C:\` and `C:\Users`. |
| Memory growth on a multi-volume snapshot | One arena, sequential roots, one `compact`. |
| Non-monotonic progress | `(index + local) / N` formula and a monotonicity test. |
| Losing results when one volume fails | Error isolation, `partial`, a snapshot is installed when at least one root succeeds. |
| GUI and CLI divergence | Shared `scan_many` core, CLI and IPC contract tests. |
| Wrong volume in the journal | `volume_for_path` and an operation test on the second volume. |
