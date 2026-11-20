# CLI Contract: Multi-volume snapshot

**Feature**: `004-multi-volume-snapshot` | **Version**: 2

## 1. `freespace scan <PATH>...`

Accepts one or several positional paths. One path keeps the previous behaviour; several paths are
scanned sequentially into one snapshot.

```text
freespace scan C:\ D:\ --json
freespace scan D:\Media --method walk --json
freespace scan C:\ --snapshot-out C:\temp\scan.fssnap --json
```

Rules:

- 1..=32 paths;
- relative, duplicate and overlapping paths → `E_INVALID_ARGUMENT` / `E_VALIDATION` (exit code 2),
  no scan starts;
- `--exclude` applies to every root;
- `--incremental` with several roots emits a warning and performs a full scan;
- `--snapshot-out` records the primary (first) volume and every root.

### JSON

```json
{
  "session_id": "…",
  "volumes": [
    {
      "id": "…C:",
      "mount_point": "C:\\",
      "label": "System",
      "file_system": "NTFS",
      "total_bytes": 1000000000000,
      "free_bytes": 250000000000
    },
    {
      "id": "…D:",
      "mount_point": "D:\\",
      "label": "Data",
      "file_system": "NTFS",
      "total_bytes": 2000000000000,
      "free_bytes": 500000000000
    }
  ],
  "nodes_count": 1200000,
  "total": {
    "logical": 1200000000000,
    "on_disk": 1250000000000,
    "physical": 1240000000000,
    "ads": 0
  },
  "roots": ["C:\\", "D:\\"],
  "methods": ["mft", "mft"],
  "method": "mft",
  "excluded": 42,
  "issues": []
}
```

- `volume` (singular) remains for one path for one major cycle; new code reads `volumes`.
- `method` is the single method or `mixed`.
- `total` is the sum over every root.
- `nodes_count` is the sum over every root.
- Exit codes: `0` — success or partial success; `1` — failure or no root scanned; `2` — argument
  error.

## 2. `freespace tree <PATH>...`

Accepts one or several paths and prints every root.

```text
freespace tree C:\ D:\ --depth 2 --min-size 104857600 --json
```

### JSON

```json
{
  "method": "mft",
  "session_id": "…",
  "nodes_count": 1200000,
  "total": {
    "logical": 1200000000000,
    "on_disk": 1250000000000,
    "physical": 1240000000000,
    "ads": 0
  },
  "roots": [
    { "name": "C:\\", "path": "C:\\", "kind": "directory", "children": [] },
    { "name": "D:\\", "path": "D:\\", "kind": "directory", "children": [] }
  ]
}
```

- `roots` contains every root in argument order.
- `total` is the sum over every root.
- Human-readable output prints the roots in order.

## 3. Compatibility

- One path: previous fields are kept; `volumes` and `methods` are added.
- Several paths: previously the command accepted exactly one path, so the behaviour is extended, not
  broken.
- `--json` remains valid JSON in both modes.
