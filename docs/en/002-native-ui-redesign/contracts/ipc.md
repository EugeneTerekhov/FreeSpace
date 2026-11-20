# IPC contract: commands, events, pagination and security

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

The contract describes the boundary between React and Rust. Commands are the only way to get data
or perform an action; events carry progress and invalidation, but do not replace data pages. All
schemas are typed and verified by golden fixtures.

## 1. General rules

- `IPC_VERSION = 1`. Additive changes are compatible; removing/renaming a field or
  changing the meaning requires a new version and a UI update.
- Command names are `snake_case`; events are `kebab-case`.
- Input and output are JSON-compatible DTOs from [data-model.md](../data-model.md); large pages
  may use the binary `tauri::ipc::Response`.
- A command error is `Result<T, UiError>`; the UI does not show raw Rust text as the only
  explanation.
- Every command validates the input before accessing the FS; unknown fields are ignored, unknown
  enum values are rejected.
- Long-running commands return `task_id`/`operation_id` and continue in the background; cancellation
  is a separate command.
- Events are throttled to 10–20 Hz; `sequence` grows monotonically within a task, gaps are allowed,
  duplicates are dropped by `sequence`.
- The UI does not call FS-related commands directly, bypassing `freespace-ops`.

## 2. Error envelope

```json
{
  "code": "E_ACCESS_DENIED",
  "category": "access",
  "message_key": "error.access_denied",
  "details": "C:\\System Volume Information",
  "retryable": false,
  "task_id": null,
  "journal_entry_id": null
}
```

| Code | Category | When |
|-----|-----------|-------|
| `E_VALIDATION` | validation | Invalid input, enum, size, path |
| `E_ACCESS_DENIED` | access | No rights/access |
| `E_NOT_FOUND` | io | Object/volume/snapshot not found |
| `E_STALE_CURSOR` | stale | The cursor is stale; the UI starts from the first page |
| `E_STALE_NODE` | stale | The node is outside the current generation |
| `E_BUSY` | operation | A conflicting operation is already running |
| `E_CANCELLED` | cancel | The task was cancelled |
| `E_PREVIEW_EXPIRED` | validation | The preview has expired/the token is invalid |
| `E_ELEVATION_REQUIRED` | access | Administrator rights are required |
| `E_ELEVATION_DENIED` | access | The user declined elevation |
| `E_OPERATION_PARTIAL` | operation | Some targets were not processed |
| `E_IO` | io | File system error |
| `E_UNSUPPORTED` | validation | FS/format/operation is not supported |
| `E_OFFLINE` | internal | A network action attempt is prohibited |
| `E_INTERNAL` | internal | Internal error |

## 3. Application and shell

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `app_bootstrap` | `{}` | `{ ipc_version, app_version, os, webview_version, settings, window_state, capabilities, about }` | `E_INTERNAL` |
| `app_settings_get` | `{}` | `UiSettings` | `E_INTERNAL` |
| `app_settings_set` | `{ patch: Partial<UiSettings> }` | `UiSettings` | `E_VALIDATION` |
| `app_window_state_get` | `{}` | `WindowState` | `E_INTERNAL` |
| `app_window_state_set` | `{ state: WindowState }` | `{}` | `E_VALIDATION` |
| `app_navigation_request` | `{ screen, node_id?, path? }` | `{}` | `E_VALIDATION`, `E_NOT_FOUND` |
| `app_open_logs` | `{}` | `{}` | `E_IO` |
| `app_about_info` | `{}` | `{ version, build, webview_version, licenses, data_dir }` | `E_INTERNAL` |
| `app_elevation_status` | `{}` | `{ elevated, can_elevate, reason_key }` | `E_INTERNAL` |
| `app_elevation_request` | `{ reason_key }` | `{ elevated }` | `E_ELEVATION_DENIED`, `E_INTERNAL` |
| `app_capabilities` | `{}` | `{ commands, events, csp_version }` | `E_INTERNAL` |

`app_bootstrap` is called once before the first screen; `settings` and `window_state` are already
migrated values. `app_capabilities` is needed for the parity matrix and diagnostics.

## 4. Volumes and scanning

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `volumes_list` | `{}` | `VolumeSummary[]` | `E_INTERNAL` |
| `volumes_summary` | `{ volume_id }` | `VolumeDetail` | `E_NOT_FOUND` |
| `volumes_refresh` | `{}` | `VolumeSummary[]` | `E_INTERNAL` |
| `scan_start` | `{ path, method: auto\|mft\|walk, incremental, exclusions, include_ads, metric }` | `{ task_id, generation }` | `E_VALIDATION`, `E_ACCESS_DENIED`, `E_BUSY` |
| `scan_status` | `{ task_id }` | `ScanTask` | `E_NOT_FOUND` |
| `scan_cancel` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `scan_sessions` | `{}` | `ScanSessionSummary[]` | `E_INTERNAL` |

Rules: only one active `scan` per volume at a time; a repeated start returns `E_BUSY`.
`generation` grows after each completed/partial scan.

## 5. Tree, search and node shares

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `tree_root` | `{ generation, metric, sort, page_size }` | `TreePage` | `E_STALE_CURSOR`, `E_NOT_FOUND` |
| `tree_children` | `{ generation, parent_id, metric, sort, filter, cursor?, page_size }` | `TreePage` | `E_STALE_NODE`, `E_STALE_CURSOR` |
| `tree_search` | `{ generation, query, filter, sort, cursor?, page_size }` | `TreePage` | `E_VALIDATION`, `E_STALE_CURSOR` |
| `tree_node` | `{ generation, node_id }` | `NodeRow` | `E_STALE_NODE`, `E_NOT_FOUND` |
| `tree_path` | `{ generation, node_id }` | `NodeRow[]` (ancestors) | `E_STALE_NODE` |

`TreePage` and `NodeRow` are per [data-model.md](../data-model.md). A node's share of its parent
and of the volume is computed by the native layer and included in `NodeRow`, so the inline tree
progress bars do not need separate commands. `size_details` (section 7) provides the details for
the hover card.

## 6. Analysis and cleanup

`sort` accepts the keys `name`, `path`, `size`, `modified`, `items`. The `path` key sorts by the full
path and therefore groups objects by volume in a multi-volume snapshot.

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `top_files_page` | `{ generation, limit, metric?, sort?, filter, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `types_summary` | `{ generation, filter }` | `TypeSummary[]` | `E_STALE_CURSOR` |
| `types_files_page` | `{ generation, type, filter, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `duplicates_start` | `{ path, min_size, algorithm, keep_rule, exclusions }` | `{ task_id }` | `E_VALIDATION`, `E_BUSY` |
| `duplicates_status` | `{ task_id }` | `DuplicateTask` | `E_NOT_FOUND` |
| `duplicates_groups_page` | `{ task_id, cursor?, page_size }` | `DuplicateGroupPage` | `E_STALE_CURSOR` |
| `duplicates_group` | `{ task_id, group_id }` | `DuplicateGroup` | `E_NOT_FOUND` |
| `duplicates_cancel` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `cleanup_candidates` | `{ generation, older_than_days, filter }` | `CleanupCandidate[]` | `E_STALE_CURSOR` |
| `cleanup_empty_dirs_page` | `{ generation, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `snapshots_list` | `{ volume_id? }` | `SnapshotSummary[]` | `E_IO`, `E_UNSUPPORTED` |
| `snapshots_create` | `{ path, out }` | `{ task_id, snapshot_id }` | `E_VALIDATION`, `E_IO` |
| `snapshots_compare` | `{ base_id, target_id }` | `{ task_id }` | `E_UNSUPPORTED`, `E_VALIDATION` |
| `snapshots_diff_page` | `{ task_id, cursor?, page_size, sort }` | `SnapshotDiffPage` | `E_STALE_CURSOR` |
| `snapshots_delete` | `{ snapshot_id }` | `{}` | `E_IO`, `E_NOT_FOUND` |
| `snapshots_status` | `{ task_id }` | `TaskStatus` | `E_NOT_FOUND` |

Rules: `duplicates_start` and `snapshots_compare` do not block the UI; their progress arrives via
events. `cleanup_candidates` and `cleanup_empty_dirs_page` only read data; deletion goes through
`operation_preview`.

`types_summary` accepts an optional `group_by` (`extension` by default or `category`); the
`extension`/`category` fields in the response are filled accordingly. `journal_page` accepts an
optional `filter` (`text`, `kinds`, `results`); filtering runs in Rust.

## 7. Journal, quarantine and file operations

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `journal_page` | `{ filter, cursor?, page_size }` | `JournalPage` | `E_STALE_CURSOR` |
| `journal_entry` | `{ entry_id }` | `JournalEntryView` | `E_NOT_FOUND` |
| `journal_undo` | `{ entry_id }` | `{ operation_id }` | `E_BUSY`, `E_UNSUPPORTED` |
| `journal_restore` | `{ entry_id, quarantine_ids? }` | `{ operation_id }` | `E_BUSY`, `E_IO` |
| `quarantine_items_page` | `{ cursor?, page_size }` | `QuarantinePage` | `E_STALE_CURSOR` |
| `size_details` | `{ generation, node_id }` | `SizeDetails` | `E_STALE_NODE` |
| `long_paths_page` | `{ generation, filter, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `long_paths_validate` | `{ path }` | `{ valid, reason_key?, normalized? }` | `E_VALIDATION` |
| `rename_preview` | `{ targets, regex, replacement, scope }` | `RenamePreview` | `E_VALIDATION`, `E_BUSY` |
| `rename_apply` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY` |
| `reveal_in_explorer` | `{ path }` | `{}` | `E_ACCESS_DENIED`, `E_NOT_FOUND` |
| `permissions_get` | `{ path }` | `PermissionsView` | `E_ACCESS_DENIED` |
| `permissions_preview` | `{ path, changes }` | `PermissionsPreview` | `E_VALIDATION`, `E_ACCESS_DENIED` |
| `permissions_apply` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_ELEVATION_REQUIRED` |
| `attributes_preview` | `{ targets, changes }` | `OperationPreview` | `E_VALIDATION` |
| `attributes_apply` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED` |

`SizeDetails` contains `logical_size`, `on_disk_size`, `physical_size`, `allocated_size`,
`hardlink_contribution`, `ads_size`, `compression_savings`, `sparse_savings`, `link_target`,
`metric_label`. The UI does not recompute these values.

`reveal_in_explorer` only opens a directory in Explorer or shows a file selected inside its parent
folder. It changes nothing, accepts a path only inside the scanned area and has no CLI equivalent:
it is a navigation action of the shell, not a data operation. Deleting from the tree goes through
`operation_preview`/`operation_confirm` with `delete_mode = recycle_bin`; renaming goes through
`rename_preview`/`rename_apply`.

## 8. Inspection, reports and unlocking

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `metadata_get` | `{ generation, node_id }` | `MetadataView` | `E_STALE_NODE`, `E_UNSUPPORTED` |
| `owners_page` | `{ generation, filter, cursor?, page_size }` | `OwnerSummaryPage` | `E_STALE_CURSOR` |
| `owners_files_page` | `{ generation, owner, cursor?, page_size }` | `TreePage` | `E_STALE_CURSOR` |
| `archives_entries_page` | `{ task_id?, path, filter, cursor?, page_size }` | `ArchiveEntryPage` | `E_UNSUPPORTED`, `E_ACCESS_DENIED` |
| `archives_export_list` | `{ path, format, out }` | `ExportResult` | `E_UNSUPPORTED`, `E_IO` |
| `export_preview` | `{ scope, format, composition }` | `ExportPreview` | `E_VALIDATION` |
| `export_start` | `{ preview_id }` | `{ task_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY` |
| `export_status` | `{ task_id }` | `TaskStatus` | `E_NOT_FOUND` |
| `export_cancel` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `unlock_locks` | `{ path }` | `FileLock[]` | `E_NOT_FOUND`, `E_ACCESS_DENIED` |
| `unlock_release` | `{ path, mode: soft\|force, process_ids }` | `{ operation_id }` | `E_BUSY`, `E_ACCESS_DENIED` |
| `unlock_schedule_reboot` | `{ path }` | `{ operation_id }` | `E_ACCESS_DENIED` |

Archives are not extracted to disk; `archives_export_list` exports only the list of contents.
Password-protected/corrupted archives return `E_UNSUPPORTED` with a clear key.

## 9. Operations, shredder and monitoring

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `operation_preview` | `{ kind, targets, options }` | `OperationPreview` | `E_VALIDATION`, `E_ACCESS_DENIED` |
| `operation_confirm` | `{ preview_id, confirmation_token, delete_mode? }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY`, `E_ELEVATION_REQUIRED` |
| `operation_cancel` | `{ operation_id }` | `{}` | `E_NOT_FOUND` |
| `operation_status` | `{ operation_id }` | `OperationProgress` | `E_NOT_FOUND` |
| `operation_undo` | `{ operation_id }` | `{ operation_id }` | `E_UNSUPPORTED`, `E_BUSY` |
| `operation_restore` | `{ quarantine_ids }` | `{ operation_id }` | `E_IO`, `E_BUSY` |
| `shred_preview` | `{ targets, passes, pattern: zeros\|random }` | `OperationPreview` | `E_VALIDATION` |
| `shred_start` | `{ preview_id, confirmation_token }` | `{ operation_id }` | `E_PREVIEW_EXPIRED`, `E_BUSY` |
| `shred_status` | `{ operation_id }` | `OperationProgress` | `E_NOT_FOUND` |
| `shred_cancel` | `{ operation_id }` | `{}` | `E_NOT_FOUND` |
| `monitor_start` | `{ threshold, interval, exclusions }` | `{ task_id }` | `E_VALIDATION`, `E_BUSY` |
| `monitor_stop` | `{ task_id }` | `{}` | `E_NOT_FOUND` |
| `monitor_status` | `{ task_id }` | `MonitorStatus` | `E_NOT_FOUND` |
| `monitor_events_page` | `{ task_id, filter, cursor?, page_size }` | `MonitorEventPage` | `E_STALE_CURSOR` |
| `monitor_reveal_file` | `{ path }` | `{}` | `E_VALIDATION`, `E_NOT_FOUND`, `E_ACCESS_DENIED` |
| `monitor_reveal_executable` | `{ process_id }` | `{}` | `E_VALIDATION`, `E_NOT_FOUND`, `E_ACCESS_DENIED` |
| `monitor_set_exclusions` | `{ exclusions }` | `{}` | `E_VALIDATION` |

`monitor_reveal_file` and `monitor_reveal_executable` open Explorer at the folder of a monitor
event's file or process executable; they are not limited to the snapshot scope and change nothing.

Rules: `operation_confirm` without a valid `preview_id` + `confirmation_token` is prohibited;
`shred` is always irreversible and requires separate confirmation; `operation_cancel` may
return `E_UNSUPPORTED` if the operation has passed the point of no return.

## 10. Settings, elevation and "About"

| Command | Input | Output | Errors |
|---------|------|-------|--------|
| `settings_get` | `{}` | `SettingsView` | `E_INTERNAL` |
| `settings_set` | `{ patch }` | `SettingsView` | `E_VALIDATION`, `E_IO` |
| `settings_validate_paths` | `{ paths }` | `PathValidation[]` | `E_VALIDATION` |
| `settings_reset` | `{ section }` | `SettingsView` | `E_VALIDATION` |
| `about_info` | `{}` | `AboutInfo` | `E_INTERNAL` |
| `about_licenses` | `{}` | `LicenseInfo[]` | `E_INTERNAL` |

`SettingsView` = the existing 001 settings + `[ui]` from `UiSettings`. The `ui` section must not
break the CLI `config get/set`; the CLI sees only the keys it knows.

## 11. Events

| Event | Payload | Frequency | Purpose |
|---------|---------|---------|------------|
| `scan-progress` | `{ task_id, sequence, progress, phase_key, current_path? }` | 10–20 Hz | Scan progress |
| `scan-completed` | `{ task_id, state, generation, summary?, error? }` | once | Completion/partial result |
| `duplicates-progress` | `{ task_id, sequence, progress, phase_key }` | 10–20 Hz | Duplicate search |
| `duplicates-completed` | `{ task_id, state, groups_count, error? }` | once | Completion of the search |
| `operation-progress` | `{ operation_id, sequence, state, processed, total, current_target?, errors? }` | 10–20 Hz | Operation progress |
| `operation-completed` | `{ operation_id, state, journal_entry_id?, undo_available, error? }` | once | Operation completion |
| `monitor-tick` | `{ task_id, sequence, events }` | 1–2 Hz | Write monitoring |
| `task-queue-changed` | `{ tasks: ScanTask[] }` | on change | Shell/status |
| `volume-changed` | `{ volumes: VolumeSummary[] }` | on Windows event | Volume appearance/removal |
| `settings-changed` | `{ settings: SettingsView }` | on change | Screen synchronization |
| `navigation-request` | `{ screen, node_id?, path? }` | on event | Deep links, single instance |

Event envelope:

```json
{
  "version": 1,
  "event": "scan-progress",
  "sequence": 42,
  "task_id": "…",
  "emitted_at": "2026-11-16T12:00:00Z",
  "payload": { "progress": { "processed": 1000, "total": 100000 } }
}
```

Rules: `sequence` is monotonic within a task; the UI drops `sequence ≤ last_seen`;
progress is no more than 20 Hz; errors are not duplicated in every event, but arrive in
`*-completed`/`UiError`.

## 12. Pagination, cursors and generations

- `page_size` — 1..2000, default 500; `max_tiles` — 100..10 000.
- `cursor` is an opaque string ≤ 512 characters; the UI does not parse it.
- `next_cursor = null` — end; `stale_cursor = true` — start from the first page.
- `generation` — a monotonic number of the data snapshot; pages of another generation are rejected.
- Changing sorting/filter/generation resets the cursor.
- `total_estimate` is an estimate, not an exact counter; the UI does not use it for totals.

## 13. Operation preview and confirmation

```text
operation_preview
  → preview_id, confirmation_token, expires_at, targets, total_size, warnings
operation_confirm(preview_id, token, delete_mode?)
  → operation_id
operation_status / operation-progress / operation-completed
  → journal_entry_id, undo_available
```

Rules:

- `preview_id` and `confirmation_token` are one-time, TTL 10 minutes, bound to the user and
  the target set.
- Repeated confirmation — `E_PREVIEW_EXPIRED`.
- The UI must show `warnings` (SSD, system path, large volume) before confirmation.
- Irreversible deletion and the shredder require a separate confirmation different from the usual
  one.
- The operation does not start if `requires_elevation = true` and there are no rights.
- On partial success the UI shows `partial` and a link to the journal, not "success".

## 14. Security and capabilities

- Capabilities: `core:default` + targeted `core:window:*`, `core:event:*`,
  `single-instance:default`; the `fs`, `shell`, `http`, `process` plugins are not connected.
- CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:;
  style-src 'self' 'unsafe-inline'; script-src 'self'`; remote URLs are prohibited.
- `withGlobalTauri: false`; devtools are disabled in release; `dangerousDisableAssetCspModification`
  is not used.
- All paths are canonicalized; going outside the selected scope — `E_ACCESS_DENIED`.
- Commands do not accept arbitrary shell commands, URLs or SQL.
- Network calls are prohibited; the WebView2 background network is disabled by arguments and
  checked by a network guard in E2E.
- Logs are local only in `data/logs`; without file contents and secrets.

## 15. Contract verification

- For each command there is a golden fixture of success and at least one error in
  `specs/002-native-ui-redesign/contracts/fixtures/` (or `tests/contract/`).
- The test compares the frontend client's command list with `app_capabilities`; an extra/missing
  command is a failure.
- The test verifies that `sequence` is monotonic and that `stale_cursor` is handled.
- The test verifies that the UI does not call prohibited plugins/network APIs.
- All contract changes are accompanied by an update of `IPC_VERSION` (if necessary) and the
  parity matrix.
