# Research: FreeSpace — technical decisions for interface 002

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

All open decisions from `plan.md` are resolved below in the format "decision —
rationale — alternatives". Dependency versions are pinned in `Cargo.lock` and
`package-lock.json`; the text lists compatible ranges.

## 1. GUI stack and complete replacement of egui

- **Decision**: Tauri 2.x + React 18.3 + TypeScript 5.6 + Vite 6.x. The new client
  `apps/freespace-desktop` completely replaces `apps/freespace-gui`; egui dependencies and the
  old views are removed as part of 002. No second UI, beta flag, or side-by-side is created.
- **Rationale**: the stack is already approved in `plan.md` and the `web/`
  mockup; it enables an exact port of the interface language, CSS themes, DOM accessibility,
  Playwright, and a large component ecosystem. The constitution permits Tauri when web
  technologies are justified, and all native functionality remains in Rust. The old UI was
  never released, so maintaining it in parallel has no value and only doubles the cost of tests
  and packaging.
- **Alternatives considered**: stay on egui (no needed visual control, CSS themes, or DOM
  accessibility); Slint (smaller ecosystem and worse mockup porting); WinUI 3 (heavy
  integration with Rust and C++/C#); React 19 (does not match the approved mockup and adds
  migration risk without benefit for the task).

## 2. Repository structure and removal of the old GUI

- **Decision**: the only GUI crate is `apps/freespace-desktop` with the binary
  `freespace-gui.exe`; the `apps/freespace-gui` directory is deleted, and its sources remain in
  git history as the parity reference. The workspace `Cargo.toml` and `default-members` are
  updated at the same time so that the build never contains two GUIs.
- **Rationale**: a complete replacement without an intermediate dual shipment. The binary name
  preserves shortcuts, scripts, the installer, and context menu entries. Git history is
  sufficient to verify the 11 screens and their tabs during the port.
- **Alternatives considered**: keep `apps/freespace-gui` as a reference crate (violates the
  decision on complete replacement and creates dead code); rename the directory without
  changing the binary (breaks external scenarios); run the new client alongside (rejected by
  the product owner).

## 3. Application layer `freespace-app`

- **Decision**: the new crate `crates/freespace-app` contains application services, DTOs,
  pagination, tree node share aggregation, UI settings migration, and task orchestration. Tauri
  commands are thin adapters: input validation, service call, serialization, events. There is no
  WinAPI in `freespace-app`.
- **Rationale**: 11 screens must not duplicate egui view logic; commands must not turn into
  business logic. A separate crate is tested without Tauri and reused by the CLI/future
  frontends. The domain remains in `freespace-core`, operations in `freespace-ops`.
- **Alternatives considered**: logic directly in Tauri commands (duplication, complex tests);
  everything in `freespace-core` (blurs the domain and drags UI DTOs into the core); everything
  in TypeScript (violates logic ownership and security).

## 4. IPC: commands, events, and transport

- **Decision**: control calls are typed Tauri commands with JSON schemas; large sets (tree
  pages) use compact DTOs, and when necessary a binary `tauri::ipc::Response`;
  progress and changes are Tauri events with 10–20 Hz throttling. Every long-running task has
  `task_id`, state, progress, and `cancel(task_id)`.
- **Rationale**: JSON is convenient for typing and tests; a binary response reduces overhead on
  large pages. Events must not become a second data channel: they carry progress/invalidation,
  while data is requested page by page.
- **Alternatives considered**: a single WebSocket/HTTP server inside the application (extra
  attack surface and network); full JSON of the tree (does not meet the budget); polling
  without events (extra load and delays).

## 5. Pagination and stable cursors

- **Decision**: cursor pagination with an opaque `cursor` encoding the sort key and `node_id`;
  default page size 500, maximum 2000. The response contains `rows`, `next_cursor`,
  `total_estimate` (if known), and `generation`/`snapshot_id`. Changing sort/filter resets the
  cursor; a stale cursor returns `stale_cursor`, and the UI re-requests the first page.
- **Rationale**: offset pagination on 10 million nodes degrades and produces duplicates on
  changes. A cursor is stable, does not require a full count, and works well with
  virtualization. `generation` protects against mixing data from different scans.
- **Alternatives considered**: offset/limit (slow and unstable); return the whole tree
  (prohibited by the budget); keyset by name only (unstable with duplicates).

## 6. Tree: virtualization and inline progress bars

- **Decision**: data exploration happens only through the virtualized tree; the Treemap is
  excluded from the product. Each node row shows a semi-transparent fill proportional to the
  node's share of its parent (and of the volume); the shares are computed by Rust and included in
  `NodeRow`. The node icon depends on the object type (directory, text file, image, video,
  archive, volume, file). Size details are shown in the hover card of the row.
- **Rationale**: the tree reads better than a heatmap on dense data; inline progress bars provide
  legibility without a separate screen; shares in Rust keep the UI from summing rows.
  Virtualization (`@tanstack/react-virtual`) holds 30 fps at 10 million nodes without a DOM blowup.
- **Alternatives considered**: Canvas Treemap (hard to read, duplicates navigation, needs
  hit-testing and a legend); SVG tree (heavy on large sets); layout in TypeScript (duplicating the
  algorithm).

## 7. Tables and virtualization

- **Decision**: `@tanstack/react-table` for the column model, visibility, order, and width;
  `@tanstack/react-virtual` for row virtualization and, if needed, columns. Sorting, filtering,
  and search are server-side; the table requests pages and does not keep the whole set in
  memory.
- **Rationale**: mature, small libraries without a heavy UI framework; they work well with
  cursor pagination; they allow a single `DataTable` for 11 screens. Server-side sorting keeps
  totals consistent.
- **Alternatives considered**: a custom virtualizer (more code and edge cases); AG Grid (heavy
  license and styles); MUI DataGrid (heavy UI framework); Ant Design (excessive design
  language).

## 8. View state

- **Decision**: Zustand 5.x. Separate stores: `shellStore` (screen, theme, language,
  notifications), `scanStore` (task, progress), `selectionStore` (selection, filters), and a
  factory of per-screen stores. Page data is kept in the screen store with `generation` and
  `cursor`; events update only the affected slices.
- **Rationale**: minimal boilerplate, targeted subscriptions, and no providers; well suited to
  an event-driven desktop application. There is no global data cache — the source of truth is
  Rust, and pages are short-lived.
- **Alternatives considered**: React context/useReducer (frequent re-renders and prop drilling
  across 11 screens); Redux Toolkit (heavier for the task); TanStack Query (useful but adds a
  second cache layer and conflicts with the event model; can be added later if necessary).

## 9. Styles and design tokens

- **Decision**: CSS custom properties in `styles/tokens.css` + CSS Modules for components. The
  theme is set by `:root[data-theme="dark"|"light"]`; the system theme via
  `prefers-color-scheme` and the saved setting. The ban on hard-coded colors is enforced by a
  lint/script.
- **Rationale**: zero runtime, fast theme switching, full control over CSS, and no conflicts
  with CSP. CSS Modules provide local names and do not require CSS-in-JS.
- **Alternatives considered**: Tailwind (harder to keep tokens and the hard-coding ban);
  styled-components/emotion (runtime and CSP risks); vanilla-extract (additional build);
  global CSS without modules (collisions across 11 screens).

## 10. Fonts and icons

- **Decision**: system fonts `Segoe UI Variable`, `Segoe UI`, `Cascadia Mono` for monospaced
  text; `lucide-react` 0.468 with named imports. No CDNs and no downloaded fonts.
- **Rationale**: matches Windows, requires no network or licensing investment, and scales well
  at 100–200% DPI. Lucide is already used in the mockup.
- **Alternatives considered**: embedded web fonts (size and licenses); an icon font (worse
  accessibility); an SVG sprite (more manual work).

## 11. Theme without a flash at startup

- **Decision**: Rust reads `settings.toml` before creating the window and passes the theme,
  language, and reduced-motion via Tauri `initialization_script`
  (`window.__FREESPACE_BOOT__`). A synchronous `public/boot.js` in `<head>` before CSS sets
  `data-theme` and `lang`. In a browser without Tauri, `boot.js` uses `prefers-color-scheme`.
- **Rationale**: `localStorage` is prohibited, and an asynchronous IPC after the first paint
  causes a flash. The initialization script runs in WebView2 before the page loads and requires
  no network.
- **Alternatives considered**: store the theme in localStorage (violates FR-011); wait for IPC
  and switch the theme after mounting (flash); a separate native window theme (does not cover
  the content).

## 12. WebView2 profile in the `data` directory

- **Decision**: before creating the webview, `WEBVIEW2_USER_DATA_FOLDER` is set to
  `<exe_dir>\data\webview2` (or the equivalent Tauri `data_directory`, if available in the
  chosen version). Additionally, arguments are set that disable background Edge network
  components. The path is checked for writability; on error, the application reports the
  problem and runs without saving the profile.
- **Rationale**: by default WebView2 creates a profile in `%LOCALAPPDATA%`, which directly
  violates FR-093/FR-106 and the constitution. A local profile also simplifies portability and
  trace removal.
- **Alternatives considered**: keep the default profile (violation); disable the profile
  entirely (impossible for WebView2); move the profile after startup (too late and risky).
- **Spike result (T003)**: Tauri 2.11.6 exposes `WebviewWindowBuilder::data_directory` and
  `additional_browser_args`, so no environment variable is needed: the window is created in
  `setup`, the profile lives in `data/webview2`, and background Edge components are disabled
  by arguments. Verified by building and testing `freespace-desktop`; profile creation is
  checked on the first window launch.

## 13. Settings and migration without localStorage

- **Decision**: UI settings are a new `[ui]` section in `data/settings.toml` (theme, language,
  density, units, reduced motion, last screen, column visibility); window state is
  `data/ui-state.toml` (size, position, maximized, monitor). Before migration, a backup copy
  `settings.toml.bak` is created; the migration is versioned and idempotent. React does not use
  `localStorage`/`IndexedDB`.
- **Rationale**: a single store and a single writer (Rust); settings survive reinstallation and
  work in the portable build; no hidden data in the WebView2 profile.
- **Alternatives considered**: localStorage (violation and loss on profile clearing); a separate
  JSON in `%APPDATA%` (prohibited); keep in memory only (loss of settings).

## 14. Localization: a single TOML source

- **Decision**: `locales/ru.toml` and `locales/en.toml` remain the single source.
  `ui/scripts/gen-i18n.mjs` (Node + `smol-toml`) flattens nested tables into flat keys and
  generates `ui/src/i18n/generated/ru.ts`, `en.ts`, and `keys.ts` (a union of keys). Generated
  files are committed; CI checks for absence of diff. Rust continues to use `rust-i18n` with the
  same TOML files.
- **Rationale**: no two sets of strings, keys are typed, missing-key is caught by a test,
  language switching is instant. The generator does not pull in a heavy i18n framework.
- **Alternatives considered**: separate JSON catalogs maintained manually (desynchronization);
  serve strings over IPC (no typing and an extra runtime layer); Fluent (more powerful but
  excessive for two languages).

## 15. Navigation, deep links, and a single instance

- **Decision**: screen state is a typed enum in `shellStore`, without a URL router. The last
  screen is saved; deep links come from the CLI/context menu and are passed via the Tauri
  single-instance plugin. A repeated launch activates the existing window.
- **Rationale**: a desktop application does not need a URL; an enum provides type safety and
  simple serialization. Single-instance preserves one `data` directory and one journal.
- **Alternatives considered**: React Router (an unnecessary abstraction for desktop); multiple
  instances (journal/quarantine conflict); a deep link over the network (offline ban).

## 16. Tauri security

- **Decision**: CSP `default-src 'self'; connect-src ipc: http://ipc.localhost;
  img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'`; no remote
  URLs. Minimal capabilities: `core:default` and targeted window/event permissions; the
  fs/shell/http plugins are not connected. `withGlobalTauri: false`, devtools disabled in
  release, `dangerousDisableAssetCspModification` is not used. All paths are validated and
  canonicalized in `security.rs`/`freespace-app`.
- **Rationale**: React must not gain access to the file system or network; a narrow attack
  surface is critical for a utility that sees the entire disk. `style-src 'unsafe-inline'` is
  needed for React style attributes and does not open the network.
- **Alternatives considered**: disable CSP (unacceptable); connect fs/shell plugins for
  convenience (bypasses `freespace-ops`); `withGlobalTauri: true` (extra global surface).

## 17. WebView2 distribution

- **Decision**: a custom fixed WebView2 runtime is included in both the installer and the
  portable build; the version is pinned at release. If the measured installer size exceeds the
  agreed budget, the installer switches to `offlineInstaller`, while the portable build keeps
  the fixed runtime. The decision and sizes are recorded in the spike report.
- **Rationale**: installation and launch must work offline on a clean Windows 10 where
  WebView2 may be absent. A fixed runtime gives consistent behavior and does not depend on Edge
  updates. `offlineInstaller` is an acceptable size compromise.
- **Alternatives considered**: require the system WebView2 (risk of failure); download
  bootstrapper (violates offline); MSIX with a dependency on Edge (installation restrictions).

## 18. Installer: Tauri NSIS instead of Inno

- **Decision**: the primary installer is Tauri NSIS; Inno Setup is removed from the
  distribution after the scripts are ported. NSIS hooks call
  `freespace-gui.exe context-menu uninstall`, remove `data` and shortcuts; the portable ZIP is
  built separately. Signing is via Tauri `signCommand` or `certificateThumbprint`.
- **Rationale**: one installer, native support for WebView2, Tauri artifacts, and signing; less
  duplication and divergence. Inno scripts are ported to NSIS hooks and tested.
- **Alternatives considered**: keep Inno as a wrapper (two installers and double maintenance);
  MSI (worse customization of `data` removal); MSIX (registry and context menu restrictions).

## 19. Executable name and product

- **Decision**: the product is `FreeSpace`, the GUI crate is `freespace-desktop`, the binary is
  `freespace-gui.exe`. Shortcuts, scripts, the context menu, and the installer continue to work.
- **Rationale**: preserves compatibility with external scenarios and does not create a second
  product.
- **Alternatives considered**: `freespace-desktop.exe` (breaks shortcuts and documentation);
  `freespace.exe` (conflicts with the CLI); renaming after release (unnecessary migration).

## 20. Testing

- **Decision**: Rust — unit/integration tests for `freespace-app`, commands, validation,
  serialization, and settings migration; frontend — Vitest + RTL; Playwright on a Vite build
  with a mock transport; Tauri E2E — `tauri-driver` + WebdriverIO on Windows; accessibility —
  `@axe-core/playwright` + Narrator; visual regression — Playwright screenshots with fixed
  fonts, DPI, and tolerances; network guard — request interception and CSP verification.
- **Rationale**: different layers require different tools; the mock transport allows fast and
  stable verification of 11 screens, while Tauri E2E confirms the real IPC and packaging.
- **Alternatives considered**: manual checks only (does not scale); E2E only (slow and
  unstable); Cypress (worse WebView2/Windows support).

## 21. Performance and measurements

- **Decision**: a synthetic tree generator for 10 million nodes in Rust; core benchmarks
  remain; UI measurements — Playwright Performance API for frames/responsiveness, Windows
  process counters for memory, E2E scenarios on a control VHD. Budgets: 200 ms, 30 fps, 4 GB,
  3/5 s.
- **Rationale**: measurable gates before stories 3+; synthetic data is reproducible and does
  not touch real data. Memory is counted across the entire process tree, not just Rust.
- **Alternatives considered**: measurements on the user's real disk (prohibited); manual
  estimation only (not objective); microbenchmarks without UI (do not reflect WebView).

## 22. Accessibility

- **Decision**: native DOM semantics (buttons, tables, trees, dialogs), ARIA only where needed,
  focus management, keyboard shortcuts, announcements via `aria-live`,
  `prefers-reduced-motion`, `prefers-contrast`. Narrator is checked manually.
- **Rationale**: WebView2 provides UIA; proper semantics give accessibility without a separate
  layer. Automated checks do not replace Narrator.
- **Alternatives considered**: canvas-only interface (inaccessible); accesskit (for egui, not
  needed); ARIA on every element (noise for the screen reader).

## 23. Errors, logs, and observability

- **Decision**: a single error envelope `{ code, category, message_key, details,
  retryable, task_id? }`; a React error boundary per screen and for the shell; local logs
  `data/logs/ui.log` with rotation; no telemetry. Command errors are logged on the Rust side,
  the UI shows a localized message and the journal reference.
- **Rationale**: the user sees a clear message, support gets the code and details; data does not
  leave the machine. Error boundaries prevent one screen from crashing the shell.
- **Alternatives considered**: show raw Rust text (poorly localized and exposes internals);
  telemetry (prohibited); console only (undiagnosable).

## 24. Window, splash screen, and native chrome

- **Decision**: a native Windows window with the system title bar and buttons; inside, a custom
  content header. A splash screen is shown until the UI is ready and hidden after the first
  frame; window state is saved in `ui-state.toml`.
- **Rationale**: the native title bar provides accessibility, the system menu, Snap behavior,
  and correct rendering at 100–200% DPI for free. The custom header preserves the mockup's
  visual language without reinventing window buttons.
- **Alternatives considered**: fully frameless (more code, accessibility and DPI risk); a
  standard splash without a custom splash (less cohesive); remember state in localStorage
  (prohibited).

## 25. Build and CI

- **Decision**: two independent CI jobs on `windows-latest`: Rust (`fmt`, `clippy -D
  warnings`, `test`, benchmarks on a schedule) and frontend (`npm ci`, `typecheck`, `lint`,
  Vitest, Playwright, visual, axe). Tauri E2E and packaging are a separate job with a dependency
  cache. i18n generation is checked for absence of diff. Cache — `Swatinem/rust-cache` and
  `actions/setup-node` with `npm` cache.
- **Rationale**: fast feedback loops and independent gates; Windows is mandatory for WebView2,
  paths, and DPI. Generation checking prevents catalogs from diverging.
- **Alternatives considered**: one shared job (slow); Linux frontend build (does not check
  WebView2); skipping visual tests in CI (regressions).

## 26. Justification of new dependencies

| Dependency | Purpose | Why needed | Alternative |
|------------|---------|------------|-------------|
| `tauri`, `tauri-build` 2.x | Native window + WebView2 + IPC | Approved stack; access to the Windows window and packaging | egui (rejected), WinUI 3 (complex) |
| `@tauri-apps/api` 2.x | Typed command/event invocation | Official Tauri client | Hand-written `window.__TAURI__` (worse types) |
| `react`, `react-dom` 18.3 | UI | Approved stack and mockup | Preact (smaller ecosystem), React 19 (risk) |
| `typescript` 5.6, `vite` 6.x | Types and build | IPC typing and fast dev/build | webpack (slower), swc-only (less control) |
| `@tanstack/react-table` 8.x | Column and table model | A single DataTable across 11 screens | Custom model (more code), AG Grid (license) |
| `@tanstack/react-virtual` 3.x | Virtualization | 10 million rows without a DOM explosion | Custom virtualizer (risk), react-window (less flexible) |
| `zustand` 5.x | View state | Targeted subscriptions without providers | Context/useReducer (re-renders), Redux (heavy) |
| `lucide-react` 0.468 | Icons | Already in the mockup, tree-shaking | SVG sprite (manual work) |
| `smol-toml` (dev) | i18n generation | Reads existing TOML without a second format | Manual JSON (desynchronization) |
| `vitest`, RTL, user-event, jsdom (dev) | Frontend tests | Fast component tests | Jest (slower with Vite) |
| `eslint`, `typescript-eslint`, react-hooks, react-refresh (dev) | TS/React lint | Catches hook errors, unused code, and style violations before tests | `tsc` only (does not catch hooks), Biome (younger) |
| `@playwright/test` (dev) | UI/visual tests | Cross-browser support, screenshots, tracing | Cypress (worse WebView2) |
| `@axe-core/playwright` (dev) | Accessibility | Automated AA checks | Manual audit (does not scale) |
| `tauri-driver`, `webdriverio` (dev) | Tauri E2E | Real WebView2 and IPC | Playwright directly (does not control WebView2) |
| `freespace-ops` (workspace) | Preview, journal, quarantine and undo in `freespace-app` | The operation layer must call the native safe implementation instead of duplicating it; `freespace-app` stays UI-agnostic | Keep operations in Tauri commands (violates principle I) |
| `freespace-report` (workspace) | Create and compare `.fssnap` in `freespace-app` | The Snapshots screen reuses the existing format and diff algorithm instead of a second implementation | A GUI-only snapshot format (incompatible with the CLI and 001) |

## 27. Closed open decisions

| Question from the plan | Decision |
|------------------------|----------|
| WebView2 fixed vs system | Fixed runtime in the distribution; `offlineInstaller` is a fallback when the size budget is exceeded |
| Tauri bundler vs Inno | Tauri NSIS; Inno is removed, scripts are ported to NSIS hooks |
| TanStack vs custom virtualizer | TanStack Table + Virtual |
| Zustand vs context | Zustand |
| egui after replacement | Removed completely; no second UI is maintained |
| Beta/flag | Not needed; full replacement immediately |
| Exe name | `freespace-gui.exe` is preserved |
| WebView2 profile | `data/webview2`, not `%LOCALAPPDATA%` |
| Default theme | System |
| Window title bar | Native Windows |

## 28. Dependency audit

Checked on 2026-11-16 against `package.json`/`package-lock.json` and `Cargo.lock`.

- **Frontend runtime (6)**: `react`, `react-dom`, `zustand` (state), `@tanstack/react-table` and
  `@tanstack/react-virtual` (tables and virtualization), `@tauri-apps/api` (IPC), `lucide-react`
  (icons). There are no network libraries, analytics or telemetry-bearing UI frameworks.
- **Dev/test**: Vite, TypeScript, ESLint, Vitest + Testing Library + jsdom, Playwright +
  `@axe-core/playwright`, WebdriverIO + `edgedriver` for Tauri E2E, `smol-toml` for the
  localization generator. Every package is used by a `package.json` script.
- **Rust**: the dependency set is recorded in `plan.md`; `cargo tree --duplicates` reports only
  transitive ecosystem duplicates (`base64`, `bitflags`, `digest`, `flate2`); there are no direct
  duplicates between workspace crates.
- Constraints: `@tauri-apps/cli` and `tauri-build` are build-time only; `withGlobalTauri: false`,
  and `check:network` rejects `fs`/`shell`/`http` permissions and remote URLs.
- All licences are permissive (MIT/Apache-2.0/BSD); the About screen lists the key components
  offline and never opens a remote page.
- Tauri versions are aligned: the Rust crate `tauri 2.12.5` with `@tauri-apps/api 2.12.4`; the CLI
  enforces the same major/minor at build time. Bumping `tauri`/`wry` (2.11.6 → 2.12.5 / 0.55.1 →
  0.57.2) was required for that check and is verified by `npm run tauri -- build --debug --no-bundle`.
