# Contract: FreeSpace Command Line

**Feature**: `001-freespace-disk-analyzer` | **Version**: 1 | **Date**: 2026-11-15

The CLI uses the same core as the GUI and MUST provide feature parity (FR-074…FR-078).

## General Rules

- Format: `freespace <command> [subcommand] [arguments] [options]`.
- Global options: `--json`, `--quiet`, `--yes`, `--no-color`, `--lang <ru|en>`, `--config <path>`,
  `--log-level <error|warn|info|debug>`.
- `--json` switches output to a machine-readable envelope (see below); human-readable output
  goes to stdout, errors go to stderr.
- Destructive commands in non-interactive mode MUST require `--yes`; without it the command
  exits with code `5` and changes nothing.
- Paths are accepted both regular and long (verbatim); the shell passes them as-is.
- All commands are idempotent with respect to reads; destructive operations are not repeated automatically.

## JSON Envelope

```json
{
  "version": "1",
  "command": "scan",
  "status": "ok | error | partial | cancelled",
  "data": {},
  "warnings": [],
  "errors": [],
  "duration_ms": 1234
}
```

- `data` — the command result; the schema depends on the command.
- `warnings` — non-fatal issues (for example, inaccessible directories).
- `errors` — fatal and partial errors with the fields `code`, `message`, `path`.
- JSON MUST be valid UTF-8 and MUST NOT contain service messages in stdout.

## Commands

### `scan`

```text
freespace scan <path> [--method auto|mft|walk] [--incremental] [--exclude <glob>]...
                      [--snapshot-out <file>] [--json]
```

- `data`: `{ "session_id", "volume", "nodes_count", "total": SizeMetrics, "roots": [...] }`.
- `--incremental` applies only to NTFS and only when a saved journal state exists.

### `tree`

```text
freespace tree <path> [--depth <n>] [--min-size <bytes>] [--json]
```

- `data`: hierarchical list of nodes `{name, path, kind, size{...}, children[]}`.

### `top`

```text
freespace top <path> --limit <n> [--json]
```

- `data`: `{ "items": [{ "path", "size", "modified" }] }`.

### `types`

```text
freespace types <path> [--json]
```

- `data`: `{ "items": [{ "extension", "category", "count", "total_size", "share" }] }`.

### `filter`

```text
freespace filter <path> [--kind <file|dir|link>] [--min-size <bytes>] [--older-than <days>]
                       [--type <ext,...>] [--min-resolution <WxH>] [--min-bitrate <kbps>]
                       [--owner <account>] [--json]
```

- `data`: `{ "items": [{ "path", "size", "metadata": {...} }] }`.

### `duplicates`

```text
freespace duplicates <path> [--min-size <bytes>] [--algorithm sha256|md5]
                              [--keep-rule oldest|newest|shortest|original-dir]
                              [--json]
```

- `data`: `{ "groups": [{ "id", "hash", "size", "reclaimable_bytes", "members": [...] }] }`.
- The search does not perform deletion; use `delete` to delete.

### `cleanup`

```text
freespace cleanup suggest <path> [--older-than <days>] [--json]
freespace cleanup empty-dirs <path> [--json]
```

- `data`: `{ "candidates": [{ "path", "category", "reclaimable_bytes", "risk" }] }`.

### `unlock`

```text
freespace unlock <path> [--list] [--soft] [--force] [--yes] [--json]
```

- Without `--soft/--force` it works as `--list`.
- `--force` requires `--yes`; it terminates the process after confirmation.

### `delete`

```text
freespace delete <path>... --mode recycle|quarantine|permanent [--yes] [--json]
```

- `--mode` is required (the user's decision from the UI/CLI).
- For `quarantine`, insufficient space results in error `E_INSUFFICIENT_SPACE` with no partial execution.

### `shred`

```text
freespace shred <path>... --passes <n> --pattern zeros|random [--yes] [--json]
```

- `--passes ≥ 1`; on an SSD a warning is added to `warnings` and TRIM is performed.

### `rename`

```text
freespace rename <path>... --regex <pattern> --replacement <text>
                           [--case lower|upper|preserve] [--preview] [--yes] [--json]
```

- Without `--yes`, only a preview is performed (`status = ok`, `data.preview = true`).
- On conflicts — `E_CONFLICT`, nothing is changed.

### `attrs`

```text
freespace attrs <path>... --set read-only,hidden --clear archive [--yes] [--json]
```

### `acl`

```text
freespace acl <path>... --owner <account> | --reset | --copy-from <path> [--yes] [--json]
```

### `snapshot`

```text
freespace snapshot create <path> --out <file.fssnap> [--json]
freespace snapshot diff <base.fssnap> <target.fssnap> [--export <file>] [--json]
```

### `export`

```text
freespace export <path> --format csv|xml|html|xlsx --out <file> [--include tree,top,types,duplicates,owners] [--json]
```

### `archives`

```text
freespace archives <path> [--json]
```

- `data`: `{ "items": [{ "archive", "entries": [{ "path", "size", "compressed_size" }] }] }`.

### `monitor`

```text
freespace monitor [--threshold <bytes>] [--interval <seconds>] [--json]
```

- Streaming JSON Lines: one `DiskEvent` object per line.

### `drives`

```text
freespace drives [--json]
```

### `config`

```text
freespace config get <key> [--json]
freespace config set <key> <value> [--json]
```

### `context-menu`

```text
freespace context-menu install [--machine] [--yes]
freespace context-menu uninstall [--machine] [--yes]
```

### `schedule`

```text
freespace schedule export <profile> --out <file.xml>
```

- Creates an XML task for the Windows Task Scheduler.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Access denied |
| 4 | Object not found |
| 5 | Operation cancelled by the user or confirmation missing |
| 6 | Conflicts detected (names, permissions, snapshots) |
| 7 | Partial success (some objects processed) |
| 8 | Unsupported format or capability |
| 9 | Insufficient free space |
| 10 | Object is locked by another process |

## Stability

- JSON schemas and exit codes are stable within a major version.
- New fields may be added without changing the major version; consumers MUST ignore
  unknown fields.
- Removing or renaming commands — only in a major version.
