# FreeSpace CLI contract

**Contract version**: 1  
**Applies to**: FreeSpace 2.0.x  
**Source of truth**: `specs/001-freespace-disk-analyzer/contracts/cli.md` and
`specs/003-cross-platform-portable/contracts/`

The CLI uses the same core crates as the GUI. Every GUI capability has a CLI equivalent with
machine-readable output; the CLI may provide more capabilities than the GUI. The interface is fully
offline and never sends telemetry.

## Invocation

```text
freespace <command> [subcommand] [arguments] [options]
```

Global options:

| Option | Meaning |
|--------|---------|
| `--json` | Emit the JSON envelope instead of human-readable text |
| `--quiet` | Suppress human-readable output |
| `--yes` | Confirm a destructive operation non-interactively |
| `--no-color` | Disable ANSI colours |
| `--lang <ru\|en>` | Interface language |
| `--config <path>` | Use an explicit settings file |
| `--log-level <error\|warn\|info\|debug>` | Diagnostic verbosity |
| `--lock-timeout <seconds>` | Wait for another FreeSpace process (default 30) |

Human-readable text goes to stdout and warnings/errors to stderr. With `--json`, stdout contains
exactly one valid UTF-8 JSON document, except for `monitor`, which streams JSON Lines as documented
below.

## JSON envelope

```json
{
  "version": "1",
  "command": "scan",
  "status": "ok",
  "data": {},
  "warnings": [],
  "errors": [],
  "duration_ms": 1234
}
```

`status` is `ok`, `error`, `partial` or `cancelled`. Each error carries a stable `code` (`E_*`), a
`message` and, when applicable, a `path`. New fields may be added within the major version;
consumers must ignore unknown fields.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General failure (I/O, configuration, corrupted snapshot) |
| 2 | Invalid argument or path |
| 3 | Access denied or protected system path |
| 4 | Object not found |
| 5 | Confirmation missing or operation cancelled |
| 6 | Conflict (names, permissions, snapshots) |
| 7 | Partial success |
| 8 | Unsupported format or capability (`E_UNSUPPORTED`) |
| 9 | Insufficient free space |
| 10 | Object or run locked by another process |

Destructive commands require `--yes` in non-interactive mode. Without it the command exits with
code `5` and changes nothing.

## Commands

### `drives`

```text
freespace drives [--json]
```

`data`: `{ "items": [{ "mount_point", "label", "file_system", "total_bytes", "free_bytes", "used_bytes", "cluster_size", "serial_number", "drive_type", "is_removable", "status" }] }`.

Windows enumerates volumes through WinAPI, and Linux uses the mount table and `statvfs`. Inaccessible
volumes are reported with `"status": "inaccessible"` instead of being dropped.

### `capabilities`

```text
freespace capabilities [--json]
```

Describes the capabilities of the current platform. `data` is a `Capabilities` object:

```json
{
  "platform": "linux",
  "scan_methods": { "mft": false, "usn": false, "walk": true },
  "trash": true,
  "permanent": true,
  "shred": false,
  "acl": false,
  "attrs": false,
  "ads": false,
  "unlock": false,
  "context_menu": false,
  "monitor": true,
  "elevation": true,
  "long_paths": false,
  "case_sensitive": true,
  "max_path": 4096
}
```

| Field | Meaning |
|-------|---------|
| `platform` | `windows`, `linux` or `other` |
| `scan_methods` | Available methods: `mft`, `usn`, `walk` |
| `trash` | System trash: Recycle Bin or XDG trash |
| `permanent` | Irreversible deletion |
| `shred` | Overwriting, Windows CLI only |
| `acl` | NTFS owner and permissions |
| `attrs` | Windows file attributes |
| `ads` | NTFS alternate data streams |
| `unlock` | Restart Manager lock release |
| `context_menu` | Explorer context-menu integration |
| `monitor` | Write monitoring |
| `elevation` | Elevation detection and request |
| `long_paths` | Long-path support |
| `case_sensitive` | File names are case-sensitive |
| `max_path` | Path length limit in characters, or `null` |

Windows reports `mft: true`, `usn: true`, `shred: true`, `acl: true`, `attrs: true`, `ads: true`,
`unlock: true`, `long_paths: true`, `case_sensitive: false`, `max_path: null`. Linux reports only
`walk`, `trash`, `permanent`, `monitor`, `elevation`, `case_sensitive: true` and `max_path: 4096`;
all other capabilities are `false`. An unsupported command returns `E_UNSUPPORTED` and exit code
`8`.

### `config`

```text
freespace config get <key> [--json]
freespace config set <key> <value> [--json]
```

Supported keys: `language`, `theme`, `units`, `duplicate_keep_rule`, `outdated_threshold_days`,
`confirm_destructive_actions`, `monitor_threshold_bytes`, `monitor_interval_secs`, `exclusions`,
`context_menu_enabled`. The legacy `quarantine_retention_days` key is ignored when an older
`settings.toml` still contains it.

`context_menu_enabled` is `false` by default. `config set context_menu_enabled true` registers the
Explorer entries immediately; `false` removes them. In a portable build (`--no-default-features`)
the key stays readable but cannot be enabled, and the `context-menu` command does not exist.

### `scan`

```text
freespace scan <path> [--method auto|mft|walk] [--incremental]
                      [--exclude <glob>]... [--snapshot-out <file>]
                      [--include-ads] [--size-mode logical|on-disk|physical]
                      [--json]
```

`data`: `{ "session_id", "volume", "nodes_count", "total": SizeMetrics, "roots": [...] }`.

On Windows `auto`, `mft` and `walk` are available, and `--incremental` updates a previous NTFS scan
through the USN journal. On Linux only the directory walk is used; `--method mft` and `--incremental`
return `E_UNSUPPORTED`, and the scan method is reported explicitly in the result.

### `tree`

```text
freespace tree <path> [--depth <n>] [--min-size <bytes>] [--json]
```

`data`: hierarchical nodes `{ "name", "path", "kind", "size": SizeMetrics, "children": [] }`.

### `top`

```text
freespace top <path> --limit <n> [--json]
```

`data`: `{ "items": [{ "path", "size", "modified" }] }`.

### `types`

```text
freespace types <path> [--json]
```

`data`: `{ "items": [{ "extension", "category", "count", "total_size", "share" }] }`.

### `filter`

```text
freespace filter <path> [--kind file|dir|link] [--min-size <bytes>]
                       [--older-than <days>] [--type <ext,...>]
                       [--owner <account|unknown>]
                       [--min-resolution <WxH>] [--min-bitrate <kbps>]
                       [--min-duration <seconds>]
                       [--metadata exif,audio,video,pe] [--json]
```

`data`:

```json
{
  "items": [{
    "path": "C:\\Videos\\clip.mp4",
    "kind": "file",
    "size": 123456,
    "modified": "2026-11-16T10:00:00Z",
    "owner": "PC\\Alice",
    "metadata": {
      "kind": "video",
      "status": "ok",
      "tags": { "Width": "3840", "Height": "2160", "Codec": "h264" },
      "read_at": 1763290000000
    }
  }],
  "count": 1
}
```

`--owner` accepts an account name, a SID or `unknown`. The command exits successfully with
`count: 0` and `matched_owner: false` when the account is not present. Metadata constraints exclude
files whose metadata is unsupported or corrupted; without metadata options all matching objects are
returned.

### `duplicates`

```text
freespace duplicates <path> [--min-size <bytes>] [--algorithm sha256|md5]
                              [--keep-rule oldest|newest|shortest|original-dir]
                              [--json]
```

`data`: `{ "groups": [{ "id", "hash", "size", "reclaimable_bytes", "members": [...] }] }`.
Search never deletes anything.

### `cleanup`

```text
freespace cleanup suggest <path> [--older-than <days>] [--json]
freespace cleanup empty-dirs <path> [--json]
```

`data`: `{ "candidates": [{ "path", "category", "reclaimable_bytes", "risk" }] }`.

### `unlock`

```text
freespace unlock <path> [--list] [--soft] [--force] [--yes] [--json]
```

Without `--soft`/`--force` the command lists locking processes. `--force` requires `--yes`. The
command is available on Windows only (Restart Manager); on Linux it returns `E_UNSUPPORTED`.

### `delete`

```text
freespace delete <path>... --mode recycle|permanent [--yes] [--json]
```

`--mode` is mandatory. Exactly two modes are accepted:

- `recycle` — system trash: Recycle Bin on Windows, XDG trash on Linux;
- `permanent` — irreversible deletion.

The removed `quarantine` value fails with `E_INVALID_ARGUMENT` and is never treated as `permanent`.
If trash is unavailable on the volume, the command fails before changing data; permanent deletion is
offered separately and requires its own confirmation. Every deletion goes through a preview, a
confirmation and a journal entry.

### `shred`

```text
freespace shred <path>... --passes <n> --pattern zeros|random [--yes] [--json]
```

The command is available on Windows only; on Linux and other platforms it returns `E_UNSUPPORTED`
(code `8`). On SSD volumes the result contains a warning and TRIM is requested.

### `rename`

```text
freespace rename <path>... --regex <pattern> --replacement <text>
                           [--case lower|upper|preserve] [--preview] [--yes]
                           [--json]
```

Without `--yes` only the preview runs. Conflicts return `E_CONFLICT` and change nothing. Bulk rename
is CLI-only; contextual rename of a single file stays in the GUI tree.

### `attrs` and `acl`

```text
freespace attrs <path>... --set read-only,hidden --clear archive [--yes] [--json]
freespace acl <path>... --owner <account> | --reset | --copy-from <path> [--yes] [--json]
```

Both commands work on Windows only. On Linux they return `E_UNSUPPORTED`; POSIX permissions are not
changed through the CLI.

### `owners`

```text
freespace owners <path> [--json]
```

Builds the per-owner report. `data`:

```json
{
  "items": [{
    "owner_id": 0,
    "sid": "S-1-5-21-…",
    "display_name": "PC\\Alice",
    "total_bytes": 123456,
    "file_count": 42,
    "share": 0.31
  }],
  "count": 1,
  "total_bytes": 123456,
  "unknown_count": 0,
  "error_count": 0
}
```

Objects whose owner cannot be resolved are reported under `unknown`. On Linux the owner is resolved
from `uid`/`gid` and `/etc/passwd`.

### `snapshot`

```text
freespace snapshot create <path> --out <file.fssnap> [--json]
freespace snapshot diff <base.fssnap> <target.fssnap> [--export <file>] [--json]
```

### `export`

```text
freespace export <path> --format csv|xml|html|xlsx --out <file>
                        [--include tree,top,types,duplicates,owners] [--json]
```

`data`: `{ "path", "format", "sections", "row_count", "total_bytes" }`.

All formats come from one report model, so headers, order and totals match. CSV is UTF-8 with a BOM
and one `# section: <name>` block per section; XML follows the documented element names; HTML is
self-contained and escapes user text; XLSX contains `Summary`, `Tree`, `TopFiles`, `Types`,
`Duplicates` and `Owners` sheets. See
`specs/001-freespace-disk-analyzer/contracts/report-formats.md`.

### `archives`

```text
freespace archives <path> [--export <file.csv|json|xml|html>] [--json]
```

`path` may be a single archive or a directory. Supported containers: ZIP, 7z, TAR, TAR/GZ and ISO.
RAR requires the optional `rar` build feature and a compatible library; otherwise the command
returns `E_UNSUPPORTED` without crashing. Password-protected and damaged archives produce a clear
error.

`data`:

```json
{
  "items": [{
    "archive": "C:\\Downloads\\archive.zip",
    "format": "zip",
    "total_size": 12345,
    "file_count": 2,
    "encrypted_count": 0,
    "entries": [{
      "path": "docs/readme.txt",
      "size": 120,
      "compressed_size": 80,
      "is_directory": false,
      "encrypted": false,
      "share": 0.01
    }]
  }],
  "count": 1
}
```

Archive lists export to CSV, JSON, XML and HTML. Resource limits reject archive bombs with
`E_UNSUPPORTED`.

### `schedule`

```text
freespace schedule export <profile> --out <file.xml> [--json]
```

Built-in profiles: `daily-scan`, `weekly-duplicates`, `monthly-export`. The output is a UTF-16LE
Task Scheduler definition with a BOM:

```powershell
freespace schedule export daily-scan --out daily-scan.xml
schtasks /create /tn "FreeSpace daily scan" /xml daily-scan.xml /f
```

Profiles are validated before export. Unknown profiles return `E_NOT_FOUND`. The command targets
Windows.

### `monitor`

```text
freespace monitor [--threshold <bytes>] [--interval <seconds>]
                  [--iterations <n>] [--root <path>] [--no-etw] [--json]
```

Streams JSON Lines instead of the usual single envelope. Each line is one object:

```json
{"type":"write","timestamp_ms":1700000000000,"pid":1234,"process":"backup.exe","bytes":8388608,"paths":["\\Device\\HarddiskVolume3\\data\\video.mkv"],"threshold_exceeded":false}
{"type":"notification","timestamp_ms":1700000000000,"bytes":2147483648,"window_ms":60000,"threshold_bytes":1073741824}
{"type":"usn","timestamp_ms":1700000000000,"file_ref":42,"parent_ref":5,"name":"changed.bin"}
```

On Windows the Kernel-File ETW session (process + path) needs administrator rights. Without them the
command falls back to `GetProcessIoCounters` (process + bytes) and the USN journal tail (path hints).
On Linux it uses `/proc/<pid>/io` counters. `--iterations 0` runs until interrupted; `--interval 0`
polls as fast as the platform allows (100 ms).

### `context-menu`

```text
freespace context-menu install [--machine] [--executable <path>]
freespace context-menu uninstall [--machine] [--all]
```

Installs *Analyze with FreeSpace* for files, folders and drives, plus *Unlock with FreeSpace* for
files and folders. Entries are absent by default: neither unpacking the archive nor a fresh
configuration registers them. A current-user install or removal also updates `context_menu_enabled`, so the GUI
settings screen stays in sync. The default scope is the current user
(`HKCU\Software\Classes`); `--machine` writes to `HKLM`, requires elevation and does not change the
per-user setting; `--all` removes both scopes and disables the setting. Cleanup is idempotent.

Paths with spaces, Unicode characters and trailing separators are quoted for `CommandLineToArgvW`.
A bare path passed to the executable is expanded to `scan <path>`. The command is available on
Windows only; in the portable build (`--no-default-features`) it does not exist.

## Journal and undo

The operation journal is available in the GUI. Undo is supported only for renames
(`operation_undo` / `journal_undo`); deletion is not undone or restored by the application — objects
in the system trash are restored with the operating system tools. Old journal entries with the
`quarantine` and `restore` kinds are read as audit records and do not create new operations.

## Parallel runs

Every command takes an exclusive advisory lock on `<exe_dir>/data/cli.lock`. A second process queues
for `--lock-timeout` seconds and then fails with `E_LOCKED` (exit code `10`). The lock is released
automatically when the process exits, including after a crash. Read-only commands are also
serialised, which keeps JSON logs and the cache consistent.

## Data storage

Settings, the SQLite journal, the hash cache and the run lock live in `<exe_dir>/data`. FreeSpace
never writes to `%APPDATA%`, `%LOCALAPPDATA%`, `%ProgramData%` or a temporary directory. A
`data/quarantine` folder left by an older version is not used and is not deleted automatically;
remove it manually after checking that it holds nothing you need. When the folder is read-only,
settings stay in memory for the session and the command reports an error instead of falling back to
a system location.

## Stability

JSON schemas, command names and exit codes are stable within a major version. Removing or renaming a
command requires a major version bump. New optional fields may appear at any time.
