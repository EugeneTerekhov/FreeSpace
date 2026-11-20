---
description: "Task list for the FreeSpace 002 UX recomposition (11 screens, no Treemap)"
---

# Tasks: FreeSpace — UX recomposition (11 screens, no Treemap)

**Input**: Design documents from `/specs/002-native-ui-redesign/`

**Prerequisites**: [plan.md](./plan.md) ("UX recomposition"), [spec.md](./spec.md),
[research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/),
[quickstart.md](./quickstart.md)

## Status of this plan

This is a **new plan** for feature 002. It replaces the previous 24-screen task list.

- Before the recomposition the repository contained a **24-screen** Tauri 2 + React UI with a Canvas Treemap
  (`ui/src/screens/`: `Dashboard`, `Scan`, `Treemap`, `TopFiles`, `Types`, `Owners`, `Snapshots`,
  `Export`, `Elevation`, `About`, …) and the corresponding Rust crates/Tauri commands. It is the
  historical baseline; several tasks below build on it.
- The product moves to **11 screens in 6 groups**, without the Treemap, with a grouped sidebar,
  an app icon, type icons and inline progress bars in the tree.
- The reference for the new composition is the prototype `apps/freespace-desktop/ui/demo-next/`
  (untracked). The prototype is **not** the product implementation; it is removed after the port to
  `src/` (T104).
- `[x]` is set only after implementation in `src/`, tests and green checks (constitution,
  principle V).

> **Implementation status (2026-11-18).** The recomposition is implemented: an 11-screen / 6-group
> registry, a grouped sidebar with the application icon, new tokens and monospace typography,
> Overview with embedded scanning, Tree with inline share bars and size details, the
> Analysis/Cleanup/Inspect/File operations/Settings tabs, global export, and the Treemap removed end
> to end (frontend, Tauri commands, DTOs, fixtures, benchmark). The `demo-next/` prototype is deleted
> (T104). Checks: `npx tsc --noEmit`, `npm run lint`, `check:tokens`, `check:i18n`,
> `check:network`, `npm run test:unit` (29 files / 185 tests), Playwright E2E (55 scenarios) and axe
> (24 tests, 11 screens × 2 themes plus tabs) are green; `cargo check --workspace --all-targets`,
> `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` are clean.
> Open items require visual references and manual acceptance (T100, T102, T103, T105).

**Tests**: test-first for critical modules; UI — Vitest/RTL, Playwright, axe, visual regression,
Tauri E2E.

**Format**: `[ID] [P?] [Group/Screen] Description`

- **[P]** — can run in parallel (different files, no dependencies).
- **[Group]** — Overview / Analysis / Cleanup / Operations / Tools / System or a screen ID.
- The description names the exact file path.

## Path Conventions

- Workspace crates: `crates/<name>/src/`
- GUI Rust: `apps/freespace-desktop/src-tauri/src/` (in tasks `src-tauri/...`)
- GUI React: `apps/freespace-desktop/ui/src/` (in tasks `ui/...`)
- Frontend tests: `apps/freespace-desktop/ui/tests/`
- Tauri E2E: `tests/tauri_e2e/`
- Contracts and fixtures: `specs/002-native-ui-redesign/contracts/`
- Localization: `locales/`

---

## Phase 0: Baseline (already in code) — reused

Briefly records what the recomposition builds on. Tasks are closed because the code and tests
already exist in the repository.

- [x] T001 Rust crates `freespace-{core,platform,scan,ops,analysis,report,cli,app}` and Tauri commands
- [x] T002 IPC transport, input validation, `E_*` error envelope, cursor pagination, tasks/events
- [x] T003 `[ui]` settings, migration, `data` placement, WebView2 in `data/webview2`, theme boot without flash
- [x] T004 Design tokens (dark/light/system), ui-sdk wrappers, contrast test, `check:tokens`
- [x] T005 Test infrastructure: Vitest/RTL, Playwright, axe, Tauri E2E, CLI golden fixtures
- [x] T006 NSIS installer, portable ZIP, offline WebView2, CI (fmt/clippy/test/types/lint/tests)

## Phase 1: Shell and navigation — 6 groups

- [x] T010 [Shell] Implement the registry of 11 screens and 6 groups in `ui/src/app/routing.ts` with tests
- [x] T011 [Shell] Grouped sidebar (group headings, `aria-current`) and app icon (application image `/icon.png`, decorative) in `ui/src/app/AppShell.tsx`
- [x] T012 [Shell] Deep links and command palette for 11 screens in `ui/src/app/` with tests (screen restore superseded by T014)
- [x] T013 [P] [Shell] RTL tests for navigation: groups, active screen, icon, keyboard in `ui/tests/unit/app/`
- [x] T014 [Shell] Always open the Overview on launch: ignore `last_screen`, stop writing it, update tests and FR-016

## Phase 2: Overview

- [x] T020 [Overview] Merge "Disks" + "Scanning" into `ui/src/screens/Overview.tsx` (volume cards + inline progress)
- [x] T021 [Overview] Inline scan progress: path, speed, objects, access errors, cancel in `ui/src/screens/Overview.tsx`
- [x] T022 [P] [Overview] Vitest/E2E states of Overview (loading/empty/error/partial/cancel) in `ui/tests/`

## Phase 3: Tree

- [x] T030 [Tree] Icons by object type (directory/open directory/text/image/video/archive/volume/file) in `ui/src/components/TreeRow.tsx`
- [x] T031 [Tree] Inline row progress bar: share of parent, starts after the chevron, semi-transparent fill at full row height, spacing between rows, in `ui/src/components/TreeRow.tsx` + `ui/src/screens/Tree.tsx`
- [x] T032 [Tree] "Size details" card on hover and focus with a delay, in `ui/src/screens/Tree.tsx` + `ui/src/components/DataTable.tsx`
- [x] T033 [Tree] Expand/collapse, sort, filter, search, pagination in `ui/src/screens/Tree.tsx`
- [x] T034 [P] [Tree] Tests: shares, icons, keyboard, virtualization at 10M nodes in `ui/tests/` and `tests/contract/core_commands.rs`
- [x] T035 [Tree] Row context menu (`Shift+F10` and right click): "Open in Explorer" through `reveal_in_explorer`, "Copy path", "Rename" with a preview and "Delete to Recycle Bin" with confirmation and a single deletion mode, in `ui/src/components/ContextMenu.tsx`, `ui/src/components/RenameDialog.tsx`, `ui/src/lib/clipboard.ts`, `ui/src/screens/Tree.tsx` and `crates/freespace-platform/src/reveal.rs`

## Phase 4: Analysis and Duplicates

- [x] T040 [Analysis] "Analysis" screen with "Largest files", "Types", "Owners" tabs in `ui/src/screens/Analysis.tsx`
- [x] T041 [Analysis] "Duplicates" screen with groups, "keep one copy" rule and transition to preview in `ui/src/screens/Duplicates.tsx`
- [x] T042 [P] [Analysis] Tests for tabs, filters and totals in `ui/tests/unit/screens/analysis/`

## Phase 5: Cleanup

- [x] T050 [Cleanup] "Cleanup" screen with a "Snapshots" tab (create/compare/diff) in `ui/src/screens/Cleanup.tsx`
- [x] T051 [P] [Cleanup] Tests for recommendations, risk and snapshot comparison in `ui/tests/`

## Phase 6: Operations

- [x] T060 [Operations] "Journal" screen with filters, undo and restore in `ui/src/screens/Journal.tsx`
- [x] T061 [Operations] "Shredder" screen (method, passes, confirmation) in `ui/src/screens/Shred.tsx`
- [x] T062 [Operations] "Write monitoring" screen (events, threshold, filters) in `ui/src/screens/Monitor.tsx`
- [x] T063 [P] [Operations] Operation tests on a temporary directory in `ui/tests/`

## Phase 7: Tools

- [x] T070 [Tools] "Inspect" screen: "Metadata", "Archives", "Unlock", "Long paths" tabs in `ui/src/screens/Inspect.tsx`
- [x] T071 [Tools] "File operations" screen: "Rename", "Permissions" tabs in `ui/src/screens/FileOps.tsx`
- [x] T072 [P] [Tools] Tests for read-only access, long paths and ACL in `ui/tests/`

## Phase 8: System

- [x] T080 [System] "Settings" screen with "Elevation" and "About" sections in `ui/src/screens/Settings.tsx`
- [x] T081 [System] Global "Export" action (CSV/XML/HTML/XLSX) on source screens in `ui/src/app/`
- [x] T082 [P] [System] Tests for settings, export and offline behavior in `ui/tests/`

## Phase 9: Removing the Treemap

- [x] T090 Remove the "Map" screen, `ui/src/screens/Treemap.tsx`, Canvas rendering and hit-testing, the Treemap palette
- [x] T091 Remove the `treemap_*` commands, related DTOs and routes (if not reused)
- [x] T092 Update tests/baselines/docs without the Treemap; verify the `tree` CLI is unchanged

## Phase 10: Tests, parity and documentation

- [x] T100 [P] Update Vitest/E2E/axe for 11 screens; visual baselines are not maintained
- [x] T101 Fill the 11-screen parity matrix + global "Export" in `specs/002-native-ui-redesign/contracts/parity.md`
- [x] T102 Update RU/EN localization keys and screenshots for 11 screens in `locales/` and `ui/src/i18n/`
- [x] T103 Run `quickstart.md` on the new composition (Overview/Tree/tabs) and record the results
- [x] T104 Remove the `apps/freespace-desktop/ui/demo-next/` prototype after porting to `src/`
- [x] T105 Final checks: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `npm run typecheck/lint/test:*`

---

## Dependencies & Execution Order

- **Phase 0** — already in code; everything else builds on it.
- **Phase 1** — after Phase 0; blocks the screens (it provides the 11-screen registry and groups).
- **Phase 2–8** — after Phase 1; mostly parallel across files.
- **Phase 9** — after Phase 3 and Phase 4 (the Treemap is no longer needed) and before the final baselines.
- **Phase 10** — after Phase 2–9.

## Within Each Screen

- Test first (Vitest/RTL or Rust), then implement.
- Existing 24-screen implementations are reused as a source of logic, but layout and navigation
  are brought to the 11-screen composition.
- Visual baselines — after the screen stabilizes.

## Parallel Opportunities

- Phase 1: T010, T011, T012 — different files; T013 after them.
- Phase 2–8: screens are independent across files after Phase 1.
- Phase 9: Treemap removal can run in parallel with Phase 4–8.
- Phase 10: tests/baselines/parity — parallel, final run sequential.

## Notes

- `[P]` — different files, no dependencies.
- The 24-screen UI tasks remain in git history; this plan replaces them.
- All destructive scenarios — only on temporary directories/VHDs; admin tests behind
  `FREESPACE_ADMIN_TESTS=1`.
- 0 network calls, 0 hard-coded colors, 0 strings outside localization — blocking checks.
- Commits — Conventional Commits; one task or logical group per commit.
