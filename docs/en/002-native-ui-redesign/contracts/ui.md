# UI contract: tokens, components, screens and accessibility

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

The contract fixes the product language of the interface. It is mandatory for 11 screens and is
verified by tests: hard-coded colors, strings outside the localization catalog, a missing state or
screenshot are considered defects.

## 1. Design tokens

Tokens are CSS custom properties in `ui/src/styles/tokens.css`. Components use only `var(--fs-*)`;
the values below are starting values and are refined by visual tests without changing names.

### 1.1. Colors

| Token | Dark theme | Light theme | Purpose |
|-------|-------------|--------------|------------|
| `--fs-bg` | `#344b52` | `#e8e1db` | Window background |
| `--fs-surface` | `#3a535a` | `#f2ede9` | Main surfaces |
| `--fs-surface-raised` | `#405d65` | `#f7f3f0` | Cards, panels |
| `--fs-surface-overlay` | `#4a6b74` | `#dcd5cc` | Menus, popovers, hover rows |
| `--fs-line` | `rgb(250 232 218 / 12%)` | `rgb(54 46 35 / 12%)` | Separators and borders |
| `--fs-line-strong` | `rgb(250 232 218 / 20%)` | `rgb(54 46 35 / 22%)` | Active borders |
| `--fs-text` | `#fae8da` | `#362e23` | Primary text |
| `--fs-text-muted` | `#cfd3ca` | `#5d5935` | Secondary text |
| `--fs-text-subtle` | `#c6d3d2` | `#636138` | Captions, hints |
| `--fs-accent` | `#fae8da` | `#5d5935` | Fills, active elements |
| `--fs-accent-strong` | `#fae8da` | `#5d5935` | Accent text, icons, focus |
| `--fs-accent-ink` | `#344b52` | `#e8e1db` | Text on accent fill |
| `--fs-success` | `#b8dcc0` | `#2f6b3f` | Success |
| `--fs-danger` | `#f5c4b4` | `#a8442f` | Error, dangerous action |
| `--fs-warning` | `#edcf92` | `#7a5c12` | Warning |
| `--fs-info` | `#b9d6da` | `#2f5f6b` | Information |
| `--fs-focus` | `#cfd3ca` | `#6d6c3c` | Focus ring |
| `--fs-selection` | `rgb(250 232 218 / 16%)` | `rgb(169 172 93 / 22%)` | Selected row |

The palette has five anchors per theme (plan.md §2.1.1): dark `#344b52 → #fae8da`, light
`#362E23 → #E8E1DB`. Derived shades use `color-mix(in oklch, …)`; neither theme uses pure
`#000000`/`#FFFFFF` on large surfaces.

Rules:

- Any "text — background" pair is verified by an automatic contrast test (WCAG 2.2 AA: 4.5:1 for
  normal text, 3:1 for large text and non-text elements).
- Status is never conveyed by color alone: text, an icon or a shape is added.
- Transparent layers are not used for primary text.
- The tree progress bar is a semi-transparent row fill (`--fs-tree-fill`, `--fs-tree-fill-strong`)
  whose share is proportional to the node's size relative to its parent; the selected row is marked
  by the accent and a ring.
- Decorative gradients, "aurora" backdrops and glows are removed: color marks state or action only
  (principle 3).

### 1.2. Typography

| Token | Value |
|-------|----------|
| `--fs-font-mono` | `"JetBrains Mono", "Cascadia Mono", "Consolas", ui-monospace, monospace` |
| `--fs-font-sans` / `--fs-font-display` | same as `--fs-font-mono` (one family) |
| `--fs-text-2xs` | `11px/15px` |
| `--fs-text-xs` | `13px/17px` |
| `--fs-text-sm` | `15px/20px` |
| `--fs-text-base` | `16px/22px` |
| `--fs-text-md` | `19px/26px` |
| `--fs-text-lg` | `23px/30px` |
| `--fs-text-xl` | `27px/34px` |
| `--fs-text-2xl` | `33px/40px` |
| `--fs-text-3xl` | `40px/46px` |
| `--fs-text-4xl` | `48px/54px` |
| `--fs-weight-regular` | `400` |
| `--fs-weight-medium` | `500` |
| `--fs-weight-semibold` | `600` |
| `--fs-weight-bold` | `700` |

Rules: one monospaced family with tabular figures for paths, sizes and numbers; exactly one `h1`
per screen (`2xl`) — the shell heading; section overlines are `2xs`, uppercase, letter-spacing
`0.08em`; the font ships locally or falls back to the system monospace stack, never a CDN.

### 1.3. Spacing, radii, shadows, layers

| Group | Tokens |
|--------|--------|
| Spacing | `--fs-space-1: 4px`, `2: 8px`, `3: 12px`, `4: 16px`, `5: 20px`, `6: 24px`, `8: 32px`, `10: 40px` |
| Radii | `--fs-radius-sm: 4px`, `md: 6px`, `lg: 8px`, `pill: 999px` |
| Borders | `--fs-border: 1px solid var(--fs-line)`; strong — `--fs-line-strong` |
| Shadows | functional only: `--fs-shadow-overlay` for dialogs and overlays; decorative shadows removed |
| Layers | `--fs-z-sticky: 10`, `dropdown: 100`, `overlay: 200`, `dialog: 300`, `toast: 400`, `tooltip: 500` |
| Density | `--fs-row-comfortable: 36px`, `--fs-row-compact: 30px`, `--fs-header: 56px`, `--fs-sidebar: 240px`, `--fs-sidebar-collapsed: 64px`, `--fs-status: 28px` |

### 1.4. Motion

| Token | Value |
|-------|----------|
| `--fs-motion-fast` | `120ms` |
| `--fs-motion-base` | `180ms` |
| `--fs-motion-hover` | `120ms` |
| `--fs-motion-panel` | `180ms` |
| `--fs-motion-dialog` | `180ms` |
| `--fs-ease` | `cubic-bezier(.2,.8,.2,1)` |
| `--fs-motion-reduced` | `0ms` |

Rules: transitions only for functional state changes (open/close, hover, focus); no staggered
reveals, count-up or button lift. Under `prefers-reduced-motion: reduce` or the `reduced_motion=on`
setting, optional transitions are disabled; functional state changes remain instantaneous and clear.

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
- The tree and charts are redrawn with the new palette in the same frame, without flicker.

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
| `DeleteObjectDialog` | open, busy, error | `role="dialog"`, compact question, name, type, size and modification date |
| `Toast` | info, success, warning, error, with-action | `aria-live`, does not overlap the dialog |
| `Banner` | info, warning, error, offline | `role="alert"` for errors |
| `EmptyState` | no-data, no-volume, no-scan, no-results | Heading, explanation, action |
| `ErrorState` | recoverable, fatal, with-details | Code, message, "Retry"/"Journal" |
| `Loading` / `Skeleton` | page, section, row | `aria-busy`, not an endless indicator |
| `TreeRow` | collapsed, expanded, selected, loading, error | `role="treegrid"` row, type icon, `aria-expanded`, inline progress bar |
| `Legend` | categorical, size | Link between colors and labels |
| `Chart` | loading, ready, empty, error | Text alternative/table |
| `Breadcrumbs` | default, overflow | `nav`, `aria-current` |
| `Pagination` | cursor, end, loading | "Load more"/virtual scroll |
| `SplitPane` | default, collapsed | Keyboard resizing |
| `ContextMenu` | closed, open, disabled-item | `role="menu"`, arrow keys, Esc; tree row items: open in Explorer, copy path, rename, delete to Recycle Bin |
| `CommandPalette` | closed, open, filtered, empty | `role="dialog"`, `aria-activedescendant`, Ctrl+K |
| `Kbd` | default | Visual hint, not the only way |

Rules: a component without an error/empty state is not considered ready; interactive components
have `:focus-visible`; the minimum hit target is 24×24 px (32×32 recommended).

## 4. Screens

All 11 screens use the shared `Screen` frame: heading, context, main content, details panel (if
needed), loading/empty/error/partial-result states. Screens are grouped into the 6 sidebar groups;
absorbed screens remain tabs/sections and parity matrix rows.

| ID | Group | Purpose | Main data | Actions | CLI equivalent |
|----|-------|---------|-----------|---------|----------------|
| `overview` | overview | Volume overview and inline scanning | `volumes_list`, `scan_start/status`, `scan-progress`, snapshots | Analyze, cancel, compare | `drives`, `scan` |
| `tree` | analysis | Size hierarchy + size details | `tree_root/children/search`, `size_details` | Expand, filter, sort, select | `tree`, `filter` |
| `analysis` | analysis | Largest files, types, owners | `top_files_page`, `types_summary`, `owners_page` | N, filter, tabs | `top`, `types`, `owners` |
| `duplicates` | analysis | Duplicate groups | `duplicates_*` | Search, rule, select copies | `duplicates` |
| `cleanup` | cleanup | Cleanup recommendations and snapshots | `cleanup_candidates`, `snapshots_*` | Filter by reason (AND), tabs, preview | `cleanup suggest`, `cleanup empty-dirs`, `snapshot create/diff` |
| `journal` | operations | Operation history | `journal_page` | Open, undo, restore | `delete`/`shred`/`rename`/`acl` (records) |
| `shred` | operations | Irreversible destruction | `shred_preview/start/status` | Method, passes, confirmation | `shred` |
| `monitor` | operations | Write monitoring | `monitor_*`, `monitor-tick` | Start/stop, threshold, filter, sorting, open file/executable folder | `monitor` |
| `inspect` | tools | Metadata, archives, unlock, long paths | `metadata_get`, `archives_entries_page`, `unlock_locks`, `long_paths_page` | Tabs, filter, release lock | `filter --metadata`, `archives`, `unlock`, `rename`, `delete` |
| `fileops` | tools | Rename and permissions | `rename_preview/apply`, `permissions_get/preview/apply` | Tabs, preview, apply | `rename`, `acl`, `attrs` |
| `settings` | system | Settings, elevation, about | `settings_get/set`, `elevation_status/request`, `about_info` | Theme, language, units, elevation, version | `config get/set`, `--version` |

Exporting a report (`export`) is a global action available on screens that produce data
(Tree, Analysis, Duplicates, Journal), not a separate screen.

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
| Navigation | `Ctrl+1…9` — first screens of the group, `Alt+←` — back, `Alt+→` — forward |
| Search | `Ctrl+F` — focus search, `Esc` — clear/close |
| Tree | `↑/↓`, `←/→` — collapse/expand, `Home/End`, `Enter` — open, `Space` — select, `Shift+F10` — context menu |
| Table | `↑/↓`, `PageUp/PageDown`, `Home/End`, `Ctrl+A`, `Shift+↑/↓` — range, `Space` — select |
| Dialog | `Tab`/`Shift+Tab` inside, `Esc` — close, `Enter` — only on the focused button |
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
- 100/125/150/200% DPI: all 11 screens are checked; hit targets are at least 24×24 px.
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

- Full matrix: 11 screens × 2 themes × 4 DPI × 2 languages = 176 baselines.
- PR set: 8 critical screens × 2 themes × 100/150% × 2 languages + all 11 screens in the dark
  theme, 100%, RU.
- Viewport: 1440×900 (primary), 1280×800 (narrow), 1920×1080 (wide); for DPI — `deviceScaleFactor`
  emulation.
- Fonts are fixed by the system stack; font differences on CI are allowed only in a controlled
  Windows image.
- Default tolerance 0.01 (1% of pixels); for charts — 0.02.
- A baseline is updated only deliberately, with diff review; a missing baseline is a test failure.
- For each failure, the screenshot, diff and trace are saved.

## 8. Parity matrix (template)

| Screen | Capability | CLI command | Test | Screenshot | Status |
|-------|-------------|-------------|------|----------|--------|
| `overview` | Volume list and inline scanning | `drives`, `scan` | Rust + E2E | `overview-dark-100-ru` | todo |
| `tree` | Hierarchy, search, filter, size details | `tree`, `filter` | Vitest + E2E | `tree-dark-100-ru` | todo |
| `analysis` | Largest files, types, owners | `top`, `types`, `owners` | Vitest + E2E | `analysis-dark-100-ru` | todo |
| … | … | … | … | … | todo |

The matrix is filled up to 11 screen rows; absorbed screens (Scanning, Map, Size details, Owners,
Snapshots, Export, Elevation, About) remain separate capability rows. `verified` is set only after
the screen, the test and the screenshot have been accepted and the CLI behavior has been checked.
