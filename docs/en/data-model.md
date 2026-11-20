# Data Model: FreeSpace

**Feature**: `001-freespace-disk-analyzer` | **Date**: 2026-11-15 | **Plan**: [plan.md](./plan.md)

> **2.0 note (2026-11-20)**: the 2.0 model removes `QuarantineItem`, keeps only `RecycleBin` and
> `Permanent` in `DeleteMode`, and schema migration 2 drops the quarantine table. Old
> `quarantine`/`restore` journal entries remain readable as audit records. The current model is
> [003/data-model.md](../../specs/003-cross-platform-portable/data-model.md).

The model describes domain entities, their fields (without tying them to a physical representation),
and integrity rules. Types are conceptual; the implementation uses compact structures and arenas.

## 1. Volume

Represents a disk or partition available for analysis.

| Field | Type | Description |
|------|-----|----------|
| `id` | UUID/string | Stable identifier within the machine |
| `mount_point` | path | Drive letter or mount point |
| `label` | string | Volume label |
| `file_system` | enum | NTFS, FAT32, exFAT, ReFS, other |
| `total_bytes` | u64 | Total size |
| `free_bytes` | u64 | Free space |
| `cluster_size` | u32 | Cluster size (if known) |
| `scan_method` | enum | MFT, USN, directory-walk |
| `is_elevated` | bool | Whether the scan was performed with elevated permissions |
| `is_removable` | bool | Removable media |
| `status` | enum | available, offline, inaccessible |

**Rules**:
- `free_bytes ≤ total_bytes`.
- `scan_method = MFT` is allowed only for NTFS and when permissions are available.
- For `status = offline`, scanning and operations are prohibited.

## 2. FsNode (File System Node)

A file, directory, or link in the analysis tree.

| Field | Type | Description |
|------|-----|----------|
| `id` | u32 | Index in the node arena |
| `parent_id` | u32? | Parent node (null for the volume root) |
| `first_child_id` | u32? | First child node |
| `next_sibling_id` | u32? | Next sibling |
| `name` | string | Name without path |
| `kind` | enum | file, directory, symlink, junction, hardlink |
| `file_ref` | u64 | NTFS File Reference Number (for deduplication) |
| `size` | SizeMetrics | Size metrics (see below) |
| `timestamps` | struct | created, modified, accessed (UTC) |
| `attributes` | bitmask | read-only, hidden, system, archive, compressed, sparse, reparse |
| `owner_id` | u32? | Indexed SID of the owner |
| `access` | enum | ok, denied, error |
| `changed_during_scan` | bool | Object changed during the scan |
| `link_target` | string? | Link target (if applicable) |
| `ads` | list | Alternate streams (if present) |

**Rules**:
- `kind = file` has no children; `kind = directory` may have children.
- For `kind = hardlink`, `size.logical` is counted; `size.physical` — only once per `file_ref`.
- For `kind = symlink/junction`, the target is not traversed without the "follow links" flag.
- `access = denied` does not exclude the node from the tree but excludes its size from exact totals (it is flagged).

## 3. SizeMetrics (Size Metrics)

| Field | Type | Description |
|------|-----|----------|
| `logical` | u64 | Logical size (sum of file lengths) |
| `on_disk` | u64 | Actually occupied space (accounting for compression/sparse) |
| `physical` | u64 | Physical contribution with hard link deduplication |
| `ads` | u64 | Size of alternate streams |
| `savings` | u64 | Savings from compression/sparse |
| `method` | enum | exact, estimated |

**Rules**:
- `physical ≤ logical` for directories with hard links.
- `savings = logical − on_disk` (non-negative).
- If `method = estimated`, the UI and reports must mark the value as estimated.

## 4. FsLink (Link)

| Field | Type | Description |
|------|-----|----------|
| `node_id` | u32 | Link node |
| `link_type` | enum | hard, symlink, junction |
| `target_path` | string | Link target |
| `target_file_ref` | u64? | Target FRN (if available) |
| `counted` | bool | Whether the target object is counted in physical size |

## 5. ScanSession (Scan Session)

| Field | Type | Description |
|------|-----|----------|
| `id` | UUID | Session identifier |
| `volume_id` | UUID | Volume |
| `roots` | list of paths | Analysis areas |
| `method` | enum | MFT, USN, directory-walk |
| `started_at` / `finished_at` | time | Boundaries |
| `status` | enum | running, completed, cancelled, failed |
| `progress` | 0..100 | Progress |
| `nodes_count` | u64 | Node count |
| `errors` | list | Access errors and others |
| `journal_state` | struct? | JournalID and NextUSN for incremental |
| `exclusions` | list | Excluded paths |
| `snapshot_id` | UUID? | Snapshot created from the session |

**State transitions**: `running → completed | cancelled | failed`; transitions backward are prohibited.

## 6. DuplicateGroup (Duplicate Group)

| Field | Type | Description |
|------|-----|----------|
| `id` | UUID | Group identifier |
| `hash_algorithm` | enum | sha256, md5 |
| `hash` | string | Full hash |
| `size` | u64 | File size |
| `members` | list of `DuplicateMember` | Members |
| `keep_node_id` | u32? | Copy recommended for keeping |
| `reclaimable_bytes` | u64 | Reclaimable size |
| `stale` | bool | Files changed during the search |

**DuplicateMember**: `node_id`, `path`, `mtime`, `selected_for_deletion`, `verified` (byte-for-byte
match), `reason`.

**Rules**:
- At least two members.
- Deletion is prohibited if all members are marked for deletion.
- `stale = true` blocks deletion until re-verification.

## 7. CleanupCandidate (Cleanup Candidate)

| Field | Type | Description |
|------|-----|----------|
| `node_id` | u32 | Object |
| `category` | enum | temp, cache, log, backup, outdated, empty-dir, duplicate, large |
| `reason` | string | Human-readable explanation |
| `reclaimable_bytes` | u64 | Potential savings |
| `risk` | enum | low, medium, high |
| `recommended_action` | enum | review, delete, keep |
| `selected` | bool | Selected by the user |

## 8. Snapshot

| Field | Type | Description |
|------|-----|----------|
| `format_version` | u32 | Schema version |
| `id` | UUID | Identifier |
| `volume` | Volume snapshot | Volume metadata |
| `created_at` | time | Creation date |
| `method` | enum | Scan method |
| `node_count` | u64 | Node count |
| `roots` | list | Analysis roots |
| `checksum` | string | Contents checksum |
| `path` | string | Path to the `.fssnap` file |

**Rules**:
- A mismatch of `format_version` or `volume.id` makes comparison invalid.
- A snapshot is immutable after creation.

## 9. SnapshotDiff (Snapshot Delta)

| Field | Type | Description |
|------|-----|----------|
| `base_snapshot_id` / `target_snapshot_id` | UUID | Snapshots being compared |
| `added` / `removed` / `changed` | list of `DiffEntry` | Changes |
| `growth_by_directory` | list of `GrowthEntry` | Growth by directory |
| `total_growth_bytes` | i64 | Total change |

**DiffEntry**: `path`, `kind`, `size_delta`, `mtime_delta`.

## 10. FileLock (File Lock)

| Field | Type | Description |
|------|-----|----------|
| `node_id` | u32 | File |
| `process_id` | u32 | PID |
| `process_name` | string | Process name |
| `handle_type` | enum | file, directory, unknown |
| `access_mask` | u32 | Access type |
| `can_soft_close` | bool | Graceful close available |
| `requires_reboot` | bool | Deletion on reboot required |

## 11. Operation

| Field | Type | Description |
|------|-----|----------|
| `id` | UUID | Identifier |
| `kind` | enum | delete, shred, move, rename, permissions, attributes, restore, archive-extract, delete-on-reboot |
| `targets` | list of `OperationTarget` | Objects |
| `delete_mode` | enum? | recycle-bin, quarantine, permanent |
| `params` | struct | Parameters (overwrite method, passes, rules, etc.) |
| `status` | enum | planned, confirmed, running, completed, partially-completed, failed, cancelled, rolled-back |
| `requires_elevation` | bool | Whether permissions are required |
| `created_at` / `finished_at` | time | Boundaries |
| `result_summary` | struct | Successes/errors/size |

**OperationTarget**: `node_id`, `path`, `size`, `status`, `error`.

**Transitions**: `planned → confirmed → running → completed | partially-completed | failed`;
`running → cancelled`; `completed | partially-completed → rolled-back` (if reversible).

**Rules**:
- A transition to `running` is impossible without `confirmed`.
- For `kind = shred`, `delete_mode` is always `permanent`.
- For `kind = delete`, `delete_mode` is required (user's choice).

## 12. JournalEntry (Journal Entry)

| Field | Type | Description |
|------|-----|----------|
| `id` | u64 | Auto-increment |
| `operation_id` | UUID | Operation |
| `timestamp` | time | When |
| `user` | string | Account |
| `summary` | string | Short description |
| `affected_count` | u32 | Number of objects |
| `affected_bytes` | u64 | Size |
| `result` | enum | success, partial, error |
| `undo_available` | bool | Whether undo is available |
| `details` | JSON | Details |

## 13. QuarantineItem (Quarantine Item)

| Field | Type | Description |
|------|-----|----------|
| `id` | UUID | Identifier |
| `original_path` | string | Original path |
| `stored_path` | string | Path in quarantine |
| `volume_id` | UUID | Source volume |
| `size` | u64 | Size |
| `deleted_at` | time | Date placed |
| `expires_at` | time | Retention period |
| `reason` | string | Reason |
| `status` | enum | stored, restored, purged, corrupted |
| `checksum` | string | Checksum for integrity verification |

## 14. Report

| Field | Type | Description |
|------|-----|----------|
| `id` | UUID | Identifier |
| `format` | enum | csv, xml, html, xlsx |
| `scope` | struct | What is included (tree, top, types, duplicates, owners) |
| `source_session_id` | UUID | Source session |
| `path` | string | Report file |
| `created_at` | time | Date |
| `row_count` | u64 | Row count |
| `total_bytes` | u64 | Total size |

## 15. AutomationProfile (Automation Profile)

| Field | Type | Description |
|------|-----|----------|
| `id` | UUID | Identifier |
| `name` | string | Profile name |
| `command` | string | CLI command |
| `args` | list | Arguments |
| `schedule` | struct? | Schedule description (for export to Task Scheduler) |
| `last_run` | time? | Last run |
| `last_exit_code` | i32? | Exit code |
| `log_path` | string | Log path |

## 16. DiskEvent (Monitoring Event)

| Field | Type | Description |
|------|-----|----------|
| `timestamp` | time | When |
| `process_id` | u32? | PID |
| `process_name` | string? | Process name |
| `path` | string? | Write path |
| `bytes_written` | u64 | Size |
| `rate_bytes_per_sec` | u64 | Rate |
| `source` | enum | etw, io-counters, usn |

## 17. MetadataRecord (Metadata)

| Field | Type | Description |
|------|-----|----------|
| `node_id` | u32 | File |
| `kind` | enum | exif, audio, video, pe |
| `tags` | string map | Tags and values |
| `read_at` | time | When read |
| `status` | enum | ok, unsupported, corrupted |

## 18. OwnerSummary (Owner Summary)

| Field | Type | Description |
|------|-----|----------|
| `owner_id` | u32 | SID index |
| `sid` | string | SID |
| `display_name` | string | Account name |
| `total_bytes` | u64 | Total size |
| `file_count` | u64 | File count |

## 19. ArchiveEntry (Archive Entry)

| Field | Type | Description |
|------|-----|----------|
| `archive_node_id` | u32 | Archive node |
| `internal_path` | string | Path inside the archive |
| `size` | u64 | Size |
| `compressed_size` | u64? | Compressed size |
| `is_directory` | bool | Directory |
| `encrypted` | bool | Encrypted |
| `share` | f64 | Share of the archive contents |

## 20. Settings

| Field | Type | Description |
|------|-----|----------|
| `language` | enum | ru, en |
| `theme` | enum | light, dark, system |
| `units` | enum | binary, decimal |
| `exclusions` | list of masks | Excluded paths |
| `duplicate_keep_rule` | enum | oldest, newest, shortest-path, original-dir |
| `outdated_threshold_days` | u32 | Staleness threshold |
| `monitor_threshold` | struct | Notification threshold and interval |
| `quarantine_retention_days` | u32 | Quarantine retention period |
| `context_menu_enabled` | bool | Enable Explorer entries (false by default) |
| `confirm_destructive_actions` | bool | Always confirm (true by default) |

The data directory is not a setting: it is always `<exe_dir>\data`, so `%APPDATA%`,
`%LOCALAPPDATA%`, `%ProgramData%` and temporary directories do not participate in the model.

## Relationships Between Entities

```text
Volume 1 ─── * ScanSession 1 ─── * FsNode
ScanSession 1 ─── 0..1 Snapshot 1 ─── * SnapshotDiff
FsNode 1 ─── * FsLink
FsNode 1 ─── * FileLock
FsNode * ─── * DuplicateGroup (via DuplicateMember)
FsNode 1 ─── * CleanupCandidate
FsNode 1 ─── * MetadataRecord
FsNode 1 ─── * ArchiveEntry
Volume 1 ─── * OwnerSummary
Operation 1 ─── * JournalEntry
Operation 1 ─── * QuarantineItem
AutomationProfile 1 ─── * Report
ScanSession 1 ─── * Report
DiskEvent * ─── 0..1 FsNode (by path)
Settings 1 ─── 1 application
```

## Integrity and Validation Rules

- The sum of `SizeMetrics` of child nodes MUST equal the parent's metrics, accounting for excluded and
  inaccessible objects (for `method = exact`).
- An operation cannot be performed on a node with `access = denied` without elevation.
- Quarantine cannot occupy more free space than is available; if space is insufficient, the operation
  requires a user choice.
- All timestamps are stored in UTC and displayed in local time.
- Paths are stored in full form (verbatim if necessary) and are not truncated in the journal.
- A `Snapshot` with a mismatched `volume.id` does not participate in comparison.
- `Settings.confirm_destructive_actions` cannot be disabled for the shredder and permission changes.
