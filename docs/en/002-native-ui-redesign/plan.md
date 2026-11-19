# Implementation Plan: FreeSpace — migration to ui-sdk and a minimal visual language

**Branch**: `002-native-ui-redesign` | **Date**: 2026-11-18 | **Spec**: [spec.md](./spec.md)

> **This is a rewritten plan.** The `plan.md` version of 2026-11-16 described a hand-rolled UI
> kit and a decorative "aurora/bento" theme (gradients, glows, orange + violet). It is **fully
> replaced** by this document. Without changes remain the IPC contracts, the safety model
> (`freespace-ops`), CLI parity and offline delivery from `spec.md` and `contracts/`. Changes are
> the **presentation layer** and the screen set (11 instead of 24 — see "UX recomposition" below):
> (1) components are built on **ui-sdk** instead of a custom kit; (2) the visual language moves
> toward **clarity, space and functional color**.

> **Decision record (v2, 2026-11-18).** The Treemap (heatmap) is removed from the product
> entirely; data exploration happens only through the **Tree** with **inline progress bars**. The
> screens are recomposed from **24 down to 11** and grouped in the sidebar under **6 groups**.
> See the "UX recomposition" section below; it supersedes every mention of "24 screens" and
> "Treemap" in this plan.

## Summary

FreeSpace keeps its core (Rust: scanning, size accounting, operations, journal, quarantine) and
its shell (Tauri 2 + React 19 + TypeScript + Vite) unchanged. The presentation layer moves from a
custom component kit to **ui-sdk** (`@ui-sdk/react` + `@ui-sdk/core`, already installed, version
`0.1.0-beta.10`): interactive components (buttons, fields, selects, dialogs, tabs, menus,
tooltips, toasts) are taken ready-made from ui-sdk, while product-specific components (the
virtualized table/tree, the command palette) are built as primitives on
`@ui-sdk/core` with the same keyboard, focus, ARIA and agent-bridge guarantees.

At the same time the visual system is reworked: a single **JetBrains Mono** typeface, a spacious
grid, minimal functional color following the **60-30-10** rule, two full themes (Light / Dark) on
complex neutral shades (no pure `#000000`/`#FFFFFF`), tokens in **OKLCH**, and **WCAG AA** contrast
(4.5:1 text, 3:1 non-text). Decorative effects (gradients, radial glows, button "lift", cascading
entrances) are removed: color and motion remain only where they carry function.

## UX recomposition: dropping the Treemap and reducing the screens

> **Decision record (2026-11-18, v2).** This section supplements and **supersedes** every place
> in the plan that mentions "24 screens" and "Treemap". The `spec.md` specification is updated
> next: FR-035…FR-043 are repurposed for the tree with inline progress bars, FR-023 (screen list)
> is corrected, and SC-004 (30 fps tree), SC-008/SC-016 (DPI and screenshots) and Assumptions are
> adjusted. Until that edit lands, this section is the source of truth for the presentation layer.

### Decision 1 — The Treemap (heatmap) is removed entirely

The heatmap is dropped from the product. Data is explored only through the **Tree** with **inline
progress bars**. Reasons: the map reads poorly on dense data, duplicates the tree's navigation,
requires a separate Canvas implementation (hit-testing, legend, single-hue ramp) and violates the
"clarity over decoration" principle. `TreemapCanvas`, `treemap/palette`, `treemap/keyboard`, the
"Map" screen and FR-035…FR-043 are removed. The `tree` CLI contract is unchanged.

### Decision 2 — Inline progress bars instead of a separate "Scan" screen

Scan progress is no longer a dedicated screen. It is shown inline:

1. **Overview** — a progress line under the selected volume (current path, speed, objects, cancel).
2. **Shell status bar** — the aggregate progress of the active task and the current path.
3. **Tree** — inline progress bars on expandable nodes: the node's share of its parent and of the
   volume.

The user sees gradual progress and cancels scanning from anywhere without switching screens.

### Decision 3 - App icon in the sidebar

The sidebar brand block shows the **application image** (`/icon.png`), not just a colored tile.
The image is decorative and hidden from screen readers (the container has `aria-hidden="true"`
and the image has an empty `alt`).

### Decision 4 — Screen recomposition: 24 → 11, hierarchy by groups

Screens that are close in meaning are merged; the sidebar gains a **two-level hierarchy** — group
headers with screens inside.

| Group | Screens | Absorbed screens (of 24) |
|-------|---------|--------------------------|
| **Overview** | Overview | Disks + Scan (inline progress) |
| **Analysis** | Tree · Analysis · Duplicates | Tree + Size details (hover card); Analysis = Largest files + Types + Owners |
| **Cleanup** | Cleanup | Cleanup + Snapshots (tabs) |
| **Operations** | Journal · Shredder · Write monitor | — |
| **Tools** | Inspect · File operations | Inspect = Metadata + Archives + Unlock + Long paths; File operations = Rename + Permissions |
| **System** | Settings | Settings + Elevation + About |

Report export is not a screen but a **global action** (an "Export" button on the screens that
produce data: Tree, Analysis, Duplicates, Journal).

**Result: 11 screens in 6 groups** (was 24). Removed as standalone screens: Map, Scan, Size
details, Owners, Snapshots, Export, Elevation, About.

### Sidebar structure

```text
Overview
  Overview
Analysis
  Tree
  Analysis
  Duplicates
Cleanup
  Cleanup
Operations
  Journal
  Shredder
  Write monitor
Tools
  Inspect
  File operations
System
  Settings
```

Groups are static headers (collapsing is optional but allowed); the active screen is highlighted
with the accent and `aria-current`. Screen identifiers (`ScreenId`) shrink to this list; the parity
matrix and CLI analogs are kept row by row — every absorbed screen remains a row of the
"screen → capability → CLI command" matrix.

## 1. Design Principles

These seven rules are the source of truth for every visual decision and review:

1. **Clarity over decoration.** Any ornament that gets in the way of reading data is removed.
2. **Spacious space.** Margins and "air" give freedom of movement and keep data from running together.
3. **Minimal functional color.** Color signals state or action, not mood.
4. **Clear typographic hierarchy.** A single scale, no random sizes or weights.
5. **Unobtrusive, purposeful interaction.** No "tricks": highlight, hover and motion only where
   they help convey state.
6. **Pixel-level alignment.** A grid and baseline are mandatory for every element.
7. **Every element has a reason to exist.** A superfluous element is a defect, not a detail.

## 2. Design System

### 2.1 Color

The color canon lives in `ui/src/styles/tokens.css` and `themes.css`. All components must use only
`var(--fs-*)`; the `check:tokens` script forbids hard-coded colors outside these files. Brand tones
are specified in HEX (exactly from the product palette); intermediate steps are derived with
`color-mix(in oklch, ...)`, which keeps OKLCH's smooth transitions.

Palette requirements (mandatory):

1. Two full themes: **Light** and **Dark** (plus system, inherited from `spec.md`).
2. No pure `#000000`/`#FFFFFF` on large surfaces — a five-tone palette per theme (dark: teal ->
   cream; light: brown/olive -> greige) with derived surface steps.
3. **60-30-10** balance: 60% background, 30% surfaces/cards, 10% accent and active states.
4. Text and interactive element contrast of at least **WCAG AA** (4.5:1 text, 3:1 large text and
   borders/states). Verified by an automated contrast test.
5. Brand tones are HEX; derived shades use `color-mix(in oklch, ...)`; transparency only for lines,
   selections and overlays.

#### 2.1.1 Palette anchors

| Theme | Anchors (dark -> light) |
|-------|--------------------------|
| Dark | `#344b52`, `#587983`, `#9db3b4`, `#cfd3ca`, `#fae8da` |
| Light | `#362E23`, `#3A3C26`, `#6D6C3C`, `#A9AC5D`, `#E8E1DB` |

In the dark theme `#344b52` is the background, `#587983` the top surface step, `#9db3b4`/`#cfd3ca`
the captions, `#fae8da` the text and accent. In the light theme `#E8E1DB` is the background,
`#362E23` the text, `#6D6C3C`/`#3A3C26` the secondary text and accent, `#A9AC5D` the selection.

#### 2.1.2 Dark theme tokens

| Token | Value | Purpose |
|-------|-------|---------|
| `--fs-bg` | `#344b52` | Window background |
| `--fs-surface` | `#3a535a` | Surfaces |
| `--fs-surface-raised` | `#405d65` | Cards, panels |
| `--fs-surface-overlay` | `#4a6b74` | Menus, popovers, hover |
| `--fs-line` | `rgb(250 232 218 / 0.12)` | Dividers, borders |
| `--fs-line-strong` | `rgb(250 232 218 / 0.2)` | Active borders |
| `--fs-text` | `#fae8da` | Primary text |
| `--fs-text-muted` | `#cfd3ca` | Secondary text |
| `--fs-text-subtle` | `#c6d3d2` | Captions, hints |
| `--fs-accent` | `#fae8da` | Primary fill, active |
| `--fs-accent-strong` | `#fae8da` | Accent text/icon, focus |
| `--fs-accent-ink` | `#344b52` | Text on accent fill |
| `--fs-focus` | `#cfd3ca` | Focus ring |
| `--fs-selection` | `rgb(250 232 218 / 0.16)` | Selected row/tile |
| `--fs-success` | `#b8dcc0` | Success |
| `--fs-danger` | `#f5c4b4` | Error, destructive action |
| `--fs-warning` | `#edcf92` | Warning |
| `--fs-info` | `#b9d6da` | Info |

#### 2.1.3 Light theme tokens

| Token | Value | Purpose |
|-------|-------|---------|
| `--fs-bg` | `#e8e1db` | Window background |
| `--fs-surface` | `#f2ede9` | Surfaces |
| `--fs-surface-raised` | `#f7f3f0` | Cards, panels |
| `--fs-surface-overlay` | `#dcd5cc` | Menus, popovers, hover |
| `--fs-line` | `rgb(54 46 35 / 0.12)` | Dividers, borders |
| `--fs-line-strong` | `rgb(54 46 35 / 0.22)` | Active borders |
| `--fs-text` | `#362e23` | Primary text |
| `--fs-text-muted` | `#5d5935` | Secondary text |
| `--fs-text-subtle` | `#636138` | Captions, hints |
| `--fs-accent` | `#5d5935` | Primary fill, active |
| `--fs-accent-strong` | `#5d5935` | Accent text/icon, focus |
| `--fs-accent-ink` | `#e8e1db` | Text on accent fill |
| `--fs-focus` | `#6d6c3c` | Focus ring |
| `--fs-selection` | `rgb(169 172 93 / 0.22)` | Selected row/tile |
| `--fs-success` | `#2f6b3f` | Success |
| `--fs-danger` | `#a8442f` | Error, destructive action |
| `--fs-warning` | `#7a5c12` | Warning |
| `--fs-info` | `#2f5f6b` | Info |

#### 2.1.4 Color usage rules

- **One accent.** The only expressive tone is `--fs-accent*`: cream `#fae8da` in the dark theme (on
  `#344b52`) and dark olive `#5d5935` in the light theme (on `#e8e1db`). Applied to the primary
  button, the active/selected state, the focus ring and links — the "10%" of the 60-30-10 rule.
- **Semantic colors** (`--fs-success/danger/warning/info`) are strictly functional and muted to the
  palette: status, operation result, risk level. Never decoration; always accompanied by text and
  an icon.
- **Surfaces carry 90% of the interface.** Level separation (bg -> surface -> raised -> overlay) is
  achieved by lightness within the palette, not by adding color.
- **Contrast (verified):** dark — text 7.75:1, muted 6.08:1, subtle 6.01:1 against the background
  (and 4.59:1 on raised); light — 10.32:1, 5.51:1, 4.92:1; semantic colors and the accent pair
  are >= 4.5:1 on surfaces/raised.
- **Scrollbar:** the track is `--fs-bg` (blends with the background), the thumb is a palette step
  derived from `--fs-text-subtle` and the background.
- **The tree progress-bar palette** is a semi-transparent row fill within the palette; the
  selected row is marked by the accent and a ring, not a rainbow.
- **Transparency** is allowed only for `--fs-line`, `--fs-selection` and overlays; primary text and
  fills are opaque.

### 2.2 Typography

A single typeface **JetBrains Mono** (weights 400/500/600/700), with no separate sans/display. The
choice is grounded in the product's nature: paths, sizes, tables and numbers benefit from
monospacing and tabular figures; one typeface removes "randomness" from the hierarchy.

| Token | Value |
|-------|-------|
| `--fs-font-mono` | `"JetBrains Mono", "Cascadia Mono", "Consolas", ui-monospace, monospace` |
| `--fs-weight-regular` | `400` |
| `--fs-weight-medium` | `500` |
| `--fs-weight-semibold` | `600` |
| `--fs-weight-bold` | `700` |

Type scale (modular, ratio ~1.2, anchored at 13px):

| Token | Size/line-height | Weight | Role |
|-------|------------------|--------|------|
| `--fs-text-2xs` | `11px / 15px` | 500 | Overlines, tree labels, badges, kbd |
| `--fs-text-xs` | `13px / 17px` | 400 | Table cells, dense lists, metadata |
| `--fs-text-sm` | `15px / 20px` | 400 | Secondary text, descriptions |
| `--fs-text-base` | `16px / 22px` | 400 | Body text, fields, forms |
| `--fs-text-md` | `19px / 26px` | 500 | List items, navigation, buttons |
| `--fs-text-lg` | `23px / 30px` | 600 | Section titles |
| `--fs-text-xl` | `27px / 34px` | 600 | Panel titles |
| `--fs-text-2xl` | `33px / 40px` | 600 | Screen title (h1) |
| `--fs-text-3xl` | `40px / 46px` | 700 | Large numbers/statistics |
| `--fs-text-4xl` | `48px / 54px` | 700 | Large dashboard statistic |

Rules:

- Numbers and sizes are always tabular (`font-variant-numeric: tabular-nums`; JetBrains Mono is
  monospaced by default).
- Section overlines use `2xs`, `500`, uppercase, `0.08em` letter-spacing.
- One `h1` per screen (`2xl`); the heading hierarchy strictly follows the scale with no skipped
  levels.
- Sizes/paths/codes use the monospace, at most one step smaller than the neighboring value.
- JetBrains Mono is shipped **locally** (woff2, only the needed weights) in `ui/src/assets/fonts/`;
  no CDN and no network requests. Fallback is the system monospace.

### 2.3 Spacing and rhythm

A single spacing scale on an **8px** grid (a 4px half-step for micro-alignment):

| Token | Value | Token | Value |
|-------|-------|-------|-------|
| `--fs-space-1` | `4px` | `--fs-space-6` | `24px` |
| `--fs-space-2` | `8px` | `--fs-space-8` | `32px` |
| `--fs-space-3` | `12px` | `--fs-space-10` | `40px` |
| `--fs-space-4` | `16px` | `--fs-space-12` | `48px` |
| `--fs-space-5` | `20px` | `--fs-space-16` | `64px` |

Alignment rules:

- Outer margins are multiples of 8px; 4px is only for fitting icons/captions to the baseline.
- Text sits on a 4px baseline; containers are aligned to the 8px grid.
- Page horizontal padding is 24px; the vertical step between sections is 32px.
- Density: comfortable (36px rows) / compact (30px) via `data-density`, without changing the grid.
- App header 56px, sidebar 240px (collapsed 64px), status bar 28px.

### 2.4 Layout architecture

**Page structure (top to bottom):**

1. Native Windows window title (system buttons live outside the app).
2. Content header: screen title, contextual actions (right).
3. Context strip: current volume, scan state, active tasks.
4. Main area (primary section) — the screen's data.
5. Details panel (right) — where the screen requires it.
6. Status bar (bottom): state, progress, notifications.

**Grid system:** 12 columns, 24px gutter, 24px side margins, container up to 1440px. The sidebar
sits outside the grid (fixed 240/64px). The main area is divided into 12 columns; cards and panels
occupy whole column counts.

**Section hierarchy:** screen -> section (an `lg` title + a `2xs` overline) -> panel/card
(`--fs-surface-raised` + `--fs-line`) -> component. Sections are separated by 32px; inside a panel
16px.

### 2.5 Radii, borders, shadows, motion

| Group | Values |
|-------|--------|
| Radii | `--fs-radius-sm: 4px`, `md: 6px`, `lg: 8px`, `pill: 999px` |
| Borders | `1px solid var(--fs-line)`; strong — `--fs-line-strong` |
| Shadows | functional only: dialog/overlay `--fs-shadow-overlay`; decorative shadows removed |
| Layers | `--fs-z-sticky: 10`, `dropdown: 100`, `overlay: 200`, `dialog: 300`, `toast: 400`, `tooltip: 500` |
| Motion | `--fs-motion-fast: 120ms`, `--fs-motion-base: 180ms`, `--fs-ease: cubic-bezier(.2,.8,.2,1)`, `--fs-motion-reduced: 0ms` |

Rules: transitions only for functional state changes (open/close, hover, focus);
`prefers-reduced-motion: reduce` or `reduced_motion=on` disables non-essential transitions. No
cascading entrances, no count-up, no button "lift".

## 3. Component Architecture (ui-sdk)

ui-sdk provides the interactive behavior (keyboard, focus, ARIA, dismissal, popups, agent-bridge);
the app only supplies markup and styles via tokens. Styling uses `data-slot` and state attributes
(`data-open`, `data-pressed`, `data-highlighted`, ...) and CSS variables, never generated ids.

The global stylesheet declares `@layer ui-sdk;` as its first line (above any imports), so the app's
own styles always win.

### 3.1 Custom kit -> ui-sdk component mapping

| Current component | ui-sdk component (entry) |
|-------------------|--------------------------|
| `Button` / `IconButton` | `@ui-sdk/react/button` (`Button`) |
| `Field` / `Fields` | `@ui-sdk/react/field` (`Field`, `FieldLabel`, `FieldDescription`, `FieldError`) |
| `SearchField` | `Field` + `@ui-sdk/react/input`, or `@ui-sdk/react/combobox` |
| `Select` | `@ui-sdk/react/select` |
| `Combobox` | `@ui-sdk/react/combobox` |
| `Segmented` | `@ui-sdk/react/segmented-control` or `toggle-group` |
| `Tabs` | `@ui-sdk/react/tabs` |
| `Toggle` | `@ui-sdk/react/switch` (or `toggle`) |
| `Checkbox` | `@ui-sdk/react/checkbox` |
| `Radio` | `@ui-sdk/react/radio` (radio-group) |
| `Tooltip` | `@ui-sdk/react/tooltip` |
| `Dialog` | `@ui-sdk/react/dialog` |
| `ConfirmDialog` | `@ui-sdk/react/alert-dialog` |
| `Toast` / `Feedback` | `@ui-sdk/react/toast` + `OverlayHost` (`@ui-sdk/react/overlays`) |
| `ContextMenu` | `@ui-sdk/react/menu` (or `trigger-menu`) |
| `CommandPalette` | `@ui-sdk/react/command` |
| `Progress` | `@ui-sdk/react/progress` |
| `Meter` | `@ui-sdk/react/meter` |
| Dividers | `@ui-sdk/react/separator` |
| List scrolling | `@ui-sdk/react/scroll-area` |
| SplitPane | `@ui-sdk/react/resizable` |

### 3.2 Primitives on `@ui-sdk/core` (what ui-sdk does not ship)

- **`DataTable` / `TreeTable`** — the virtualized table and tree. ui-sdk has no table/data-grid, so
  this is a custom primitive: `createGridFocus`/`createTreeCollection` + `createSelection` +
  `createTypeahead` from `@ui-sdk/core`, virtualization via the existing
  `@tanstack/react-virtual`. Rendered via `createParts` from `@ui-sdk/react/primitive`.
- **`TreeRow`** — a virtualized tree row with a type icon and a semi-transparent inline progress
  bar; focus and keyboard control are a primitive on `@ui-sdk/core` (roving focus), so the tree is
  accessible and visible to the agent-bridge.
- **`Badge` / `Chip` / `Kbd` / `Card` / `EmptyState` / `ErrorState` / `Banner`** — presentational;
  built as thin non-interactive wrappers (interactive variants use `Toggle`/`Button`).

### 3.3 Agent bridge

`createAgent()` from `@ui-sdk/react/agent` is connected once at startup. Every interactive element
gets a name (label/title/`aria-label`); destructive actions are marked with `data-agent-confirm`.
This provides screen-reader accessibility and lets the UI be scripted without bypassing safety:
changes go through the same callbacks and can be refused (`details.cancel()`, `allow`).

## 4. Technical Context

**Language/Version**: Rust 1.85+ (edition 2024), TypeScript 5.6+, Node.js 20.19+, Windows x64.

**Primary Dependencies**:

- UI: Tauri 2.x, **React 19.3** (a ui-sdk requirement), React DOM 19.3, TypeScript 5.6, Vite 6.x,
  `@tauri-apps/api` 2.x.
- Components/behavior: **`@ui-sdk/react` + `@ui-sdk/core` `0.1.0-beta.10`** (already in
  `package.json`; the version is pinned — it is a beta, upgrades only after changelog review).
- Tables/virtualization: `@tanstack/react-table` 8.x, `@tanstack/react-virtual` 3.x.
- State: `zustand` 5.x (per-screen stores + shell store).
- Icons: `lucide-react` 0.468 (local, single set).
- Font: **JetBrains Mono** — local woff2 in `ui/src/assets/fonts/` (weights 400/500/600/700).
- Localization: `locales/ru.toml`/`en.toml` + `scripts/gen-i18n.mjs`.
- Tests: `vitest`, RTL, `jsdom`, `@playwright/test`, `@axe-core/playwright`, `tauri-driver`.
- Rust: `tauri`, the existing `freespace-core/platform/scan/ops/analysis/report/cli` +
  `freespace-app`.

**Storage**: unchanged — a single `data` directory next to the exe; the WebView2 profile in
`data/webview2`; `localStorage`/`IndexedDB` outside `data` are not used.

**Testing**: `cargo fmt/clippy/test` (core), `vitest`+RTL (components/screens), Playwright (visual
regression), axe (accessibility), a token contrast test, `check:tokens`/`check:i18n`/`check:network`,
admin tests behind `FREESPACE_ADMIN_TESTS=1`.

**Target Platform**: Windows 10 21H2+/Windows 11 x64; WebView2 (offline) at least Chromium 111
(for `oklch()`).

**Performance Goals**: response <= 200 ms; tree >= 30 fps at 10M nodes; memory <= 4 GB; first
screen <= 3 s; 8 TB analysis <= 60 s; events throttled to 10-20 Hz. Unchanged.

**Constraints**: offline/no telemetry; CSP `default-src 'self'` + a nonce for ui-sdk CSS
(meta `csp-nonce` / `html.cspNonce` in Vite); WinAPI only in `freespace-platform`; destructive
operations only via `freespace-ops`; one GUI (egui removed); the CLI contract unchanged.

**Scale/Scope**: 11 screens in 6 groups, RU/EN, 3 theme modes, DPI 100-200%, up to 10M nodes; one
GUI + CLI.

## 5. Constitution Check

| Principle | Conformance | Rationale |
|-----------|-------------|-----------|
| I. Destructive operation safety | PASS | ui-sdk is presentation only; preview/confirm/journal/quarantine stay in `freespace-ops`. `data-agent-confirm` and `allow` keep an agent/script from bypassing confirmation. |
| II. Accurate space accounting | PASS | Sizes are computed by Rust; ui-sdk does not affect calculations. |
| III. Performance at scale | PASS | ui-sdk adds no overhead; virtualization and Canvas remain, budgets unchanged. |
| IV. Native Windows accuracy | PASS | Tauri + WebView2 remain (ADR 0001); ui-sdk does not constrain the WinAPI core. |
| V. Test-first approach | PASS | Tests are written before replacing each component; ui-sdk provides `@ui-sdk/core/testing` for keyboard/focus/ARIA. |
| VI. CLI/GUI parity | PASS | A single core; the 11-screen parity matrix is unchanged. |
| VII. Privacy and offline | PASS | No network; JetBrains Mono is shipped locally, ui-sdk is an npm dependency bundled offline. |
| Tech constraints: UI — egui/eframe, Tauri with justification | PASS (ADR 0001) | Tauri is already justified by ADR `docs/adr/0001-tauri-react-gui.md`; ui-sdk is a library on top of React, no new ADR required, justification recorded in `research.md`. |

## 6. Project Structure

```text
apps/freespace-desktop/ui/src/
├── app/                    # shell, routing, providers, boot (logic unchanged)
├── screens/                # 11 screens in 6 groups — restyled onto new tokens/ui-sdk
├── components/             # only §3.2 primitives and thin wrappers remain
│   ├── ui/                 # wrappers over ui-sdk (Button, Field, Select, ...) with tokens
│   ├── DataTable.tsx       # primitive on @ui-sdk/core + @tanstack/react-virtual
│   ├── TreeTable.tsx
│   └── ...
├── styles/
│   ├── tokens.css          # OKLCH tokens, scale, grid, radii, layers
│   ├── themes.css          # data-theme="dark|light"
│   └── global.css          # @layer ui-sdk; + base resets
├── api/  stores/  i18n/    # unchanged
└── assets/fonts/           # JetBrains Mono woff2 (400/500/600/700)
```

## 7. Delivery Phases

| Phase | Content | Outcome |
|-------|---------|---------|
| 0. Foundation | New OKLCH tokens (both themes), `@layer ui-sdk`, font, CSP nonce, contrast test, `check:tokens` | Tokens and themes accepted, AA contrast |
| 1. ui-sdk primitives | Kit replacement: Button, Field/Input, Select, Combobox, Checkbox, Switch, Tabs, Dialog, AlertDialog, Toast, Tooltip, Menu, Command, Progress, Meter, Separator, ScrollArea, Resizable | Interactive layer on ui-sdk |
| 2. Core primitives | DataTable/TreeTable (grid focus + virtual), tree keyboard, command palette, agent bridge | Product-specific components ready |
| 3. Visual language | Spacious grid, JetBrains Mono typography, 11-screen restyle, decoration removal | A single minimal style |
| 4. Accessibility | axe on 11 screens, Narrator scenarios, agent test (`createAgent` + snapshot/act) | AA + agent-ready |
| 5. Regression and acceptance | Regenerate visual baselines, full run, update the `docs/en/` translation, acceptance | Visual layer release |

## 8. Complexity Tracking

| Decision | Why needed | Simpler alternative rejected because |
|----------|------------|--------------------------------------|
| ui-sdk instead of a custom kit | Ready-made keyboard/focus/ARIA/agent-bridge guarantees; less code and fewer behavior tests. | A custom kit duplicates behavior and accessibility; more expensive to maintain. |
| DataTable/TreeRow primitives on `@ui-sdk/core` | ui-sdk ships no table/canvas; focus/selection behavior comes from core, rendering stays with the product. | Hand-writing ARIA/focus risks accessibility bugs. |
| JetBrains Mono as the single typeface | Data/paths/tables benefit from monospacing; one scale with no randomness. | A sans+mono pair introduces a second hierarchy and lacks tabular figures for UI text. |
| OKLCH tokens | A modern color space, smooth transitions; a palette requirement. | HEX-only is weaker at managing lightness and consistency. |
| Local font shipping | Offline (principle VII). | CDN/webfont violates the offline requirement. |
| Semi-transparent tree progress bar | Minimal functional color; the share reads through lightness, accent marks selection. | 10 gradients/rainbow is decorative color, contradicting principle 3. |

## 9. Post-Design Constitution Re-Check

After phases 0-2, re-verify that:

- all destructive actions go through `freespace-ops` and an `alert-dialog` with `data-agent-confirm`;
- contrast is 4.5:1 (text) / 3:1 (non-text) in both themes across all 11 screens;
- the CSP with nonce does not block ui-sdk CSS and does not allow network requests;
- `@layer ui-sdk` is declared first, so the app's own styles win;
- JetBrains Mono is shipped locally, without a CDN;
- virtualization and Canvas fit the 200 ms / 30 fps / 4 GB budgets;
- the agent bridge does not bypass safety (`allow`, `details.cancel()`).

## 10. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `@ui-sdk/*` is a beta; the API may change | Version pinned; upgrades only after changelog review and tests. |
| CSP blocks ui-sdk CSS | Meta `csp-nonce` / `html.cspNonce`; `@layer ui-sdk`; a CSP test in E2E. |
| WebView2 older than Chromium 111 lacks `oklch()` | Offline runtime >= 111; HEX fallback for older environments; version check. |
| Migration breaks existing screens | Component-by-component replacement; old tests stay as contract until replaced. |
| Visual language regression | Regenerate baselines after stabilization; `check:tokens` blocks hard-coded colors. |
| Font increases delivery size | Glyph subsetting, only needed weights; measured in research. |
| Lower density (spaciousness) vs data | Comfortable/compact via `data-density`; tables stay virtualized. |
| Accessibility of custom primitives | `@ui-sdk/core/testing` (keyboard/focus/ARIA) + axe on all screens. |
