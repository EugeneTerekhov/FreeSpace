# Implementation Plan: FreeSpace — a unified next-generation native interface

**Branch**: `002-native-ui-redesign` | **Date**: 2026-11-16 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-native-ui-redesign/spec.md`

**Replacement, not parallel support**: the old egui interface was never released. As part of 002
it is fully replaced by the new client: `apps/freespace-gui` and the egui dependencies are deleted,
a second UI is not built or shipped. The beta flag, side-by-side and a separate cutover phase are
excluded. This decision is recorded in [spec.md](./spec.md#clarifications) and in ADR
[0001-tauri-react-gui.md](../adr/0001-tauri-react-gui.md).

## Summary

FreeSpace moves the graphical interface to Tauri 2 + React 18 + TypeScript + Vite. Rust
remains the sole owner of domain logic, WinAPI, scanning, operations, the journal and
quarantine; React is responsible only for presentation and input. The new client contains 24 screens,
a single product UI kit, dark/light/system theme, virtualized tables and a Canvas
Treemap with chunked data delivery, safe operations through the existing `freespace-ops`,
keyboard and Narrator accessibility, 100–200% DPI, RU/EN localization and offline delivery.
The old egui client is deleted within this same feature; the CLI contract and the core do not change.

## Technical Context

**Language/Version**: Rust 1.85+ (edition 2024), TypeScript 5.6+, Node.js 20.19+, Windows x64

**Primary Dependencies**:

- UI: Tauri 2.x (`tauri`, `tauri-build`), React 18.3, React DOM 18.3, TypeScript 5.6,
  Vite 6.x, `@vitejs/plugin-react` 4.x, `@tauri-apps/api` 2.x, the `@types/react` and
  `@types/react-dom` types.
- Components and icons: `lucide-react` 0.468 (local package, no CDN).
- Lint: `eslint` 9.x, `typescript-eslint` 8.x, `eslint-plugin-react-hooks`,
  `eslint-plugin-react-refresh`.
- Tables and virtualization: `@tanstack/react-table` 8.x and `@tanstack/react-virtual` 3.x —
  column model, visibility/order and row virtualization; sorting and filtering remain
  server-side.
- View state: `zustand` 5.x — small typed stores per screen and a shared
  shell store; no Redux and no global data cache.
- Localization: the existing `locales/ru.toml` and `locales/en.toml` as the single source;
  the `ui/scripts/gen-i18n.mjs` generator with `smol-toml` produces typed TS catalogs.
- Tests: `vitest`, `@testing-library/react`, `@testing-library/user-event`, `jsdom`,
  `@playwright/test`, `@axe-core/playwright`, `tauri-driver` and `webdriverio` on Windows.
- Rust: `tauri`, `tauri-build`, `serde`, `serde_json`, `thiserror`, `tracing`,
  `tracing-subscriber`, the existing `freespace-core`, `freespace-platform`,
  `freespace-scan`, `freespace-ops`, `freespace-analysis`, `freespace-report`,
  `freespace-cli`; asynchronous tasks use the Tauri async runtime, without a direct `tokio`.
- New application layer: `crates/freespace-app` — commands, services, DTOs, pagination,
  Treemap aggregation and settings migration; it contains no WinAPI and does not replace the core.

The justification for every new dependency and the alternatives are in [research.md](./research.md).

**Storage**: a single `data` directory next to the executable. The existing SQLite
(journal, quarantine, hash cache), TOML settings and `.fssnap` snapshots are preserved. The new
UI settings are stored in `data/settings.toml` (the `[ui]` section), window state — in
`data/ui-state.toml`; `localStorage`, `IndexedDB` and a WebView2 profile outside `data` are forbidden.
The WebView2 profile is forced into `data/webview2`. `%APPDATA%`,
`%LOCALAPPDATA%`, `%ProgramData%` and temporary directories are not used.

**Testing**: `cargo test --workspace` (core, CLI, operations, new commands and validation),
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`vitest` + React Testing Library for components and screens, Playwright for the Vite build,
`tauri-driver`/WebDriver for Tauri E2E, `@axe-core/playwright` for accessibility, Playwright visual
regression with fixed fonts/DPI, a network guard, administrative tests
only when `FREESPACE_ADMIN_TESTS=1`.

**Target Platform**: Windows 10 21H2+ and Windows 11, x64; WebView2 (system or shipped
offline); NTFS as the primary FS, the non-NTFS fallback is preserved.

**Project Type**: desktop-app + CLI (Cargo workspace + Tauri frontend)

**Performance Goals**:

- Interface response to actions — no more than 200 ms during scanning and navigation.
- Treemap — no lower than 30 frames/s on the control set of 10 million nodes.
- Memory of the whole application, including the native layer and WebView2 — no more than 4 GB for 10 million nodes.
- First interactive screen — no more than 3 s; first screen with data — no more than 5 s after
  the data is ready.
- Full analysis of an 8 TB NTFS volume — no more than 60 s (the existing core budget does not change).
- Progress and events — throttle 10–20 Hz; a full JSON tree for 10 million nodes is forbidden.

**Constraints**:

- Fully offline operation, no telemetry, network requests or remote assets.
- CSP `default-src 'self'`; minimal Tauri capabilities; no fs/shell/http plugins;
  devtools disabled in release.
- All WinAPI calls — only in `crates/freespace-platform`; Tauri commands and React do not call
  WinAPI directly.
- Destructive operations — only through `freespace-ops`: preview, confirmation, deletion
  mode selection, journal, quarantine/Recycle Bin, undo and system-path protection.
- Settings, journal, quarantine, cache and WebView2 data — only in `data` next to the exe.
- One graphical interface: egui is removed, a second UI is not supported.
- The CLI contract, exit codes, report and snapshot formats do not change.
- 24 screens, RU/EN, dark/light/system theme, AA, DPI 100/125/150/200%, reduced motion.
- The executable name `freespace-gui.exe` is preserved; the exe name, shortcuts and context menu
  do not break.

**Scale/Scope**: 24 screens, 5 screen batches, 11 user stories, 130 FR, 22 SC;
up to 10 million nodes per volume, volumes up to 8 TB, 2 languages, 3 theme modes, 4 DPI scales; one GUI client
and the existing CLI.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Compliance | Justification |
|-----------|------------|---------------|
| I. Safety of destructive operations | PASS | The new UI only calls commands; preview, confirmation, deletion mode selection, journal, quarantine, undo and system-path protection remain in `freespace-ops`. The UI gets no direct access to the FS. |
| II. Accuracy of space accounting | PASS | Rust computes all sizes; the UI shows logical/on-disk/physical size with an explicit label, does not recompute sums and does not round them for totals. |
| III. Performance on large volumes | PASS | MFT/USN and the core do not change; the tree and tables — paginated and virtualized, the Treemap — aggregated tiles and Canvas LOD, progress throttled at 10–20 Hz, the 200 ms/30 fps/4 GB budgets are verified separately. |
| IV. Native Windows accuracy | PASS | WebView2 is a Windows component; WinAPI remains only in `freespace-platform`; Tauri commands do not call WinAPI. Long paths, NTFS, permissions, elevation and the context menu are preserved. |
| V. Test-first approach | PASS | Rust tests for commands/validation/serialization, Vitest/RTL, Playwright, tauri-driver E2E, axe and visual regression; fixtures — only temporary directories and test VHDs; admin tests behind `FREESPACE_ADMIN_TESTS=1`. |
| VI. CLI and GUI parity | PASS | One core; the CLI contract is protected by reference JSON checks; the parity matrix "screen — capability — CLI command — test — screenshot" is filled in before release. |
| VII. Privacy and offline | PASS | No remote assets, telemetry or network plugins; CSP, minimal capabilities, offline WebView2, all data in `data`. |
| Tech constraints: UI — egui/eframe, Tauri only with justification | PASS (with ADR) | Tauri is allowed by the constitution given a justified need for web technologies and preservation of native functionality. The justification is ADR `docs/adr/0001-tauri-react-gui.md`; egui is fully replaced, a second UI is not supported. |

There are no violations requiring a stop. The deviation regarding the UI framework is justified by the ADR and does not reduce
native functionality. The complexity of the replacement is recorded in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/002-native-ui-redesign/
├── plan.md              # This file
├── research.md          # Phase 0: decisions, alternatives and budgets
├── data-model.md        # Phase 1: view and state entities
├── quickstart.md        # Phase 1: validation scenarios
├── contracts/
│   ├── ui.md            # Tokens, components, 24 screens, accessibility
│   └── ipc.md           # Commands, events, pagination, Treemap, errors
├── checklists/
│   └── requirements.md  # Requirements quality checklist
└── tasks.md             # Phase 2: tasks (/speckit.tasks)
```

### Source Code (repository root)

```text
Cargo.toml                        # workspace: remove apps/freespace-gui, add the new GUI and freespace-app
rust-toolchain.toml
crates/
├── freespace-core/               # domain model, tree, metrics, settings, i18n (no semantic changes)
├── freespace-platform/           # WinAPI: volumes, MFT/USN, locks, ACL, TRIM, registry, ETW
├── freespace-scan/               # MFT and fallback traversal, incremental sessions
├── freespace-ops/                # operations, preview, journal, quarantine, shredder
├── freespace-analysis/           # duplicates, cleanup, metadata, owners, archives
├── freespace-report/             # reports, snapshots and comparison
├── freespace-app/                # NEW: application services, DTOs, pagination, Treemap aggregation
└── freespace-cli/                # CLI with an unchanged contract
apps/
└── freespace-desktop/            # NEW single GUI; replaces apps/freespace-gui
    ├── ui/                       # React 18 + TS + Vite
    │   ├── src/
    │   │   ├── app/              # shell, routing state, providers, boot
    │   │   ├── screens/          # 24 screens
    │   │   ├── components/       # product UI kit
    │   │   ├── styles/           # tokens.css, themes.css, global.css
    │   │   ├── api/              # typed IPC client
    │   │   ├── stores/           # Zustand stores
    │   │   ├── i18n/             # generated catalogs and keys
    │   │   └── treemap/          # Canvas rendering, hit-testing, palettes
    │   ├── tests/                # Vitest/RTL, Playwright, screenshots
    │   ├── scripts/gen-i18n.mjs  # TOML → typed TS catalogs
    │   ├── package.json
    │   └── vite.config.ts
    ├── src-tauri/                # Tauri 2 Rust binary
    │   ├── src/
    │   │   ├── commands/         # scan, tree, treemap, dupes, cleanup, snapshots, ops, settings…
    │   │   ├── state.rs          # AppState, tasks, channels, cancellation, progress
    │   │   ├── events.rs         # scan-progress, operation-progress, monitor-tick
    │   │   ├── security.rs       # validation of inputs, paths, capabilities
    │   │   ├── boot.rs           # theme/language/window before the first paint
    │   │   ├── lib.rs
    │   │   └── main.rs
    │   ├── capabilities/default.json
    │   ├── icons/
    │   ├── Cargo.toml
    │   └── tauri.conf.json
    └── build.rs
tests/                            # cross-crate tests; tauri_e2e/ for WebDriver
benches/                          # scanning and Treemap benchmarks
docs/
└── adr/0001-tauri-react-gui.md   # mandatory ADR
locales/                          # ru.toml/en.toml — the single source of strings
installer/                        # replace/adapt for Tauri NSIS, keep data removal and the menu
ru/                               # documentation mirrors (including 002)
en/                               # documentation translations (including 002)
```

**Structure Decision**: a single GUI crate `apps/freespace-desktop` with the binary
`freespace-gui.exe`. `crates/freespace-app` is a thin application layer between the Tauri commands and
the existing core, so as not to duplicate the logic of the 24 egui views or turn the Tauri commands into
business logic. React has no access to the FS or WinAPI: only typed commands/events.
`apps/freespace-gui` is deleted; its sources remain in git history as the parity source.
A second UI, a beta flag and side-by-side are not created.

## Delivery Phases

| Phase | Content | Outcome |
|-------|---------|---------|
| 0. ADR and skeleton | ADR, Constitution Check, Tauri skeleton, themes, IPC hello, WebView2/packaging spike | Decision recorded, risk removed |
| 1. Foundation | Tokens, both themes, shell, UI kit, i18n, API client, stores, error boundary, test infrastructure | Component gallery and shell |
| 2. Data | volumes/scan/tree/treemap/ops, pagination, cancellation, progress, security caps, settings migration | Working data bridge |
| 3. Core | Disks, Scan, Tree, Map | MVP |
| 4. Operation safety | Preview, confirmation, journal, quarantine, undo | A single layer for all cleanups |
| 5. Analysis/cleanup | Top, Types, Duplicates, Cleanup, Snapshots | Batch 2 |
| 6. FS/operations | Journal, Details, Long paths, Rename, Permissions | Batch 3 |
| 7. Inspection/reports | Metadata, Owners, Archives, Export, Unlock | Batch 4 |
| 8. Operations/settings | Shredder, Monitoring, Elevation, Settings, About | Batch 5 |
| 9. Polish | Accessibility, DPI, performance, screenshots, offline packaging, egui removal, documentation | Release |

## Complexity Tracking

| Violation / Complexity | Why Needed | Simpler Alternative Rejected Because |
|-----------------------|------------|--------------------------------------|
| Tauri 2 + WebView2 instead of egui | An exact transfer of the new interface language and React components, CSS themes, DOM accessibility and Playwright is required; the constitution allows Tauri given a justified need. | egui does not provide the required level of visual control, CSS themes and DOM accessibility; redrawing 24 screens anew in egui would mean duplicating the design system and losing the declared mockup language. |
| The new crate `freespace-app` | Application services, DTOs, pagination, Treemap aggregation and settings migration are shared by commands; otherwise the logic of 24 screens would sprawl across Tauri commands or be copied from egui. | Commands directly over the core are faster at the start, but lead to duplication and are harder to test; moving all logic into `freespace-core` blurs the domain. |
| Node.js/Vite/TypeScript in a Rust repository | A React build, IPC typing and frontend tests are required. | Dropping Node means dropping the chosen stack; an alternative UI framework does not provide the required design and accessibility. |
| IPC contract and chunked delivery | 10 million nodes cannot be transferred as full JSON; pages, aggregates and events are needed. | A full JSON/tree dump does not fit the memory and time budget; direct React access to the FS is forbidden by the constitution. |
| Offline WebView2 and installer size | Installation without a network is mandatory; system WebView2 is not guaranteed on Windows 10. | Requiring system WebView2 is a risk of installation failure; a download bootstrapper violates the offline requirement. |
| Full replacement of 24 screens and removal of egui | The product has not been released yet; maintaining two UIs is unprofitable and dangerous. | Parallel support doubles tests, accessibility and packaging; a beta flag leaves two code paths. |
| Two themes, DPI and localization across 24 screens | This is a product and acceptance requirement. | A single theme/one DPI/one language would reduce the scope, but would not match the specification. |
| Settings and WebView2 data in `data` | The constitution and FR-093 forbid `%APPDATA%`/`%LOCALAPPDATA%`; the default WebView2 profile writes to the user profile. | Keeping the default profile is a storage violation; localStorage/IndexedDB are hidden data outside `data`. |

## Post-Design Constitution Re-Check

After Phase 1 (data-model, contracts, quickstart) the following are checked again:

- all commands go through `freespace-ops` and do not give the UI direct access to the FS;
- sizes and metric types are not recomputed in TypeScript;
- pagination, the Treemap and events fit the 200 ms / 30 fps / 4 GB budgets;
- CSP and capabilities contain no network or broad file permissions;
- the webview profile and settings are placed in `data`;
- 24 screens are covered by the parity matrix and tests;
- egui is removed from the workspace, no second UI exists.

## Risks and Mitigations

| Risk | Measure |
|------|---------|
| WebView2 missing/old on Windows 10 | Offline fixed runtime in the distribution; version check; portable variant; packaging spike. |
| 10 million nodes and IPC | Pagination, aggregated tiles, Canvas, virtualization, throttle; spike before batches 3+. |
| Memory > 4 GB (Rust + WebView) | WebView budget, releasing data when leaving a screen, measuring the whole process. |
| Duplication of egui view logic | `freespace-app`, DTOs and tests; egui code is deleted rather than maintained. |
| Tauri security | CSP `default-src 'self'`, minimal capabilities, input validation, no remote, devtools off, code review. |
| Loss of user data during migration | Backup of `settings.toml`, migration version, reversibility, tests on data copies. |
| Screenshot tests are unstable | Fixed fonts/DPI, tolerances, Windows CI, stable fixtures. |
| Installer size due to WebView2 | Measure fixed vs offline installer; record the budget and decision in research. |
| Accessibility | axe/Playwright, Narrator, keyboard scenarios, early audit. |
| Timeline | Screen batches, MVP (US1) first, a buffer; batches cannot be cut without updating the specification. |
