# Operations audit: FreeSpace 002 (preview, confirmation, journal, undo)

**Date**: 2026-11-16 | **Feature**: `002-native-ui-redesign` | **Task**: T094

## Scope

Every action that deletes, moves, renames or changes permissions in the new
interface. The audit covers the Rust service layer, the Tauri adapters, the
shared React dialog and the screens that can start an operation.

## One path for every destructive action

```text
screen selection
  → operation_preview            (freespace-app::operations::OperationService::preview)
  → operation_preview_dialog     (shared OperationPreviewDialog)
  → operation_confirm            (OperationService::take: TTL, token, elevation, extra confirmation)
  → freespace-ops::delete_paths_cancellable
  → journal + operation-completed event
  → journal_undo / journal_restore
```

| Guarantee | Implementation | Evidence |
|-----------|----------------|----------|
| Explicit preview with targets, total size and mode | `OperationService::preview` builds a `freespace-ops` preview; the dialog renders every target, the total and the three delete modes | `operations_commands::preview_token_is_one_time_and_expires`, `operations.test.tsx` |
| One-time token bound to the preview | `confirmation_token` is checked and the record is removed by `take` | `operations_commands::preview_token_is_one_time_and_expires` |
| 10-minute TTL | `PREVIEW_TTL_MS`; expired previews are dropped | same test (expired branch) |
| Permanent deletion and shredding need a separate confirmation | `PreparedOperation::irreversible` plus `extra_confirmation` | `operations_commands::permanent_and_protected_operations_need_extra_confirmation`, `operations.test.tsx` |
| Protected/system paths require elevation | `freespace-ops` preview marks them; `take` returns `E_ELEVATION_REQUIRED` and the dialog shows the warning | same test, `elevation.ts` |
| No data changes when rights are missing | Elevation refusal happens before `delete_paths_cancellable` | same test |
| At most one destructive operation per volume | `OperationService::begin` reserves the volume and returns `E_BUSY` | `operations_commands::one_operation_per_volume_and_cooperative_cancel` |
| Actual journal record with mode, targets and result | `freespace-ops` writes the entry; cancellation is stored as a `status` detail | `operations_commands::quarantine_delete_is_journalled_and_restored` |
| Undo/restore only when reversible | `undo_available` is written by `freespace-ops`; `journal_page` exposes it | same test |
| Partial and cancelled results are not shown as success | `DeleteOutcome.status` and the `OperationProgress` state are mapped verbatim | `cancellation_keeps_the_actual_partial_result` |
| Duplicate copies never lose every copy | `duplicates::deletion_targets` validates the selection server-side; the preview command re-checks it | `analysis_commands::duplicate_search_keeps_one_copy_and_pages_groups` |

## Screens that start operations

| Screen | Action | Preview task id | Notes |
|--------|--------|-----------------|-------|
| Top files | delete selected rows | — | default mode Recycle Bin; permanent needs the extra checkbox |
| Types | delete selected files of a type | — | same shared dialog |
| Duplicates | delete selected copies | duplicate `task_id` | the server validates the group selection again |
| Cleanup | delete recommendations or empty directories | — | risk and category are visible before selection |
| Journal | undo or restore an entry | — | `journal_undo` / `journal_restore` go through `freespace-ops::undo_operation` |

Screens from later batches (long paths, rename, permissions, shredder) reuse
`OperationService`; their commands are not part of this phase, so they cannot
bypass it accidentally.

The only destructive command outside this path is `snapshots_delete`, which
removes an application-generated `.fssnap` report file, not user data. It is
limited to the snapshot directory and cannot be pointed at a scanned volume.

## UI guarantees

- The confirm button is disabled until the acknowledgement checkbox is checked;
  Enter does not run the operation because focus starts on the dialog, not on the
  confirm button.
- Irreversible modes add a second checkbox and switch the button to `danger`.
- Warnings (`confirm.warning.*`) are shown before confirmation; missing targets,
  large operations, protected paths and elevation are distinct keys.
- Operation progress shows the native state, processed count, current target,
  errors, cancellation and undo availability.
- The journal screen never claims success: `success`, `partial`, `failed` and
  `cancelled` are separate badges.

## Verification

```powershell
cargo test -p freespace-tests --test analysis_commands --test operations_commands
cargo test -p freespace-app operations::
npm run test:unit -- --run tests/unit/operations/operations.test.tsx
npm run test:e2e -- tests/e2e/operations.spec.ts
```

All commands are green. Destructive scenarios ran only in temporary
directories; no user data or working disks were touched.

## Result

100% of the destructive actions implemented in this phase pass through
preview → confirmation → `freespace-ops` → journal → undo/restore. No command
accepts a raw path list without a live preview token.
