---
description: "Task list for FreeSpace native UI redesign 002"
---

# Tasks: FreeSpace — a single next-generation native interface

**Input**: Design documents from `/specs/002-native-ui-redesign/`

**Prerequisites**: [plan.md](./plan.md) (required), [spec.md](./spec.md) (required), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Test tasks are mandatory. The constitution, principle V, and FR-115…FR-122 require a
test-first approach for critical modules; the UI is verified with Vitest/RTL, Playwright, axe,
visual regression, and Tauri E2E.

**Organization**: Tasks are grouped by user story. The old egui interface is removed in Phase 1; no
second UI, beta flag, or side-by-side is created. Until the 24 screens are complete the application
is not released, and the old UI is not a fallback.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependencies)
- **[Story]**: user story (US1…US11)
- The description specifies the exact file path

## Path Conventions

- Workspace crates: `crates/<name>/src/`
- GUI Rust: `apps/freespace-desktop/src-tauri/src/`; in tasks, `src-tauri/...` is shorthand for this path
- GUI React: `apps/freespace-desktop/ui/src/`; in tasks, `ui/...` is shorthand for this path
- Frontend tests: `apps/freespace-desktop/ui/tests/`
- Tauri E2E: `tests/tauri_e2e/`
- Contracts and fixtures: `specs/002-native-ui-redesign/contracts/`
- Localization: `locales/`
- Documentation: `docs/`, `ru/`, `en/`

---

## Phase 1: Setup (shared infrastructure)

**Purpose**: Approve the ADR, replace the old GUI with the new skeleton, set up the build and tests.

- [ ] T001 Review and approve ADR `docs/adr/0001-tauri-react-gui.md` (decision, alternatives, Constitution Check, WebView2, security, full replacement of egui)
- [ ] T002 Update the workspace in `Cargo.toml`: remove `apps/freespace-gui`, add `apps/freespace-desktop` and `crates/freespace-app`, update `default-members`
- [ ] T003 [P] Move the pure Treemap geometry and helper functions from `apps/freespace-gui/src/treemap/layout.rs` to `crates/freespace-app/src/treemap/layout.rs` with tests
- [ ] T004 Delete `apps/freespace-gui/` and the unused `eframe`/`egui` dependencies from `Cargo.toml`; update `Cargo.lock`; record that git history is the source of parity
- [ ] T005 [P] Create the `crates/freespace-app` skeleton (`Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/dto/mod.rs`, `src/treemap/mod.rs`)
- [ ] T006 [P] Create the Tauri application skeleton `apps/freespace-desktop/src-tauri/` (`Cargo.toml`, `build.rs`, `src/main.rs`, `src/lib.rs`, `tauri.conf.json`, `capabilities/default.json`)
- [ ] T007 [P] Create the React/Vite skeleton `apps/freespace-desktop/ui/` (`package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/main.tsx`, `src/App.tsx`, `public/boot.js`)
- [ ] T008 Add frontend dependencies and `package-lock.json`; configure the `dev`, `build`, `tauri`, `typecheck`, `lint`, `test:unit`, `test:e2e`, `test:visual`, `test:a11y`, `check:i18n`, `check:tokens`, `check:network` scripts
- [ ] T009 Configure strict TypeScript and ESLint (`eslint`, `typescript-eslint`, `eslint-plugin-react-hooks`, `eslint-plugin-react-refresh`) in `apps/freespace-desktop/ui/eslint.config.js`
- [ ] T010 Configure Vitest + React Testing Library + jsdom in `apps/freespace-desktop/ui/vitest.config.ts` and `tests/setup.ts`
- [ ] T011 Configure Playwright in `apps/freespace-desktop/ui/playwright.config.ts` (Vite webServer, chromium projects, screenshots, trace)
- [ ] T012 Configure `@axe-core/playwright` and the visual baseline catalog `apps/freespace-desktop/ui/tests/visual/`
- [ ] T013 Create the localization generator `apps/freespace-desktop/ui/scripts/gen-i18n.mjs` (TOML → `src/i18n/generated/{ru,en,keys}.ts`) and the `check:i18n` check
- [ ] T014 [P] Create the `check:tokens` (hardcoded colors ban) and `check:network` (network guard) checks in `apps/freespace-desktop/ui/scripts/`
- [ ] T015 Move icons and splash from `apps/freespace-gui/assets/` to `apps/freespace-desktop/src-tauri/icons/` and `ui/public/`; update references in `tauri.conf.json`
- [ ] T016 Configure CI in `.github/workflows/ci.yml`: Rust (fmt, clippy, test), frontend (npm ci, typecheck, lint, unit), Tauri E2E, visual/a11y, caches
- [ ] T017 Update `.gitignore` for `apps/freespace-desktop/ui/node_modules/`, `dist/`, `test-results/`, `playwright-report/`; do not ignore the generated i18n catalogs
- [ ] T018 Configure CSP, `withGlobalTauri: false`, minimal capabilities, and the release devtools configuration in `apps/freespace-desktop/src-tauri/tauri.conf.json`
- [ ] T019 [P] Add the base npm dependencies for E2E (`tauri-driver`, `webdriverio`) and the `tests/tauri_e2e/wdio.conf.ts` configuration
- [ ] T020 [P] Create the contract fixture catalogs `tests/contract/` and a stub for the golden command checks

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: Application layer, IPC, security, settings, UI kit, test infrastructure.
**⚠️ CRITICAL**: No user story starts before this phase is complete.

- [ ] T021 Implement the error envelope and `E_*` codes in `crates/freespace-app/src/error.rs` with tests
- [ ] T022 [P] Implement validation of paths, enums, sizes, and limits in `crates/freespace-app/src/validation.rs` with tests
- [ ] T023 [P] Implement cursor pagination and `generation` in `crates/freespace-app/src/paging.rs` with tests
- [ ] T024 [P] Implement the DTOs for pages, nodes, tiles, operations, and journal in `crates/freespace-app/src/dto/` with serde tests
- [ ] T025 Implement the task registry, cancellation, and progress in `apps/freespace-desktop/src-tauri/src/state.rs` with tests
- [ ] T026 [P] Implement events and throttle/sequence in `apps/freespace-desktop/src-tauri/src/events.rs` with tests
- [ ] T027 [P] Implement path canonicalization, the command allowlist, and scope protection in `apps/freespace-desktop/src-tauri/src/security.rs` with tests
- [ ] T028 Implement the `[ui]` migration in `crates/freespace-app/src/settings.rs`: `settings.toml.bak` backup, schema version, idempotency, tests
- [ ] T029 [P] Implement window state persistence and validation in `crates/freespace-app/src/window_state.rs` with tests
- [ ] T030 Implement boot injection of theme/language/reduced motion in `apps/freespace-desktop/src-tauri/src/boot.rs` with tests
- [ ] T031 Implement WebView2 placement in `data/webview2` and the offline arguments in `apps/freespace-desktop/src-tauri/src/main.rs`; record the spike result in `research.md`
- [ ] T032 Implement `app_bootstrap`, `app_capabilities`, `app_settings_*`, `app_window_state_*` in `apps/freespace-desktop/src-tauri/src/commands/app.rs` with golden tests
- [ ] T033 Create design tokens and themes in `apps/freespace-desktop/ui/src/styles/{tokens,themes,global}.css`; contrast test in `tests/unit/tokens.test.ts`
- [ ] T034 [P] Implement the UI kit, part 1: Button, IconButton, Field, SearchField, Select, Combobox, Toggle, Checkbox, Radio, Segmented, Tabs, Chip, Badge, Tooltip, Kbd in `ui/src/components/` with RTL tests
- [ ] T035 [P] Implement the UI kit, part 2: Progress, Meter, Card, StatCard, SummaryCard, Dialog, ConfirmDialog, Toast, Banner, EmptyState, ErrorState, Loading/Skeleton in `ui/src/components/` with RTL tests
- [ ] T036 Implement AppShell, Sidebar, Header, StatusBar, Breadcrumbs, SplitPane in `ui/src/app/` with RTL tests
- [ ] T037 Implement the `TauriTransport` and `MockTransport` transports plus a typed client in `ui/src/api/` with tests
- [ ] T038 Implement the Zustand shell/tasks/settings stores in `ui/src/stores/` with tests
- [ ] T039 Implement the registry of 24 screens, navigation, last-screen restore, and deep links in `ui/src/app/routing.ts` with tests
- [ ] T040 Implement the error boundary, error normalization, and local logging in `ui/src/app/ErrorBoundary.tsx` with tests
- [ ] T041 Implement the i18n runtime, `MessageKey`, `Intl` formatting, and a missing-key test in `ui/src/i18n/`
- [ ] T042 Implement `boot.js`, theme synchronization, and a no-flash test in `ui/public/boot.js` and `tests/unit/theme-boot.test.ts`
- [ ] T043 Implement the command palette and keyboard shortcut registry in `ui/src/app/CommandPalette.tsx` with tests
- [ ] T044 Create test fixtures, render helpers, and MockTransport scenarios in `ui/tests/helpers/`
- [ ] T045 [P] Create Playwright page objects and fixtures for the 24 screens in `ui/tests/e2e/pages/`
- [ ] T046 [P] Create the visual regression harness (viewports, themes, DPI, tolerances) in `ui/tests/visual/harness.ts`
- [ ] T047 [P] Create the accessibility helper (axe + keyboard) in `ui/tests/a11y/axe.ts`
- [ ] T048 [P] Create the 10 million node synthetic tree generator in `crates/freespace-app/examples/gen_fixture.rs` and the measurement harness
- [ ] T049 Create golden fixtures for success/errors of all commands in `tests/contract/fixtures/` and a test that the client matches `app_capabilities`

**Checkpoint**: the foundation is ready — stories can be implemented in parallel.

---

## Phase 3: User Story 1 — Core: disks, scanning, tree, map (P1) 🎯 MVP

**Goal**: volume dashboard, starting/cancelling a scan, virtualized tree, Canvas Treemap, and details panel.

**Independent Test**: on a test VHD/temporary directory, scan a known structure, compare totals, verify progress, cancellation, tree, map, and responsiveness.

### Tests for User Story 1

- [ ] T050 [P] [US1] Golden tests for `volumes_*`, `scan_*`, `tree_*`, `treemap_*` in `tests/contract/core_commands.rs`
- [ ] T051 [P] [US1] Vitest tests for states and keyboard on the "Disks", "Scan", "Tree", "Map" screens in `ui/tests/unit/screens/core/`
- [ ] T052 [P] [US1] Playwright E2E of the core on MockTransport in `ui/tests/e2e/core.spec.ts`

### Implementation for User Story 1

- [ ] T053 [US1] Implement the volumes service and commands in `crates/freespace-app/src/volumes.rs` and `apps/freespace-desktop/src-tauri/src/commands/volumes.rs`
- [ ] T054 [US1] Implement the scanning service and commands, progress, and cancellation in `crates/freespace-app/src/scan.rs` and `src-tauri/src/commands/scan.rs`
- [ ] T055 [US1] Implement the paginated tree, search, and ancestors in `crates/freespace-app/src/tree.rs` and `src-tauri/src/commands/tree.rs`
- [ ] T056 [US1] Implement Treemap aggregation and layout in `crates/freespace-app/src/treemap/` with tests at 10 million nodes
- [ ] T057 [US1] Implement the `treemap_tiles` command and the binary/compact response in `src-tauri/src/commands/treemap.rs`
- [ ] T058 [US1] Implement the "Disks" screen in `ui/src/screens/Dashboard.tsx` with volume cards and quick analysis
- [ ] T059 [US1] Implement the "Scan" screen in `ui/src/screens/Scan.tsx` with progress and cancellation
- [ ] T060 [US1] Implement the shared `DataTable`/`TreeTable` with virtualization in `ui/src/components/DataTable.tsx`
- [ ] T061 [US1] Implement the "Tree" screen in `ui/src/screens/Tree.tsx` with sorting, filters, and selection synchronization
- [ ] T062 [US1] Implement the Canvas Treemap and hit-testing in `ui/src/treemap/` with geometry and keyboard tests
- [ ] T063 [US1] Implement the "Map" screen in `ui/src/screens/Treemap.tsx` with "breadcrumbs" and a details panel
- [ ] T064 [US1] Implement the details panel and tree↔map synchronization in `ui/src/components/DetailsPanel.tsx`
- [ ] T065 [US1] Add visual baselines for the four core screens in `ui/tests/visual/core/`
- [ ] T066 [US1] Measure 200 ms / 30 fps / 4 GB at 10 million nodes and write the report to `docs/performance-002.md`

**Checkpoint**: US1 is fully functional and independently verifiable; this is the MVP.

---

## Phase 4: User Story 2 — Analysis and cleanup (P1)

**Goal**: largest files, types, duplicates, cleanup, and snapshots on the shared UI kit.

**Independent Test**: on the control set, verify totals, filters, duplicate rules, recommendations, and snapshot comparison without destructive operations.

### Tests for User Story 2

- [ ] T067 [P] [US2] Rust tests for `top_*`, `types_*`, `duplicates_*`, `cleanup_*`, `snapshots_*` in `tests/contract/analysis_commands.rs`
- [ ] T068 [P] [US2] Vitest tests for the five screens and the shared filters in `ui/tests/unit/screens/analysis/`
- [ ] T069 [P] [US2] Playwright E2E of the analysis on MockTransport in `ui/tests/e2e/analysis.spec.ts`

### Implementation for User Story 2

- [ ] T070 [US2] Implement the top files and types services and commands in `crates/freespace-app/src/top_types.rs` and `src-tauri/src/commands/top_types.rs`
- [ ] T071 [US2] Implement the duplicates service and commands with progress in `crates/freespace-app/src/duplicates.rs` and `src-tauri/src/commands/duplicates.rs`
- [ ] T072 [US2] Implement the cleanup and empty directory service and commands in `crates/freespace-app/src/cleanup.rs` and `src-tauri/src/commands/cleanup.rs`
- [ ] T073 [US2] Implement the snapshots and comparison service and commands in `crates/freespace-app/src/snapshots.rs` and `src-tauri/src/commands/snapshots.rs`
- [ ] T074 [US2] Implement the shared `FilterBar` and saved filters in `ui/src/components/FilterBar.tsx`
- [ ] T075 [US2] Implement the "Largest Files" screen in `ui/src/screens/TopFiles.tsx`
- [ ] T076 [US2] Implement the "Types" screen with a chart and legend in `ui/src/screens/Types.tsx`
- [ ] T077 [US2] Implement the "Duplicates" screen with groups and copy selection in `ui/src/screens/Duplicates.tsx`
- [ ] T078 [US2] Implement the "Cleanup" screen with risk cards in `ui/src/screens/Cleanup.tsx`
- [ ] T079 [US2] Implement the "Snapshots" screen and the diff table in `ui/src/screens/Snapshots.tsx`
- [ ] T080 [US2] Add visual baselines for the five analysis screens in `ui/tests/visual/analysis/`

**Checkpoint**: US1 and US2 work independently; deletion of copies is not yet confirmed without US3.

---

## Phase 5: User Story 3 — Safe operations (P1)

**Goal**: a single preview, confirmation, journal, quarantine, and undo for all destructive actions.

**Independent Test**: on a temporary directory, perform deletion to the Recycle Bin, quarantine, and permanent deletion; verify the journal, restoration, cancellation, and refusal without rights.

### Tests for User Story 3

- [ ] T081 [P] [US3] Rust tests for preview, tokens, confirmation, undo/restore, and concurrency in `tests/contract/operations_commands.rs`
- [ ] T082 [P] [US3] Vitest tests for `ConfirmDialog`, preview, and progress in `ui/tests/unit/operations/`
- [ ] T083 [P] [US3] E2E on a temporary directory: recycle/quarantine/permanent/undo in `ui/tests/e2e/operations.spec.ts`

### Implementation for User Story 3

- [ ] T084 [US3] Implement the operations service, preview tokens, and TTL in `crates/freespace-app/src/operations.rs` with tests
- [ ] T085 [US3] Implement the `operation_*` and `journal_*` commands in `src-tauri/src/commands/operations.rs`
- [ ] T086 [US3] Implement `OperationPreviewDialog` with warnings and deletion mode selection in `ui/src/components/OperationPreviewDialog.tsx`
- [ ] T087 [US3] Implement `OperationProgress` with cancellation and undo in `ui/src/components/OperationProgress.tsx`
- [ ] T088 [US3] Integrate the preview into "Top", "Duplicates", and "Cleanup" in `ui/src/screens/`
- [ ] T089 [US3] Implement the "Journal" screen with filters, undo, and restore in `ui/src/screens/Journal.tsx`
- [ ] T090 [US3] Implement elevation requirements and refusal without changing data in `ui/src/operations/elevation.ts` and `src-tauri/src/commands/app.rs`
- [ ] T091 [US3] Implement system path protection and the ban on leaving the scope in `crates/freespace-app/src/operations.rs` with tests
- [ ] T092 [US3] Implement locking of conflicting operations on a volume in `crates/freespace-app/src/operations.rs` with tests
- [ ] T093 [US3] Add visual baselines for preview, progress, and journal in `ui/tests/visual/operations/`
- [ ] T094 [US3] Conduct an audit: 100% of destructive actions go through preview/confirm/journal; report in `docs/operations-audit-002.md`

**Checkpoint**: all cleanups use the single safe layer.

---

## Phase 6: User Story 4 — File system and operations (P2)

**Goal**: journal, size details, long paths, rename, and permissions.

**Independent Test**: on a temporary structure with long paths and test ACLs, perform operations and verify the preview, journal, and errors.

### Tests for User Story 4

- [ ] T095 [P] [US4] Rust tests for `size_details`, `long_paths`, `rename`, `permissions`, `attributes` in `tests/contract/filesystem_commands.rs`
- [ ] T096 [P] [US4] Vitest tests for the five screens in `ui/tests/unit/screens/filesystem/`
- [ ] T097 [P] [US4] E2E of long paths, rename, and permissions on a temporary directory in `ui/tests/e2e/filesystem.spec.ts`

### Implementation for User Story 4

- [ ] T098 [US4] Implement the `size_details` service and command in `crates/freespace-app/src/size_details.rs` and `src-tauri/src/commands/filesystem.rs`
- [ ] T099 [US4] Implement long path paging and validation in `crates/freespace-app/src/long_paths.rs`
- [ ] T100 [US4] Implement rename preview and application in `crates/freespace-app/src/rename.rs`
- [ ] T101 [US4] Implement ACL/attribute preview and application in `crates/freespace-app/src/permissions.rs`
- [ ] T102 [US4] Implement the "Size Details" screen in `ui/src/screens/SizeDetails.tsx`
- [ ] T103 [US4] Implement the "Long Paths" screen in `ui/src/screens/LongPaths.tsx`
- [ ] T104 [US4] Implement the "Rename" screen with an old/new name diff in `ui/src/screens/Rename.tsx`
- [ ] T105 [US4] Implement the "Permissions" screen with ACL and attribute preview in `ui/src/screens/Permissions.tsx`
- [ ] T106 [US4] Extend the "Journal" with filters by type, path, and result in `ui/src/screens/Journal.tsx`
- [ ] T107 [US4] Add visual baselines for the five file system screens in `ui/tests/visual/filesystem/`

**Checkpoint**: US4 works independently; long paths and permissions do not bypass `freespace-ops`.

---

## Phase 7: User Story 5 — Inspection and reports (P2)

**Goal**: metadata, owners, archives, export, and unlock.

**Independent Test**: on a set with metadata, an archive, known owners, and a held file, verify reading, export, and the lock list without modifying files.

### Tests for User Story 5

- [ ] T108 [P] [US5] Rust tests for `metadata`, `owners`, `archives`, `export`, `unlock` in `tests/contract/inspection_commands.rs`
- [ ] T109 [P] [US5] Vitest tests for the five screens in `ui/tests/unit/screens/inspection/`
- [ ] T110 [P] [US5] E2E of inspection and export on MockTransport in `ui/tests/e2e/inspection.spec.ts`

### Implementation for User Story 5

- [ ] T111 [US5] Implement the metadata service and command in `crates/freespace-app/src/metadata.rs` and `src-tauri/src/commands/inspection.rs`
- [ ] T112 [US5] Implement owner pages and their files in `crates/freespace-app/src/owners.rs`
- [ ] T113 [US5] Implement archive pages and list export without extraction in `crates/freespace-app/src/archives.rs`
- [ ] T114 [US5] Implement export preview, start, and cancellation in `crates/freespace-app/src/export.rs`
- [ ] T115 [US5] Implement the lock list, soft/hard release, and reboot queue in `crates/freespace-app/src/unlock.rs`
- [ ] T116 [US5] Implement the "Metadata" screen in `ui/src/screens/Metadata.tsx`
- [ ] T117 [US5] Implement the "Owners" screen in `ui/src/screens/Owners.tsx`
- [ ] T118 [US5] Implement the "Archives" screen in `ui/src/screens/Archives.tsx`
- [ ] T119 [US5] Implement the "Export" screen in `ui/src/screens/Export.tsx`
- [ ] T120 [US5] Implement the "Unlock" screen in `ui/src/screens/Unlock.tsx`
- [ ] T121 [US5] Add visual baselines for the five inspection screens in `ui/tests/visual/inspection/`

**Checkpoint**: inspection does not modify data when viewing; the export matches the UI.

---

## Phase 8: User Story 6 — Operations and settings (P2)

**Goal**: shredder, write monitoring, elevation, settings, and "About".

**Independent Test**: on temporary files, verify settings, the shredder, monitoring, elevation, and version information.

### Tests for User Story 6

- [ ] T122 [P] [US6] Rust tests for `shred_*`, `monitor_*`, `settings_*`, `about_*`, `elevation_*` in `tests/contract/settings_commands.rs`
- [ ] T123 [P] [US6] Vitest tests for the five screens in `ui/tests/unit/screens/settings/`
- [ ] T124 [P] [US6] E2E of settings, the shredder (temporary file), and monitoring in `ui/tests/e2e/settings.spec.ts`

### Implementation for User Story 6

- [ ] T125 [US6] Implement shredder preview and start in `crates/freespace-app/src/shred.rs` and `src-tauri/src/commands/shred.rs`
- [ ] T126 [US6] Implement monitoring, events, and exclusions in `crates/freespace-app/src/monitor.rs` and `src-tauri/src/commands/monitor.rs`
- [ ] T127 [US6] Implement elevation status/request and the reason in `src-tauri/src/commands/app.rs`
- [ ] T128 [US6] Implement `settings_get/set/reset/validate_paths` in `crates/freespace-app/src/settings.rs` and `src-tauri/src/commands/settings.rs`
- [ ] T129 [US6] Implement `about_info` and `about_licenses` in `crates/freespace-app/src/about.rs`
- [ ] T130 [US6] Implement the "Shredder" screen in `ui/src/screens/Shred.tsx`
- [ ] T131 [US6] Implement the "Write Monitoring" screen in `ui/src/screens/Monitor.tsx`
- [ ] T132 [US6] Implement the "Elevation" screen in `ui/src/screens/Elevation.tsx`
- [ ] T133 [US6] Implement the "Settings" screen in `ui/src/screens/Settings.tsx`
- [ ] T134 [US6] Implement the "About" screen in `ui/src/screens/About.tsx`
- [ ] T135 [US6] Add visual baselines for the five settings screens in `ui/tests/visual/settings/`

**Checkpoint**: all 24 screens are implemented; polishing begins.

---

## Phase 9: User Story 7 — Themes, DPI, and motion (P2)

**Goal**: dark/light/system theme, DPI 100–200%, reduced motion without a flash.

**Independent Test**: switch themes on all screens, restart, change the system theme, verify DPI and reduced motion.

- [ ] T136 [P] [US7] Final dark/light theme tokens and Treemap palettes in `ui/src/styles/tokens.css` with a contrast test
- [ ] T137 [US7] Synchronize the system theme via `matchMedia` in `ui/src/app/theme.ts` with tests
- [ ] T138 [US7] Verify the absence of a flash on the 24 screens and record the test in `ui/tests/e2e/theme.spec.ts`
- [ ] T139 [US7] Verify 100/125/150/200% DPI on all screens; fix clipping and overlaps in `ui/src/styles/`
- [ ] T140 [US7] Implement reduced motion and `prefers-contrast` in `ui/src/styles/global.css` with tests
- [ ] T141 [US7] Add theme and DPI baselines for critical screens in `ui/tests/visual/themes/`
- [ ] T142 [US7] Verify multi-monitor DPI changes and `WindowState` persistence in `crates/freespace-app/src/window_state.rs`
- [ ] T143 [US7] Verify chart/map palettes for distinguishability and label contrast in `ui/src/treemap/palette.ts`

**Checkpoint**: both themes and DPI are ready for acceptance.

---

## Phase 10: User Story 8 — Accessibility (P2)

**Goal**: keyboard, focus, ARIA, live region, and Narrator on all screens.

**Independent Test**: walk through ten key scenarios without a mouse and with Narrator.

- [ ] T144 [P] [US8] Implement focus management and focus return in dialogs in `ui/src/app/focus.ts` with tests
- [ ] T145 [US8] Implement keyboard navigation for TreeGrid/DataTable/Treemap in `ui/src/components/` with tests
- [ ] T146 [US8] Implement ARIA roles, names, states, and `aria-live` in `ui/src/components/` and `ui/src/app/`
- [ ] T147 [US8] Add a skip-link, landmarks, and a logical Tab order in `ui/src/app/AppShell.tsx`
- [ ] T148 [US8] Implement throttling of progress and error announcements in `ui/src/app/live.ts`
- [ ] T149 [US8] Run axe on the 24 screens and fix critical violations in `ui/tests/a11y/screens.spec.ts`
- [ ] T150 [US8] Conduct a manual Narrator audit of ten scenarios and write the report to `docs/accessibility-002.md`
- [ ] T151 [US8] Verify that status is not conveyed by color alone on all screens and in the Treemap in `ui/src/screens/` and `ui/src/treemap/`
- [ ] T152 [US8] Add keyboard E2E scenarios in `ui/tests/e2e/keyboard.spec.ts`

**Checkpoint**: accessibility is confirmed automatically and manually.

---

## Phase 11: User Story 9 — RU/EN localization (P3)

**Goal**: 100% of strings from the catalog, switching without state loss, matching keys.

**Independent Test**: switch the language on all screens, verify missing keys, numbers/dates/units, and the absence of Cyrillic in EN.

- [ ] T153 [P] [US9] Extract and add all keys of the 24 screens to `locales/ru.toml` and `locales/en.toml`
- [ ] T154 [US9] Verify the `ui/src/i18n/generated/` generation and the absence of a diff in CI
- [ ] T155 [US9] Implement pluralization, numbers, dates, and units via `Intl` in `ui/src/i18n/format.ts` with tests
- [ ] T156 [US9] Implement language switching without state loss in `ui/src/stores/settings.ts` with tests
- [ ] T157 [US9] Add a test for the absence of missing keys and Cyrillic in EN in `ui/tests/unit/i18n.test.ts`
- [ ] T158 [US9] Add RU/EN visual baselines for critical screens in `ui/tests/visual/locales/`
- [ ] T159 [US9] Verify the localization of errors, operations, and notifications on all screens in `ui/src/i18n/` and `locales/`

**Checkpoint**: localization is complete; switching does not lose state.

---

## Phase 12: User Story 10 — Visual regression and tests (P3)

**Goal**: full screenshot matrix, token/string/network checks, stable CI.

**Independent Test**: change a token/string and make sure the check fails; revert it and make sure it passes.

- [ ] T160 [US10] Generate the full 24×2×4×2 baseline matrix in `ui/tests/visual/baselines/`
- [ ] T161 [US10] Configure the PR set (8 critical screens + all 24 in the dark theme at 100% RU) in `playwright.config.ts`
- [ ] T162 [US10] Implement `check:tokens` and `check:i18n` as blocking CI checks in `ui/scripts/` and `.github/workflows/ci.yml`
- [ ] T163 [US10] Implement `check:network` and the CSP check in E2E in `ui/scripts/check-network.mjs` and `tests/tauri_e2e/`
- [ ] T164 [US10] Configure Tauri E2E (`tauri-driver` + WebdriverIO) in `.github/workflows/ci.yml` and `tests/tauri_e2e/`
- [ ] T165 [US10] Configure visual test stability: fonts, DPI, tolerances, retries in `ui/tests/visual/` and `playwright.config.ts`
- [ ] T166 [US10] Create a screenshot gallery for review in `ui/tests/visual/gallery/` and a link from `docs/`
- [ ] T167 [US10] Add a test that a new screen without a baseline/test does not pass acceptance in `ui/tests/visual/coverage.test.ts`
- [ ] T168 [US10] Run the full test suite on Windows CI and eliminate flaky tests in `.github/workflows/ci.yml` and `ui/tests/`

**Checkpoint**: visual and functional regression is protected.

---

## Phase 13: User Story 11 — Single distribution and replacement (P3)

**Goal**: one GUI, offline installer, portable, removal of `data` and menu entries, CLI parity.

**Independent Test**: build the installer/portable on clean Windows without network, install, launch, uninstall, and verify the absence of traces outside `data`.

- [ ] T169 [US11] Ensure `apps/freespace-gui` and the egui dependencies are absent from the workspace and the distribution; verify `Cargo.toml` and `Cargo.lock`
- [ ] T170 [US11] Configure the Tauri NSIS installer in `apps/freespace-desktop/src-tauri/tauri.conf.json` and the hooks in `installer/nsis/`
- [ ] T171 [US11] Implement offline WebView2 (fixed runtime/offlineInstaller) and version checking in `src-tauri/`
- [ ] T172 [US11] Build the portable ZIP with `data` and WebView2 in `installer/portable.ps1`
- [ ] T173 [US11] Move removal of `data` and `context-menu uninstall` into NSIS hooks in `installer/nsis/`; remove/withdraw `installer/freespace.iss` from the distribution
- [ ] T174 [US11] Preserve the `freespace-gui.exe` name, shortcuts, icon, and splash in `installer/nsis/` and `apps/freespace-desktop/src-tauri/tauri.conf.json`; verify installation/update
- [ ] T175 [US11] Verify that settings, journal, quarantine, cache, and the WebView2 profile are located only in `data`, in `apps/freespace-desktop/src-tauri/src/main.rs` and `tests/tauri_e2e/storage.spec.ts`
- [ ] T176 [US11] Verify installation/launch/uninstall on clean Windows without network and record the report in `docs/release-validation-002.md`
- [ ] T177 [US11] Run the CLI golden tests and fill in the 24-screen parity matrix in `specs/002-native-ui-redesign/contracts/parity.md`
- [ ] T178 [US11] Verify migration of existing settings/journal/quarantine/snapshots and rollback from a backup in `crates/freespace-app/src/settings.rs` and `tests/contract/settings_commands.rs`
- [ ] T179 [US11] Update `README.md`, `docs/user-guide.md`, `docs/cli.md`, and the `ru/`/`en/` mirrors for the new UI
- [ ] T180 [US11] Update `CHANGELOG.md` and the version per Conventional Commits
- [ ] T181 [US11] Conduct final acceptance of 24/24, 0 network, AA, DPI, RU/EN, one UI, CLI parity in `specs/002-native-ui-redesign/contracts/parity.md` and `docs/release-validation-002.md`

**Checkpoint**: the release is ready; the old UI is removed and there is no second UI.

---

## Phase 14: Polish & Cross-Cutting Concerns

**Purpose**: performance, code quality, documentation, and final checks.

- [ ] T182 [P] Optimize startup: boot, first paint, first data screen to 3/5 s in `ui/src/app/` and `src-tauri/src/boot.rs`
- [ ] T183 [P] Optimize memory: release pages/tiles when leaving a screen in `ui/src/stores/` and `src-tauri/src/state.rs`
- [ ] T184 [P] Review and improve error messages and empty states on all 24 screens in `ui/src/screens/` and `ui/src/components/`
- [ ] T185 [P] Conduct a dependency audit: versions, licenses, absence of extra packages, justification in `research.md`
- [ ] T186 Remove dead code, temporary flags, and unused DTOs in `crates/freespace-app/` and `ui/src/`
- [ ] T187 Update `docs/ui-redesign-plan.md` with the status "implemented" and links to 002
- [ ] T188 Run `quickstart.md` end to end on a clean machine and record the results
- [ ] T189 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` in `Cargo.toml` and `apps/freespace-desktop/src-tauri/Cargo.toml`
- [ ] T190 Run `npm ci`, `npm run typecheck`, `npm run lint`, `npm run test:unit`, `npm run test:e2e`, `npm run test:visual`, `npm run test:a11y` in `apps/freespace-desktop/ui/package.json`
- [ ] T191 Verify that `en/` contains no Cyrillic, IDs (FR-, SC-, T-, CHK-) are unchanged, and the 002 mirrors are updated in `en/002-native-ui-redesign/`
- [ ] T192 Prepare the final report: performance, accessibility, offline, visual regression, parity in `docs/release-validation-002.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: no dependencies; removal of egui happens here.
- **Phase 2 (Foundational)**: depends on Phase 1; blocks all stories.
- **Phase 3 (US1)**: after Phase 2; MVP.
- **Phase 4 (US2)**: after Phase 2; uses the shared UI kit and filters. Destructive actions
  require Phase 5.
- **Phase 5 (US3)**: after US1; blocks destructive actions in US2 and US4–US6.
- **Phase 6 (US4)**: after US3; uses the journal and preview.
- **Phase 7 (US5)**: after Phase 2; can run in parallel with US4.
- **Phase 8 (US6)**: after Phase 2; can run in parallel with US4/US5.
- **Phase 9 (US7)**: after US1 and the UI kit; polish of all screens.
- **Phase 10 (US8)**: after US1; preferably in parallel with the screens, but the final audit is after US6.
- **Phase 11 (US9)**: after the screens exist; the final check is after US6.
- **Phase 12 (US10)**: after US1–US9; final matrix.
- **Phase 13 (US11)**: after 24 screens and US10; final replacement/distribution.
- **Phase 14 (Polish)**: after all stories.

### User Story Dependencies

- **US1 (P1)**: base; does not depend on other stories.
- **US2 (P1)**: depends on US1 (data) and US3 (confirmation of destructive actions).
- **US3 (P1)**: depends on US1; required for US2, US4, US5 (unlock), US6 (shred).
- **US4 (P2)**: depends on US1 and US3.
- **US5 (P2)**: depends on US1; unlock uses US3.
- **US6 (P2)**: depends on US1; shred uses US3.
- **US7 (P2)**: depends on US1 and the UI kit.
- **US8 (P2)**: depends on US1; final audit after US6.
- **US9 (P3)**: depends on the US1–US6 screens.
- **US10 (P3)**: depends on US1–US9.
- **US11 (P3)**: depends on 24 screens, US10, and CLI parity.

### Within Each User Story

- Tests are written and fail before implementation (constitution, principle V).
- DTOs/services — before commands; commands — before screens.
- Shared components — before specialized screens.
- Visual baselines — after the screen stabilizes.
- E2E — after the real/mock transport exists.

### Parallel Opportunities

- Phase 1: T003, T005, T006, T007, T014, T019, T020.
- Phase 2: T022–T024, T026–T027, T029, T034–T035, T045–T048.
- US1: T050–T052 (tests), then T053–T064 by file; T065 after the screens.
- US2: T067–T069 (tests), then T070–T079; T080 after the screens.
- US3: T081–T083 (tests), then T084–T092; T093–T094 after.
- US4/US5/US6: tests and backend services can proceed in parallel across different files.
- US7/US8/US9: audits and fixes can proceed in parallel after the screens exist.
- US10: baseline generation and CI setup — in parallel, but the final run is sequential.
- US11: installer/portable/offline checks — in parallel, final acceptance is sequential.

---

## Parallel Example: User Story 1

```text
# Tests (different files):
T050 Golden tests for core commands
T051 Vitest tests for core screens
T052 Playwright E2E for the core

# Implementation (different files):
T053 volumes service/commands
T054 scan service/commands
T055 tree service/commands
T056 treemap layout
T057 treemap command
T058 Dashboard
T059 Scan
T060 DataTable
T061 Tree
T062 Canvas Treemap
T063 Treemap screen
T064 DetailsPanel
```

## Parallel Example: User Story 3

```text
T081 Rust tests for operations
T082 Vitest tests for dialogs
T083 E2E of the temporary directory
T084 operations service
T085 operation/journal commands
T086 OperationPreviewDialog
T087 OperationProgress
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: ADR, removal of egui, Tauri/React skeleton, CI.
2. Phase 2: application layer, IPC, security, settings, UI kit, test base.
3. Phase 3: US1 — disks, scanning, tree, map.
4. **STOP and VALIDATE**: quickstart, scenario 1, 200 ms/30 fps/4 GB.
5. MVP demonstration; the release is not performed yet, the old UI is removed.

### Incremental Delivery

1. US1 → MVP.
2. US3 → safe operations (mandatory before cleanup).
3. US2 → analysis and cleanup.
4. US4 → file system and operations.
5. US5 → inspection and reports.
6. US6 → operations and settings.
7. US7–US9 → themes, accessibility, localization.
8. US10 → full test matrix.
9. US11 → installer, offline, removal of egui, acceptance.
10. Polish → final checks.

### Parallel Team Strategy

- Rust engineer: `freespace-app`, commands, tests, performance.
- Frontend engineer: UI kit, shell, screens, accessibility.
- Designer/QA: tokens, screenshots, axe/Narrator, quickstart.
- After Phase 2 teams can work in parallel on stories, but US3 has priority.

---

## Notes

- [P] — different files, no dependencies.
- [Story] — traceability to spec.md.
- The old egui is removed in Phase 1; its sources remain available in git history but are not built.
- All destructive scenarios — only on temporary directories/VHDs; admin tests behind
  `FREESPACE_ADMIN_TESTS=1`.
- 0 network calls, 0 hardcoded colors, 0 strings outside localization — blocking checks.
- Commits — Conventional Commits; one task or logical group per commit.
- Mark `[x]` only after the task's definition of done is met and checks are green.
