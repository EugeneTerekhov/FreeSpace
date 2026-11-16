# UI contract: tokens, components, screens and accessibility

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

The contract fixes the product language of the interface. It is mandatory for 24 screens and is
verified by tests: hard-coded colors, strings outside the localization catalog, a missing state or
screenshot are considered defects.

## 1. Design tokens

Tokens are CSS custom properties in `ui/src/styles/tokens.css`. Components use only `var(--fs-*)`;
the values below are starting values and are refined by visual tests without changing names.

### 1.1. Colors

| Token | Dark theme | Light theme | Purpose |
|-------|-------------|--------------|------------|
| `--fs-bg` | `#07060C` | `#F6F2EC` | Window background |
| `--fs-surface` | `#110E1B` | `#FFFFFF` | Main surfaces |
| `--fs-surface-raised` | `#171327` | `#FFFDFA` | Cards, panels |
| `--fs-surface-overlay` | `#1F1930` | `#F1EAE2` | Menus, popovers, hover rows |
| `--fs-line` | `rgba(255,255,255,.075)` | `rgba(17,14,27,.12)` | Separators and borders |
| `--fs-line-strong` | `rgba(255,255,255,.14)` | `rgba(17,14,27,.22)` | Active borders |
| `--fs-text` | `#F4EFE9` | `#1B1626` | Primary text |
| `--fs-text-muted` | `#A9A1B6` | `#5A5364` | Secondary text |
| `--fs-text-subtle` | `#736C82` | `#7A7285` | Captions, hints |
| `--fs-accent` | `#FF8A3D` | `#E06A1C` | Fills, active elements |
| `--fs-accent-strong` | `#FFB56B` | `#A8430A` | Accent text, icons, focus |
| `--fs-accent-ink` | `#1A1208` | `#1A1208` | Text on accent fill |
| `--fs-violet` | `#A06BFF` | `#6D3FD1` | Second accent, charts |
| `--fs-success` | `#7EE8A9` | `#1F7A4D` | Success |
| `--fs-danger` | `#F87171` | `#B42318` | Error, dangerous action |
| `--fs-warning` | `#FBBF24` | `#8A5A00` | Warning |
| `--fs-info` | `#7DD3FC` | `#0B6A8A` | Information |
| `--fs-focus` | `#FFB56B` | `#A8430A` | Focus ring |
| `--fs-selection` | `rgba(255,138,61,.16)` | `rgba(224,106,28,.14)` | Selected row/tile |

Rules:

- Any "text — background" pair is verified by an automatic contrast test (WCAG 2.2 AA: 4.5:1 for
  normal text, 3:1 for large text and non-text elements).
- Status is never conveyed by color alone: text, an icon or a shape is added.
- Transparent layers are not used for primary text.
- The Treemap palette uses separate tokens `--fs-tm-1…--fs-tm-10` for each theme; adjacent
  categories are distinguishable, labels are readable.

### 1.2. Typography

| Token | Value |
|-------|----------|
| `--fs-font-sans` | `"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif` |
| `--fs-font-display` | `"Segoe UI Variable Display", "Segoe UI", sans-serif` |
| `--fs-font-mono` | `"Cascadia Mono", "Consolas", monospace` |
| `--fs-text-xs` | `11px/16px` |
| `--fs-text-sm` | `12px/18px` |
| `--fs-text-base` | `13px/20px` |
| `--fs-text-md` | `14px/22px` |
| `--fs-text-lg` | `16px/24px` |
| `--fs-text-xl` | `20px/28px` |
| `--fs-text-2xl` | `24px/32px` |
| `--fs-text-3xl` | `32px/40px` |
| `--fs-text-4xl` | `40px/48px` |
| `--fs-weight-regular` | `400` |
| `--fs-weight-medium` | `500` |
| `--fs-weight-semibold` | `600` |
| `--fs-weight-bold` | `700` |

Rules: primary text — `base`/`md`; screen headings — `xl`/`2xl`; tables — `sm`/`base`;
monospaced — only paths, hashes, sizes in bytes and codes. No web fonts or CDN.

### 1.3. Spacing, radii, shadows, layers

| Group | Tokens |
|--------|--------|
| Spacing | `--fs-space-1: 4px`, `2: 8px`, `3: 12px`, `4: 16px`, `5: 20px`, `6: 24px`, `8: 32px`, `10: 40px` |
| Radii | `--fs-radius-sm: 6px`, `md: 8px`, `lg: 10px`, `xl: 14px`, `2xl: 20px`, `pill: 999px` |
| Borders | `--fs-border: 1px solid var(--fs-line)`; strong — `--fs-line-strong` |
| Shadows | `--fs-shadow-1: 0 1px 2px rgba(0,0,0,.24)`, `--fs-shadow-2: 0 8px 24px rgba(0,0,0,.32)`, light theme — softer |
| Layers | `--fs-z-sticky: 10`, `dropdown: 100`, `overlay: 200`, `dialog: 300`, `toast: 400`, `tooltip: 500` |
| Density | `--fs-row-comfortable: 36px`, `--fs-row-compact: 30px`, `--fs-header: 56px`, `--fs-sidebar: 240px`, `--fs-sidebar-collapsed: 64px`, `--fs-status: 28px` |

### 1.4. Motion

| Token | Value |
|-------|----------|
| `--fs-motion-hover` | `120ms` |
| `--fs-motion-panel` | `180ms` |
| `--fs-motion-dialog` | `240ms` |
| `--fs-ease` | `cubic-bezier(.2,.8,.2,1)` |
| `--fs-motion-reduced` | `0ms` |

Rules: animation does not hide a critical warning and does not block input. Under
`prefers-reduced-motion: reduce` or the `reduced_motion=on` setting, optional transitions are
disabled; functional state changes remain instantaneous and clear.

## 2. Theme contract

- Preference: `system | dark | light`; default — `system`.
- Resolved theme: `preference === system ? OS theme : preference`.
- Attributes on `<html>`: `data-theme="dark|light"`, `data-density="comfortable|compact"`,
  `data-reduced-motion="on|off"`, `lang="ru|en"`, `style="color-scheme: dark|light"`.
- The theme is applied before the first paint: `boot.js` in `<head>` reads
  `window.__FREESPACE_BOOT__` (Rust injection) or `prefers-color-scheme`.
- A system theme change is applied without a restart via a `matchMedia` listener; the stored
  preference is not overwritten.
- When the theme changes, data is not reloaded, and filters, selection and scrolling are not reset.
- The Treemap and charts are redrawn with the new palette in the same frame, without flicker.

## 3. Product kit components

| Component | Required states | Accessibility |
|-----------|------------------------|-------------|
| `AppShell` | ready, loading, error | Landmark roles `banner`, `navigation`, `main`, `contentinfo` |
| `Sidebar` | expanded, collapsed, active, badge | `<nav>`, `aria-current="page"`, keyboard |
| `Header` | default, with-back, with-actions | `<header>`, `h1` heading |
| `StatusBar` | idle, task, error, offline | `role="status"`, `aria-live="polite"` |
| `Button` | default, hover, focus, active, disabled, loading, danger, ghost | `<button>`, `aria-busy`, not color alone |
| `IconButton` | default, focus, active, disabled | `aria-label`, tooltip |
| `Field` | default, focus, error, disabled, readonly | `<label>`, `aria-describedby`, `aria-invalid` |
| `SearchField` | default, focus, with-clear, loading | `role="searchbox"`, Esc clears |
| `Select` | default, open, disabled, error | Native `select` or combobox with ARIA |
| `Combobox` | collapsed, open, filtered, empty | `aria-expanded`, `aria-controls`, active descendant |
| `Segmented` | default, selected, disabled | `role="tablist"`/radiogroup, arrow keys |
| `Tabs` | default, selected, disabled | `role="tablist"`, `aria-selected`, arrow keys |
| `Toggle` | on, off, disabled | `role="switch"`, `aria-checked` |
| `Checkbox` | unchecked, checked, indeterminate, disabled | Native input |
| `Radio` | unchecked, checked, disabled | Native input + fieldset/legend |
| `Chip` | default, selected, removable | Button/`aria-pressed` |
| `Badge` | neutral, success, warning, danger, info | Text, not color alone |
| `Tooltip` | hidden, visible | `aria-describedby`, not the sole carrier of meaning |
| `Progress` | determinate, indeterminate, error, cancelled | `role="progressbar"`, `aria-valuenow` |
| `Meter` | normal, warning, danger | `role="meter"`, text label |
| `DataTable` | loading, ready, empty, partial, error, selected | `role="grid"`/table, `aria-rowcount`, sorting |
| `TreeTable` | collapsed, expanded, selected, loading, error | `role="treegrid"`, arrow keys, `aria-expanded` |
| `List` | loading, ready, empty, error | `<ul>`/`role="listbox"` by context |
| `Card` | default, interactive, selected | Heading and description |
| `StatCard` | default, loading, warning | Number + label + trend |
| `SummaryCard` | default, selected, loading | Bento grid only on the dashboard |
| `Dialog` | closed, open, busy, error | `role="dialog"`, `aria-modal`, focus trap, Esc |
| `ConfirmDialog` | default, destructive, busy, error | Heading, consequences, explicit button |
| `Toast` | info, success, warning, error, with-action | `aria-live`, does not overlap the dialog |
| `Banner` | info, warning, error, offline | `role="alert"` for errors |
| `EmptyState` | no-data, no-volume, no-scan, no-results | Heading, explanation, action |
| `ErrorState` | recoverable, fatal, with-details | Code, message, "Retry"/"Journal" |
| `Loading` / `Skeleton` | page, section, row | `aria-busy`, not an endless indicator |
| `Treemap` | loading, ready, empty, error, zoomed | `role="img"` + text summary, keyboard |
| `Legend` | categorical, size | Link between colors and labels |
| `Chart` | loading, ready, empty, error | Text alternative/table |
| `Breadcrumbs` | default, overflow | `nav`, `aria-current` |
| `Pagination` | cursor, end, loading | "Load more"/virtual scroll |
| `SplitPane` | default, collapsed | Keyboard resizing |
| `ContextMenu` | closed, open, disabled-item | `role="menu"`, arrow keys, Esc |
| `CommandPalette` | closed, open, filtered, empty | `role="dialog"`, `aria-activedescendant`, Ctrl+K |
| `Kbd` | default | Visual hint, not the only way |

Rules: a component without an error/empty state is not considered ready; interactive components
have `:focus-visible`; the minimum hit target is 24×24 px (32×32 recommended).

## 4. Screens

All 24 screens use the shared `Screen` frame: heading, context, main content, details panel (if
needed), loading/empty/error/partial-result states.

| ID | Batch | Purpose | Main data | Actions | CLI equivalent |
|----|--------|------------|-----------------|----------|------------|
| `dashboard` | core | Overview of volumes and dynamics | `volumes_list`, snapshots | Analyze, open volume, compare | `drives` |
| `scan` | core | Starting and progress of the analysis | `scan_start/status`, `scan-progress` | Start, cancel, retry | `scan` |
| `tree` | core | Size hierarchy | `tree_root/children/search` | Expand, filter, sort, select | `tree`, `filter` |
| `treemap` | core | Visual map | `treemap_tiles` | Drill in, back, coloring, select | `tree` |
| `top_files` | analysis | Largest files | `top_files_page` | N, filter, go to tree | `top` |
| `types` | analysis | Distribution by type | `types_summary` | Select type, filter | `types` |
| `duplicates` | analysis | Duplicate groups | `duplicates_*` | Search, rule, select copies | `duplicates` |
| `cleanup` | analysis | Cleanup recommendations | `cleanup_candidates` | Select category, preview | `cleanup suggest`, `cleanup empty-dirs` |
| `snapshots` | analysis | Snapshots and comparison | `snapshots_*` | Create, compare, delete | `snapshot create/diff` |
| `journal` | filesystem | Operation history | `journal_page` | Open, undo, restore | `delete`/`shred`/`rename`/`acl` (records) |
| `size_details` | filesystem | Exact size metrics | `size_details` | Change size type, copy | `tree`, `filter` |
| `long_paths` | filesystem | Objects > 260 characters | `long_paths_page` | Open, rename, delete | `rename`, `delete`, `tree` |
| `rename` | filesystem | Bulk rename | `rename_preview/apply` | Template, preview, apply | `rename` |
| `permissions` | filesystem | NTFS permissions and attributes | `permissions_get/preview/apply` | Preview, apply | `acl`, `attrs` |
| `metadata` | inspection | File metadata | `metadata_get` | Select file, filter | `filter --metadata` |
| `owners` | inspection | Distribution by owner | `owners_page` | Select owner, go to files | `owners` |
| `archives` | inspection | Archive contents | `archives_entries_page` | Open, filter, export list | `archives` |
| `export` | inspection | Report export | `export_preview/start/status` | Format, composition, start | `export` |
| `unlock` | inspection | File locks | `unlock_locks` | Find, release, reboot | `unlock` |
| `shred` | settings | Irreversible destruction | `shred_preview/start/status` | Method, passes, confirmation | `shred` |
| `monitor` | settings | Write monitoring | `monitor_*`, `monitor-tick` | Start/stop, threshold, filter | `monitor` |
| `elevation` | settings | Privilege elevation | `elevation_status/request` | Request, explain the reason | System action; CLI elevation flags |
| `settings` | settings | Application settings | `settings_get/set` | Theme, language, units, density | `config get/set` |
| `about` | settings | Version and licenses | `about_info` | Copy version, open licenses | `--version` |

### 4.1. Mandatory screen states

Every screen must have:

1. `loading` — skeleton/progress, not an empty screen.
2. `empty` — explanation and action (select a volume, start an analysis, change the filter).
3. `error` — code, understandable message, "Retry", link to the journal.
4. `partial` — a visible note about incomplete data and the reasons (no access, cancellation).
5. `busy` — a long-running operation with progress and cancellation, if cancellation is available.
6. `ready` — data and actions.

### 4.2. Navigation and state preservation rules

- Switching screens preserves filters, sorting, selection and `scroll_anchor`.
- Changing the volume resets the data of dependent screens and shows a confirmation if there are
  unsaved changes.
- Returning to a screen does not reload data if `generation` has not changed.
- Deep links open the screen and the object; if the object has disappeared — an empty state with the
  reason.

## 5. Accessibility contract

### 5.1. Keyboard

| Context | Keys |
|----------|---------|
| Shell | `Tab`/`Shift+Tab`, `Ctrl+K` — command palette, `Ctrl+,` — settings, `F1` — help |
| Navigation | `Ctrl+1…9` — first screens of the batch, `Alt+←` — back, `Alt+→` — forward |
| Search | `Ctrl+F` — focus search, `Esc` — clear/close |
| Tree | `↑/↓`, `←/→` — collapse/expand, `Home/End`, `Enter` — open, `Space` — select |
| Table | `↑/↓`, `PageUp/PageDown`, `Home/End`, `Ctrl+A`, `Shift+↑/↓` — range, `Space` — select |
| Dialog | `Tab`/`Shift+Tab` inside, `Esc` — close, `Enter` — only on the focused button |
| Map | `Tab` — focus, `Enter` — drill in, `Backspace`/`Alt+←` — back, arrow keys — neighboring tiles |
| Notifications | `F6` — go to notifications, `Esc` — close the optional one |

### 5.2. Focus and announcements

- Focus is visible on all backgrounds (2 px, offset 2 px).
- A dialog traps focus and returns it to its initiator.
- `aria-live="polite"` — progress and statuses; `aria-live="assertive"` — errors that require a
  decision.
- Progress is announced no more than once every 2 seconds so as not to interrupt speech.
- Decorative icons are `aria-hidden="true"`; icon buttons have an `aria-label`.
- Tables have headers, `aria-rowcount`/`aria-colcount`; the tree has `aria-level`,
  `aria-expanded`, `aria-selected`.
- Color is not the sole carrier of status; errors have text and an icon.

### 5.3. Contrast, DPI and motion

- WCAG 2.2 AA: 4.5:1 normal text, 3:1 large text and borders of interactive elements.
- 100/125/150/200% DPI: all 24 screens are checked; hit targets are at least 24×24 px.
- `prefers-reduced-motion` and `prefers-contrast` are respected; the reduced motion setting
  overrides the system one only by the user's explicit choice.

## 6. Localization contract

- Keys: `dot.case`, Latin characters, for example `nav.dashboard`, `scan.progress.current_path`.
- Flat keys are generated from `locales/*.toml`; the `MessageKey` union is mandatory.
- Placeholders: `{name}`, `{count}`, `{size}`; the set and types match in RU/EN.
- Pluralization: `{count} files` via the language rules, without concatenating strings.
- Numbers, dates and units — via `Intl.*`; units — `binary` (KiB/MiB) or `decimal`
  (KB/MB) per setting.
- Prohibited: concatenation of translated fragments, strings in JSX, error messages
  in English only.
- Missing key: English fallback + test error; an empty string is prohibited.
- Changing the language does not reload the application and does not lose state.

## 7. Visual regression contract

- Full matrix: 24 screens × 2 themes × 4 DPI × 2 languages = 384 baselines.
- PR set: 8 critical screens × 2 themes × 100/150% × 2 languages + all 24 screens in the dark
  theme, 100%, RU.
- Viewport: 1440×900 (primary), 1280×800 (narrow), 1920×1080 (wide); for DPI — `deviceScaleFactor`
  emulation.
- Fonts are fixed by the system stack; font differences on CI are allowed only in a controlled
  Windows image.
- Default tolerance 0.01 (1% of pixels); for Treemap/charts — 0.02.
- A baseline is updated only deliberately, with diff review; a missing baseline is a test failure.
- For each failure, the screenshot, diff and trace are saved.

## 8. Parity matrix (template)

| Screen | Capability | CLI command | Test | Screenshot | Status |
|-------|-------------|-------------|------|----------|--------|
| `dashboard` | List of volumes and sizes | `drives` | Rust + E2E | `dashboard-dark-100-ru` | todo |
| `scan` | Start/cancel/progress | `scan` | Rust + E2E | `scan-dark-100-ru` | todo |
| `tree` | Hierarchy, search, filter | `tree`, `filter` | Vitest + E2E | `tree-dark-100-ru` | todo |
| `treemap` | Map and drill-in | `tree` | Vitest + visual | `treemap-dark-100-ru` | todo |
| … | … | … | … | … | todo |

The matrix is filled up to 24 rows; `verified` is set only after the screen, the test and the
screenshot have been accepted and the CLI behavior has been checked.
