# Tasks: Multi-volume snapshot

**Feature**: `004-multi-volume-snapshot` | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

**Rules**: critical modules are tested before implementation; `[x]` is set only after green
`cargo fmt`, `cargo clippy -D warnings`, `cargo test` and updated documentation.

## Phase 1 — Preparation

- [x] **T001** Establish the baseline: `cargo test --workspace`, `cargo clippy --workspace
  --all-targets -- -D warnings`, UI unit tests. Record known failures in `quickstart.md`.
- [x] **T002** Point `.specify/feature.json` to `specs/004-multi-volume-snapshot` (local file, not
  committed).

## Phase 2 — Scan core (`freespace-scan`)

- [x] **T010** Test: `scan_many` over two temporary directories creates one `FsTree` with two roots.
- [x] **T011** Test: one root failure does not stop the second; `failures` carries the path and cause.
- [x] **T012** Test: cancellation before the first root stops the run.
- [x] **T013** Test: overall progress is monotonic and finishes at `fraction = 1.0`.
- [x] **T014** Implement `RootScanOutcome`, `ScanFailure`, `ScanManyResult` and `scan_into`.
- [x] **T015** Implement `scan_many`: one arena, sequential roots, error isolation, one aggregation.
- [x] **T016** Implement the observer wrapper for `(index + local) / N` and the final event.
- [x] **T017** Keep `scan()` as a wrapper over `scan_into`; existing tests stay unchanged.
- [x] **T018** Export the new types from `crates/freespace-scan/src/lib.rs`.

## Phase 3 — Application model (`freespace-app`)

- [x] **T020** Test: `validate_scan_request` accepts 1..=32 paths and rejects empty, duplicate,
  overlapping and relative paths.
- [x] **T021** DTO: `ScanRequest.paths`, `Task.volume_ids`, `MAX_SCAN_ROOTS`.
- [x] **T022** Implement root validation: platform keys, duplicate and overlap rejection, limit 32.
- [x] **T023** Rewrite `run_scan` over `scan_many`: requests by path, one ADS enrichment, multi-root
  outcome.
- [x] **T024** Test: `TreeSnapshot::new_multi` keeps all volumes and sessions; `volume_for_path` picks
  the longest mount point.
- [x] **T025** Implement `volumes/sessions`, `new_multi`, `volumes()`, `sessions()`, `volume_for_path`,
  `root_paths`.
- [x] **T026** Test: export/snapshot on a multi-volume snapshot uses the first volume and every root.
- [x] **T027** Use the target path's volume for the operation journal and duplicate task metadata.

## Phase 4 — Tauri and IPC

- [x] **T030** Test: `reserve_scans` reserves every volume atomically and leaves no partial
  reservation on `E_BUSY`.
- [x] **T031** Implement `reserve_scans`/`release_scans` and task `volume_ids`.
- [x] **T032** Update `scan_start`: validate, canonicalise, resolve volumes, reserve atomically, start
  the worker.
- [x] **T033** Update the worker: multi-root snapshot, `partial` on a root error, `failed` on zero
  successful roots, summary with `volumes`/`methods`.
- [x] **T034** Add `snapshot_volumes(generation)` to commands, security and the invoke handler.
- [x] **T035** Update `events.rs`: `ScanSummary.methods/volumes`, `Task.volume_ids`.
- [x] **T036** Use the target volume in analysis and operations.
- [x] **T037** Check default paths in filesystem and inspection commands for a multi-volume snapshot.
- [x] **T038** Raise `IPC_VERSION` to 2 and update capability tests.

## Phase 5 — UI

- [x] **T040** Update `dto.ts`: `ScanRequest.paths`, `Task.volume_ids`, `ScanSummary.methods/volumes`.
- [x] **T041** Add `api.snapshotVolumes` to client, transport and mock transport.
- [x] **T042** `snapshotStore`: `volumes`, `setVolumes`, derived `volume`.
- [x] **T043** Refresh snapshot volumes after `scan-completed`.
- [x] **T044** Overview: volume selection by clicking a card, the "Analyze" button, a scanning marker
  and the scan-type selector next to the button.
- [x] **T045** Tree: sum of every root, volume-count context, fallback.
- [x] **T046** Keep the first volume only as a default path in other screens.
- [x] **T047** Add RU/EN i18n keys and regenerate the generated files.
- [x] **T048** Unit tests for the store, overview selection, tree totals and mock transport.
- [x] **T049** e2e/a11y for two volumes on the mock transport.

## Phase 6 — CLI

- [x] **T050** Test: `scan` with two paths returns two roots and the summed total; `tree` prints both.
- [x] **T051** `cli.rs`: `Scan.paths` and `Tree.paths`.
- [x] **T052** `commands/scan.rs`: `ScanManyArgs`, `scan_many`, `volumes`/`methods`, totals and the
  snapshot warning.
- [x] **T053** Keep the single-path behaviour and fields.

## Phase 7 — Contracts and fixtures

- [x] **T060** Update contract tests and fixtures for `paths`, `volume_ids`, `snapshot_volumes`,
  `methods`/`volumes`; regenerate fixtures.
- [x] **T061** Run core, analysis, inspection and filesystem contract tests.
- [x] **T062** Update Tauri e2e only where `scan_start`/`Task` is used.

## Phase 8 — Documentation

- [x] **T070** Create the English mirror `docs/en/004-multi-volume-snapshot/` without Cyrillic; keep
  IDs unchanged.
- [x] **T071** Update `docs/ru/user-guide.md` and `docs/en/user-guide.md`.
- [ ] **T072** Update `CHANGELOG.md` and the repository guides where needed.
- [x] **T073** Mark completed tasks in `tasks.md`; do not touch `checklists/quality.md`.

## Phase 9 — Verification

- [x] **T080** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`.
- [x] **T081** UI: `npm run typecheck`, `npm test`, `npm run lint`, e2e/a11y.
- [ ] **T082** Manual quickstart run on two test volumes (GUI + CLI).
- [ ] **T083** Verify budgets: two volumes 8 TB ≤ 120 s, 10 million nodes ≤ 4 GB, root page ≤ 200 ms.
- [ ] **T084** Commit with Conventional Commits (`feat: scan several volumes into one snapshot`).

