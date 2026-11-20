# IPC Contract: Multi-volume snapshot

**Feature**: `004-multi-volume-snapshot` | **Version**: IPC 2

`IPC_VERSION` is raised from 1 to 2: `scan_start` no longer accepts `path`.

## 1. `scan_start`

Starts one scan of one or several roots.

### Request

```json
{
  "request": {
    "paths": ["C:\\", "D:\\"],
    "method": "auto",
    "incremental": false,
    "exclusions": ["**/node_modules"],
    "include_ads": false,
    "metric": "on_disk"
  }
}
```

Rules:

- `paths` is 1..=32 absolute paths;
- relative, empty, duplicate and overlapping paths → `E_VALIDATION`;
- at least one reserved volume → `E_BUSY`;
- a new scan does not cancel the current snapshot before at least one root succeeds.

### Response

```json
{
  "task_id": "6f1c…",
  "generation": 12
}
```

## 2. `scan_status`

Unchanged, except `Task.volume_ids` contains every reserved volume and `volume_id` is the first one.

```json
{
  "task_id": "6f1c…",
  "kind": "scan",
  "state": "running",
  "volume_id": "…C:",
  "volume_ids": ["…C:", "…D:"],
  "progress": {
    "processed": 120000,
    "total": 480000,
    "current_path": "D:\\Media",
    "bytes_per_second": 125000000,
    "items_per_second": 900,
    "phase_key": "scan.phase.building",
    "updated_at": 1790000000000
  },
  "started_at": 1790000000000,
  "finished_at": null,
  "cancelable": true,
  "error": null
}
```

## 3. `scan_cancel`

Unchanged: cancels the current root and every remaining root.

## 4. `scan_sessions`

Returns one record per root, newest first. `volume_id` is the root's volume and `roots` is one path.
The record shape is unchanged.

## 5. `snapshot_volumes` (new)

Returns the volumes of the requested generation in root order.

### Request

```json
{ "generation": 12 }
```

### Response

```json
[
  {
    "volume_id": "…C:",
    "mount_point": "C:\\",
    "label": "System",
    "file_system": "NTFS",
    "total_bytes": 1000000000000,
    "free_bytes": 250000000000,
    "used_bytes": 750000000000,
    "is_removable": false,
    "status": "available",
    "scan_method": "mft",
    "is_elevated": true
  },
  {
    "volume_id": "…D:",
    "mount_point": "D:\\",
    "label": "Data",
    "file_system": "NTFS",
    "total_bytes": 2000000000000,
    "free_bytes": 500000000000,
    "used_bytes": 1500000000000,
    "is_removable": false,
    "status": "available",
    "scan_method": "mft",
    "is_elevated": true
  }
]
```

Errors: `E_NOT_FOUND` when no snapshot exists; `E_STALE_CURSOR` when the generation is not current.

## 6. `volumes_current`

Compatible: returns the first volume of the current snapshot. New code uses `snapshot_volumes`.

## 7. `tree_root`, `tree_children`, `tree_search`, `tree_node`, `tree_path`

The shape is unchanged. `tree_root` returns a page of every root; each row has `parent_id = null`,
`kind = "directory"` and the mount point as `path`.

## 8. `scan-progress` event

The shape is unchanged; `current_path` and counters belong to the current root. The overall
percentage is monotonic and computed as `(index + local) / roots_count`.

## 9. `scan-completed` event

### Payload

```json
{
  "task_id": "6f1c…",
  "state": "partial",
  "generation": 12,
  "summary": {
    "nodes_count": 1200000,
    "method": "mixed",
    "methods": ["mft", "directory-walk"],
    "roots": ["C:\\", "D:\\"],
    "volumes": ["C:\\", "D:\\"],
    "warnings": ["D:\\: access denied"],
    "issues_count": 7,
    "elapsed_ms": 42000
  },
  "error": null
}
```

`state`:

- `completed` — every root succeeded and there are no issues;
- `partial` — at least one root succeeded but there are errors or issues;
- `failed` — no root was scanned; `summary = null` and `error` is set;
- `cancelled` — cancellation.

## 10. Compatibility

- IPC 1 clients are incompatible: the `path` field is removed.
- `volumes_current` stays for one major cycle.
- `Task.volume_id` stays; new clients read `volume_ids`.
