# Contract: FreeSpace Report Formats

**Feature**: `001-freespace-disk-analyzer` | **Version**: 1 | **Date**: 2026-11-15

Reports contain the same set of data and differ only in presentation. Any format MUST
keep totals consistent with the application (FR-070…FR-073, SC-009).

## Common Metadata

Each report contains a metadata section:

| Field | Description |
|-------|-------------|
| `report_version` | Report schema version (`1`) |
| `generated_at` | Date and time (ISO 8601, UTC) |
| `app_version` | Application version |
| `volume` | Volume: label, letter, file system, scan method |
| `roots` | Analyzed scopes |
| `session_id` | Session identifier |
| `units` | `binary` or `decimal` |
| `sections` | Included sections |

## Section `tree`

A hierarchical list of nodes.

| Column | Type | Description |
|--------|------|-------------|
| `path` | string | Full path |
| `name` | string | Name |
| `kind` | string | file/dir/link |
| `logical_size` | u64 | Logical size |
| `on_disk_size` | u64 | Size on disk |
| `physical_size` | u64 | Physical size |
| `files` | u64 | Number of files in the subtree |
| `dirs` | u64 | Number of directories in the subtree |
| `modified` | ISO 8601 | Modification date |
| `attributes` | string | Comma-separated attributes |
| `owner` | string | Owner |

## Section `top`

| Column | Type |
|--------|------|
| `rank` | u32 |
| `path` | string |
| `size` | u64 |
| `modified` | ISO 8601 |

## Section `types`

| Column | Type |
|--------|------|
| `extension` | string (`<none>` for files without an extension) |
| `category` | string (video/archive/image/audio/document/system/other) |
| `count` | u64 |
| `total_size` | u64 |
| `share` | f64 (0..1) |

## Section `duplicates`

| Column | Type |
|--------|------|
| `group_id` | string |
| `hash_algorithm` | string |
| `hash` | string |
| `size` | u64 |
| `path` | string |
| `keep` | bool |
| `selected` | bool |
| `reclaimable_bytes` | u64 (in the first row of the group) |

## Section `owners`

| Column | Type |
|--------|------|
| `owner` | string |
| `sid` | string |
| `files` | u64 |
| `total_size` | u64 |

## CSV

- UTF-8 with BOM encoding (so the file opens correctly in Excel).
- The delimiter is a comma; values containing delimiters, quotes and line breaks are escaped.
- The first row contains headers; with multiple sections, each section is formatted as a separate
  block with a `# section: <name>` header row and a blank line between blocks.
- Numbers without thousands separators; sizes in bytes; dates in ISO 8601.

## XML

```xml
<?xml version="1.0" encoding="UTF-8"?>
<freespace-report version="1">
  <meta>
    <generated-at>2026-11-15T12:00:00Z</generated-at>
    <app-version>1.1.0</app-version>
    <volume letter="C" label="System" filesystem="NTFS" scan-method="mft"/>
    <roots><root>C:\</root></roots>
  </meta>
  <sections>
    <section name="tree">
      <node path="C:\" kind="dir" logical-size="0" on-disk-size="0" files="0" dirs="0"/>
    </section>
    <section name="top">...</section>
    <section name="types">...</section>
    <section name="duplicates">...</section>
    <section name="owners">...</section>
  </sections>
  <totals logical-size="0" on-disk-size="0" physical-size="0"/>
</freespace-report>
```

- The schema is described in a documented XSD-like form; element names are in kebab-case.
- All sizes are integer bytes; `share` is a floating-point number.

## HTML

- Self-contained file: embedded styles, no external resources or network requests.
- Sections: summary (volume, file count, method), tree (collapsible), top files, types,
  duplicates, owners.
- Escaping of all user strings is mandatory.
- For large reports the tree is limited to a configurable depth; the full list is available in CSV.

## XLSX

Sheets:

1. `Summary` — metadata and totals.
2. `Tree` — the `tree` section.
3. `TopFiles` — the `top` section.
4. `Types` — the `types` section.
5. `Duplicates` — the `duplicates` section.
6. `Owners` — the `owners` section.

Requirements: frozen header row, auto-filter, numeric format for sizes, dates in
ISO 8601 format, no formulas, correct opening in Microsoft Excel and LibreOffice Calc.

## Constraints

- Reports do not contain file contents, whole-file hashes (except for the duplicates section) or
  personal data other than owner names.
- When exporting a report with millions of rows, writing MUST be streaming; the application MUST NOT
  exhaust memory.
