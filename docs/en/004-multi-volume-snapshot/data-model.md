# Data Model: Multi-volume snapshot

**Feature**: `004-multi-volume-snapshot` | **Date**: 2026-11-20

## 1. Scan root (ScanRoot)

Describes one path selected by the user inside a request.

| Field | Type | Rule |
|---|---|---|
| `path` | path string | Absolute, non-empty; 1..=`MAX_PATH_CHARS` |
| `volume` | `Volume` | Resolved after validation and canonicalisation |
| `method` | `ScanMethod` | `mft` for an NTFS volume root, otherwise `walk`; `auto` may fall back |
| `state` | `ok` / `failed` / `cancelled` | Root result |
| `error` | `UiError?` | Set only for `failed` |

Invariants:

- Paths are unique by the platform key.
- No path is an ancestor of another.
- The number of paths is between 1 and 32.

## 2. Scan request (ScanRequest)

| Field | Type | Change |
|---|---|---|
| `paths` | `string[]` | New; replaces `path` |
| `method` | `auto` / `mft` / `walk` | Unchanged |
| `incremental` | bool | Unchanged; a full-scan warning is emitted for several roots |
| `exclusions` | `string[]` | Unchanged; shared by every root |
| `include_ads` | bool | Unchanged; enrichment runs once after all roots |
| `metric` | `logical` / `on_disk` / `physical` | Unchanged |

## 3. Scan task (Task)

| Field | Type | Change |
|---|---|---|
| `task_id` | string | Unchanged |
| `kind` | `scan` | Unchanged |
| `state` | `queued` / `running` / `cancelling` / `cancelled` / `completed` / `partial` / `failed` | `partial` when at least one root fails |
| `volume_id` | string? | First volume; kept for compatibility |
| `volume_ids` | string[] | New; every reserved volume |
| `progress` | `TaskProgress?` | Overall percentage, current path |
| `error` | `UiError?` | Set only on total failure |

Transitions:

```
queued → running → completed
                 ├→ partial   (at least one root succeeded, at least one failed)
                 ├→ cancelled (user cancelled)
                 └→ failed    (no root succeeded)
```

## 4. Scan result (ScanOutcome)

| Field | Type | Rule |
|---|---|---|
| `tree` | `FsTree` | One arena with several roots |
| `roots` | `RootScanOutcome[]` | One record per successful root |
| `failures` | `ScanFailure[]` | Failed root paths and causes |
| `warnings` | `string[]` | Includes MFT-to-walk fallback and root errors |
| `issues` | `ScanIssue[]` | Non-fatal problems from every root |
| `cancelled` | bool | True when cancellation stopped the scan |
| `ads_streams` | u64 | ADS enrichment over the whole arena |

`RootScanOutcome`:

| Field | Type |
|---|---|
| `session` | `ScanSession` (one root) |
| `method` | `ScanMethod` |
| `volume` | `Volume` |
| `excluded` | u64 |
| `issues` | `ScanIssue[]` |
| `warnings` | `string[]` |
| `cancelled` | bool |

## 5. Snapshot (TreeSnapshot)

| Field | Type | Change |
|---|---|---|
| `tree` | `FsTree` | Unchanged; may contain several roots |
| `generation` | u64 | Unchanged |
| `volumes` | `Volume[]` | New; unique volumes in root order |
| `sessions` | `ScanSession[]` | New; one per successful root |
| `roots` | `NodeId[]` | Unchanged; cached `tree.roots()` |

Accessors:

- `volume()` → `volumes[0]` (compatibility);
- `session()` → `sessions.first()` (compatibility);
- `volumes()` → all volumes;
- `sessions()` → all sessions;
- `volume_for_path(path)` → volume with the longest matching mount point;
- `root_id()` → first root (compatibility);
- `root_paths()` → every root path.

Invariants:

- `volumes` is not empty; duplicate volumes are collapsed.
- Every `NodeId` in `roots` belongs to `tree`.
- The generation is monotonic and unique for the snapshot.

## 6. Session summary (ScanSessionSummary)

One record per root; fields are unchanged. `volume_id` is that root's volume and `roots` is one path.
`scan_sessions` returns records for every root, newest first.

## 7. Scan events

`scan-progress`:

- `progress.processed` — nodes of the current root;
- `progress.total` — estimate for the current root;
- `phase_key` — current root phase;
- `current_path` — current root path;
- `bytes_per_second` — current root speed.

`scan-completed` → `ScanSummary`:

| Field | Type | Change |
|---|---|---|
| `nodes_count` | u64 | Sum over roots |
| `method` | string | Single method or `mixed` |
| `methods` | string[] | New |
| `roots` | string[] | Every root |
| `volumes` | string[] | New; mount points |
| `warnings` | string[] | Includes root errors |
| `issues_count` | u64 | Sum |
| `elapsed_ms` | u64 | Total time |

## 8. Storage and compatibility

- Settings, journal and SQLite are unchanged.
- `.fssnap` and reports stay at version 1: primary volume = `volumes[0]`, `roots` = every root.
- `snapshot_volumes(generation)` is new; `volumes_current` is compatible and returns the first volume.
