# Data Model: FreeSpace — a unified native next-generation interface

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

The model describes view entities and IPC boundaries. Domain entities (`Volume`, `FsNode`,
`SizeMetrics`, `ScanSession`, `DuplicateGroup`, `Snapshot`, `Operation`, `JournalEntry`,
`QuarantineItem`, `Settings`) remain in `freespace-core`/`freespace-ops` and are not duplicated
in TypeScript. Rust is the single source of truth; the UI stores only projections and view
state.

## 1. Interface entities

### Screen

An application screen.

| Field | Type | Required | Rules |
|-------|------|----------|-------|
| `id` | enum (`overview`, `tree`, `analysis`, `duplicates`, `cleanup`, `journal`, `shred`, `monitor`, `inspect`, `fileops`, `settings`) | required | Exactly 11 values; an unknown id is a navigation error |
| `group` | enum (`overview`, `analysis`, `cleanup`, `operations`, `tools`, `system`) | required | Sidebar group; determines the navigation order |
| `title_key` | `MessageKey` | required | `nav.*` key |
| `requires_volume` | bool | required | If true and no volume is selected — an empty state with volume selection |
| `requires_scan` | bool | required | If true and there is no data — offer a scan |
| `capabilities` | array of enum | required | Actions available on the screen |
| `cli_analog` | string or null | required | CLI command for the parity matrix |

Relations: `Screen 1 — 0..1 ScreenState`; `Screen 1 — 0..* ColumnState`;
`Screen 1 — 0..* ScreenshotBaseline`; `Screen 1 — 1 ParityMatrix`.

### ScreenState

The state of a specific screen.

| Field | Type | Rules |
|-------|------|-------|
| `screen_id` | `Screen.id` | Required |
| `phase` | enum (`idle`, `loading`, `ready`, `empty`, `partial`, `error`, `refreshing`) | Required |
| `error` | `UiError?` | Required when `phase = error`/`partial` |
| `volume_id` | `VolumeId?` | Required for volume screens |
| `selection` | `SelectionModel` | Empty by default |
| `filters` | `FilterSet` | Empty by default |
| `sort` | `SortSpec?` | A single active key |
| `page` | `PageCursor?` | Reset on filter/sort/generation change |
| `scroll_anchor` | string or null | A stable row id, not a pixel |
| `last_updated` | timestamp | For the freshness indicator |

Transitions:

```text
idle → loading → ready | empty | partial | error
ready → refreshing → ready | partial | error
ready | partial | error → loading   (volume/filter change)
any → idle                           (screen change without preserving data)
```

### ScanTask

A scan/analysis task visible in the shell.

| Field | Type | Rules |
|-------|------|-------|
| `task_id` | UUID | Unique within the session |
| `kind` | enum (`scan`, `duplicates`, `snapshot_compare`, `archive_scan`, `monitor`) | Required |
| `state` | enum (`queued`, `running`, `cancelling`, `cancelled`, `completed`, `partial`, `failed`) | Required |
| `volume_id` | `VolumeId?` | For volume tasks |
| `progress` | `TaskProgress?` | Required when `running` |
| `started_at` / `finished_at` | timestamp | `finished_at ≥ started_at` |
| `cancelable` | bool | false after the point of no return |
| `error` | `UiError?` | When `failed`/`partial` |

Transitions: `queued → running → completed | partial | failed`; `running → cancelling →
cancelled`; `completed → running` is forbidden.

### TaskProgress

| Field | Type | Rules |
|-------|------|-------|
| `processed` | u64 | Monotonically non-decreasing |
| `total` | u64? | May be unknown |
| `current_path` | string? | Truncated in the UI but stored in full |
| `bytes_per_second` | u64? | Non-negative |
| `items_per_second` | u64? | Non-negative |
| `phase_key` | `MessageKey` | Localized phase |
| `updated_at` | timestamp | No more than 20 Hz |

### TreePage

A tree/table page.

| Field | Type | Rules |
|-------|------|-------|
| `generation` | u64 | Grows on a new scan/invalidation |
| `parent_id` | `NodeId?` | null for the root/flat list |
| `cursor` | string? | Opaque, ≤ 512 characters |
| `next_cursor` | string? | null — end |
| `rows` | `NodeRow[]` | 0..2000 |
| `total_estimate` | u64? | An estimate, not an exact counter |
| `metric` | enum (`logical`, `on_disk`, `physical`) | Required |
| `sort` | `SortSpec` | Required |
| `stale_cursor` | bool | true — the UI re-requests the first page |

Rules: `rows.length ≤ page_size`; `next_cursor` is not equal to `cursor`; when
`stale_cursor = true`, the other fields may be empty.

### NodeRow

A projection of `FsNode` for a row.

| Field | Type | Rules |
|-------|------|-------|
| `id` | `NodeId` (u64/string) | Unique within the generation |
| `parent_id` | `NodeId?` | null for the root |
| `name` | string | ≤ 32 000 characters, Unicode |
| `path` | string | Full path, ≤ 32 000 characters |
| `kind` | enum (`file`, `directory`, `link`, `other`) | Required |
| `logical_size` | u64 | ≥ 0 |
| `on_disk_size` | u64 | ≥ 0 |
| `physical_size` | u64? | For hard links/ADS, if known |
| `allocated_size` | u64? | If known |
| `child_count` | u32 | For directories |
| `file_count` | u64? | For aggregates |
| `modified_at` | timestamp? | May be absent |
| `accessed_at` | timestamp? | May be absent; mark as unreliable |
| `attributes` | set of flags | read-only, hidden, system, compressed, sparse, reparse |
| `accessible` | bool | false — mark as "no access" |
| `link_target` | string? | For links |
| `ads_size` | u64? | Total ADS size |

Rule: the UI does not recompute `logical_size`/`on_disk_size` and does not sum rows for totals;
totals come from Rust.

### NodeProgress

Inline tree node progress bar.

| Field | Type | Rules |
|-------|------|-------|
| `node_id` | `NodeId` | Required |
| `share_of_parent` | f32 | 0..1, the node's size as a share of its parent |
| `share_of_volume` | f32 | 0..1, the node's size as a share of the whole volume |

Rules: the shares are computed by Rust and included in `NodeRow`; the UI uses them only for the
semi-transparent tree row fill and does not recompute totals.

### FilterSet

| Field | Type | Rules |
|-------|------|-------|
| `text` | string | ≤ 256 characters |
| `extensions` | array of strings | Normalized, without the dot |
| `categories` | array of enum | File types |
| `min_size` / `max_size` | u64? | `min ≤ max` |
| `modified_after` / `modified_before` | date? | `after ≤ before` |
| `accessible_only` | bool | false by default |
| `exclude_paths` | array of strings | Verbatim-compatible |
| `name` | string? | Name of the saved set, ≤ 64 |

### SelectionModel

| Field | Type | Rules |
|-------|------|-------|
| `mode` | enum (`none`, `single`, `multiple`, `range`) | Required |
| `ids` | `Set<NodeId>` | Only visible/loaded objects |
| `anchor_id` | `NodeId?` | For range |
| `select_all_in_filter` | bool | An explicit mode, not to be confused with `ids` |
| `excluded_ids` | `Set<NodeId>` | For "all except" |

Rule: a bulk action always shows the actual count and size; the UI does not silently substitute
"all".

### ColumnState

| Field | Type | Rules |
|-------|------|-------|
| `id` | string | Unique per screen |
| `visible` | bool | Required |
| `width_px` | u16 | 40..1200 |
| `order` | u16 | Unique |
| `sortable` | bool | From the screen schema |

### OperationPreview

| Field | Type | Rules |
|-------|------|-------|
| `preview_id` | UUID | One-time, TTL 10 minutes |
| `kind` | enum (`delete`, `move`, `rename`, `permissions`, `shred`, `restore`) | Required |
| `targets` | `OperationTarget[]` | Not empty |
| `total_size` | u64 | ≥ 0, sum of the targets |
| `delete_mode` | enum (`recycle_bin`, `quarantine`, `permanent`) | Required for delete/shred |
| `reversible` | bool | Determined by Rust |
| `requires_elevation` | bool | System/protected paths |
| `warnings` | `MessageKey[]` | SSD, system path, large volume |
| `expires_at` | timestamp | TTL |
| `confirmation_token` | string | One-time, bound to the preview |

Rules: an operation cannot be executed without `preview_id` + `confirmation_token`; on expiry —
a new preview; `targets` cannot go beyond the selected scope.

### OperationProgress

| Field | Type | Rules |
|-------|------|-------|
| `operation_id` | UUID | Required |
| `state` | enum (`preparing`, `running`, `cancelling`, `completed`, `partial`, `failed`, `cancelled`) | Required |
| `processed` / `total` | u64 | `processed ≤ total`, if total is known |
| `current_target` | string? | Full path |
| `bytes_per_second` | u64? | ≥ 0 |
| `errors` | `UiError[]` | Up to 100 in the UI, the full list in the journal |
| `undo_available` | bool | Only for reversible operations |
| `journal_entry_id` | UUID? | Reference to the journal |

### OperationTarget

| Field | Type | Rules |
|-------|------|-------|
| `path` | string | Full path, ≤ 32 000 |
| `kind` | enum | file/directory/link |
| `size` | u64 | ≥ 0 |
| `accessible` | bool | false — mark and do not execute silently |
| `is_system` | bool | Requires elevation/confirmation |

### JournalEntryView

| Field | Type | Rules |
|-------|------|-------|
| `id` | UUID | Required |
| `occurred_at` | timestamp | Required |
| `user` | string | Local user |
| `kind` | enum | Matches the operation |
| `targets_count` | u32 | ≥ 0 |
| `total_size` | u64 | ≥ 0 |
| `result` | enum (`success`, `partial`, `failed`, `cancelled`) | Required |
| `reversible` | bool | For undo/restore |
| `quarantine_ids` | UUID[] | For restoration |
| `error` | `UiError?` | On failure |

### UiSettings

| Field | Type | Default | Rules |
|-------|------|---------|-------|
| `theme` | enum (`system`, `dark`, `light`) | `system` | System on first launch |
| `language` | enum (`ru`, `en`) | `ru` | Or the OS language, if supported |
| `density` | enum (`comfortable`, `compact`) | `comfortable` | Affects row height |
| `units` | enum (`binary`, `decimal`) | `binary` | KiB/MiB vs KB/MB |
| `reduced_motion` | enum (`system`, `on`, `off`) | `system` | The system setting is respected |
| `last_screen` | `Screen.id?` | null | Restored if available |
| `sidebar_collapsed` | bool | false | — |
| `column_layouts` | map<Screen.id, ColumnState[]> | {} | Schema version |
| `saved_filters` | map<Screen.id, FilterSet[]> | {} | ≤ 20 per screen |
| `schema_version` | u16 | 1 | Migration |

### WindowState

| Field | Type | Rules |
|-------|------|-------|
| `width` / `height` | u32 | ≥ 800×600, ≤ 16 384 |
| `x` / `y` | i32 | Within the virtual screen; otherwise centered |
| `maximized` | bool | — |
| `monitor_id` | string? | For multi-monitor |

### UiError

| Field | Type | Rules |
|-------|------|-------|
| `code` | string | `E_*`, stable |
| `category` | enum (`validation`, `access`, `io`, `operation`, `internal`, `cancel`, `stale`) | Required |
| `message_key` | `MessageKey` | Localizable |
| `details` | string? | No secrets; ≤ 4 KiB |
| `retryable` | bool | Affects the "Retry" button |
| `task_id` / `journal_entry_id` | UUID? | Link to the task/journal |

### Toast

| Field | Type | Rules |
|-------|------|-------|
| `id` | UUID | — |
| `severity` | enum (`info`, `success`, `warning`, `error`) | — |
| `message_key` | `MessageKey` | — |
| `action` | enum (`none`, `open_journal`, `undo`, `retry`) | — |
| `persistent` | bool | true for errors that require a decision |

### Message / MessageKey

| Field | Type | Rules |
|-------|------|-------|
| `key` | string | `dot.case`, unique |
| `ru` | string | Not empty |
| `en` | string | Not empty, no Cyrillic |
| `placeholders` | array of strings | Match in RU/EN |
| `context` | string? | Translator comment |

Rule: keys in RU and EN match; the generator creates the `MessageKey` union; missing-key is a
build/test error.

### DesignToken / Theme

| `DesignToken` field | Type | Rules |
|---------------------|------|-------|
| `name` | string | `--fs-*` |
| `category` | enum (`color`, `space`, `radius`, `type`, `motion`, `shadow`, `border`) | — |
| `dark` / `light` | string | A valid CSS value |
| `usage` | string | Purpose |

`Theme` = `resolved` (`dark`/`light`) + `preference` (`system`/`dark`/`light`) + a set of tokens
and the chart and tree progress-bar palette. Rule: product components use only `var(--fs-*)`.

### ScreenshotBaseline

| Field | Type | Rules |
|-------|------|-------|
| `id` | UUID | — |
| `screen_id` | `Screen.id` | — |
| `theme` | enum (`dark`, `light`) | — |
| `dpi` | enum (`100`, `125`, `150`, `200`) | — |
| `language` | enum (`ru`, `en`) | — |
| `viewport` | `{w,h}` | Fixed |
| `image_path` | string | In the repository/LFS |
| `tolerance` | f32 | 0..1, default 0.01 |
| `updated_at` | date | — |

### ParityMatrix

| Field | Type | Rules |
|-------|------|-------|
| `screen_id` | `Screen.id` | — |
| `capability` | string | User-facing capability |
| `cli_command` | string | Existing CLI command |
| `tests` | string[] | Unit/E2E/visual |
| `screenshot_id` | UUID? | For the screen |
| `status` | enum (`todo`, `ported`, `verified`, `blocked`) | verified only after acceptance |

## 2. Relations

```text
Screen 1 ─── 1 ScreenState
Screen 1 ─── * ColumnState
Screen 1 ─── * ScreenshotBaseline
Screen 1 ─── 1 ParityMatrix

ScanTask 1 ─── 1 TaskProgress
ScanTask 1 ─── * TreePage (generation)

TreePage 1 ─── * NodeRow
NodeRow 1 ─── 0..1 NodeProgress

OperationPreview 1 ─── * OperationTarget
OperationPreview 1 ─── 1 OperationProgress (after confirmation)
OperationProgress 1 ─── 0..1 JournalEntryView
JournalEntryView 1 ─── * QuarantineItemView

UiSettings 1 ─── * ColumnState
UiSettings 1 ─── * FilterSet
WindowState 1 ─── 1 UiSettings (separate file alongside)
```

## 3. Integrity rules

1. **Single source of truth.** Sizes, totals, operation results, and the journal come from Rust;
   TypeScript does not recompute them and does not "fill in" missing values.
2. **Generations.** Every page is bound to `generation`; after a new scan, old pages
   are invalidated, and `stale_cursor` forces the UI to start from the first page.
3. **Cursors.** A cursor is opaque to the UI, limited to 512 characters, and is not mixed with
   the cursor of another sort/filter/generation.
4. **Pagination.** `page_size` 1..2000 (default 500); the full set of
   rows/nodes is never transmitted in a single response.
5. **Operations.** No execution without `preview_id` + `confirmation_token`; the preview expires;
   targets do not go beyond the selected scope; system paths require elevation.
6. **Selection.** `select_all_in_filter` is a separate mode; a bulk action always shows the
   actual count and total size.
7. **Localization.** The RU/EN key sets match; placeholders match; missing-key is a test/build
   error.
8. **Themes.** Components use only tokens; the `resolved` theme is computed from the preference
   and the system setting; a change is applied without a restart.
9. **Storage.** `UiSettings` and `WindowState` are written only by Rust into `data`; the UI does
   not write to `localStorage`, `IndexedDB`, or the WebView2 profile.
10. **Window.** Invalid coordinates/sizes are corrected to the visible area; state is saved
    after a successful change, not on every frame.
11. **Errors.** Every error has `code`, `category`, `message_key`, and `retryable`; the UI does
    not show raw Rust text as the only explanation.
12. **Screenshots.** A baseline exists for every screen × theme × DPI × language; without a
    baseline the visual test is not considered passed.

## 4. States and transitions

### Scanning

```text
queued → running → completed
                 ├→ partial   (access errors/cancellation with partial data)
                 ├→ failed
                 └→ cancelling → cancelled
```

### Operation

```text
draft → previewing → awaiting_confirmation → running → completed
                                             ├→ partial
                                             ├→ failed
                                             └→ cancelling → cancelled
completed (reversible) → undo_available → undone
completed (quarantine) → restore_available → restored
```

### Theme

```text
preference = system | dark | light
resolved = preference == system ? OS theme : preference
OS theme changes → resolved recomputed → tokens swapped (no restart)
```

### Screen

```text
idle → loading → ready | empty | partial | error
ready → refreshing → ready | partial | error
ready/partial/error → loading (new volume/filter/generation)
```

### Language

```text
ru ↔ en
state preserved; messages re-rendered; generated catalogs keyed identically
```

## 5. Constraints and validation at the IPC boundary

- Paths: full path ≤ 32 000 characters, canonicalization, prohibition of escaping the selected
  scope.
- Identifiers: `NodeId` within the current generation; foreign ids → `E_STALE_NODE`.
- Enumerations: an unknown value → `E_VALIDATION`.
- Sizes: u64, non-negative; overflow → `E_VALIDATION`.
- Strings: names ≤ 32 000, filters ≤ 256, cursors ≤ 512, message keys ≤ 128.
- Tree: `page_size` ≤ 2000, `max_depth` ≤ 12; node shares (`share_of_parent`/`share_of_volume`) in
  the range 0..1.
- Preview: TTL, one-time token, non-empty target list, `total_size` = sum of targets.
- Settings: `schema_version` is supported; unknown fields are preserved but do not break
  reading; migration is idempotent.
- Window: minimum size 800×600; coordinates outside the visible area are corrected.
