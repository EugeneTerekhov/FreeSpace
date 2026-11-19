# Contract: `.fssnap` Snapshot Format

**Feature**: `001-freespace-disk-analyzer` | **Version**: 1 | **Date**: 2026-11-15

A snapshot is an immutable local file with disk state for later comparison
(FR-038…FR-041, SC-008).

## Container

```text
+----------------+---------+-----------------------------+
| Magic "FSSNAP" | u32 ver | zstd-compressed CBOR envelope |
+----------------+---------+-----------------------------+
```

- Magic: 8 ASCII bytes `FSSNAP\0\0`.
- `ver`: u32 little-endian, container version (starts at `1`).
- Next comes a zstd frame containing a CBOR document `SnapshotPayload`.
- The SHA-256 checksum is computed over the uncompressed CBOR and stored in the metadata.

## SnapshotPayload (CBOR, schema v1)

```text
SnapshotPayload {
  format_version: u32,          // 1
  app_version: string,
  created_at: string,           // ISO 8601 UTC
  checksum: string,             // SHA-256 of the uncompressed CBOR
  volume: {
    id: string,
    mount_point: string,
    label: string,
    filesystem: string,
    total_bytes: u64,
    free_bytes: u64,
    cluster_size: u32,
    scan_method: "mft" | "usn" | "walk",
    elevated: bool
  },
  roots: [string],
  node_count: u64,
  settings: {
    units: "binary" | "decimal",
    follow_links: bool,
    include_ads: bool
  },
  nodes: ColumnarNodes
}
```

## ColumnarNodes

A columnar representation of the tree for compactness and streaming comparison. All arrays have
length `node_count`; indexes are u32.

| Array | Type | Description |
|-------|------|-------------|
| `parent` | [u32] | Parent index; `0xFFFFFFFF` — root |
| `name_offset` | [u32] | Offset of the name in `name_blob` |
| `name_length` | [u16] | Name length in UTF-8 bytes |
| `name_blob` | bytes | Concatenation of names |
| `kind` | [u8] | 0=file, 1=dir, 2=symlink, 3=junction, 4=hardlink |
| `logical_size` | [u64] | Logical size |
| `on_disk_size` | [u64] | Size on disk |
| `physical_size` | [u64] | Physical size |
| `mtime` | [i64] | Unix time (UTC, seconds) |
| `attributes` | [u32] | Attribute bitmask |
| `file_ref` | [u64] | NTFS FRN (0 if unknown) |
| `access` | [u8] | 0=ok, 1=denied, 2=error |
| `owner_id` | [u32] | Index into `owners`; `0xFFFFFFFF` — unknown |
| `owners` | [string] | Table of owner SIDs/names |

## Compatibility Rules

- A snapshot is read if `format_version` is supported by the application.
- Comparison is allowed only when `volume.id` matches; otherwise — error `E_INCOMPATIBLE_SNAPSHOT`.
- Newer `format_version` values are rejected with a clear message; format conversion is not
  performed automatically.
- Checksum corruption → error `E_CORRUPTED_SNAPSHOT`, the file is not modified.
- A snapshot MUST NOT contain file contents or full ACLs; only aggregated metadata.

## Comparison

- Merging two node streams by relative paths (from the root) without loading all nodes into memory.
- Result: `added`, `removed`, `changed`, `growth_by_directory`.
- For directories, growth is computed by `physical_size` (by default) with the option to switch to
  logical size.
- The comparison report is exported to the formats from `contracts/report-formats.md`.

## Versioning

- `format_version` is incremented on an incompatible schema change.
- Adding new columns is permitted while preserving backward compatibility and raising the
  application's minor version.
