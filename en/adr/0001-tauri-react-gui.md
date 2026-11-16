# ADR 0001: GUI on Tauri 2 + React 18 instead of egui

- **Status**: Accepted
- **Date**: 2026-11-16
- **Context**: FreeSpace 002 — a unified next-generation native interface
- **Related documents**: [spec.md](../002-native-ui-redesign/spec.md),
  [plan.md](../002-native-ui-redesign/plan.md),
  [research.md](../002-native-ui-redesign/research.md),
  [ui-redesign-plan.md](../../docs/ui-redesign-plan.md)

## Context

FreeSpace is a Windows utility for analyzing and reclaiming disk space. The core, CLI,
operations and WinAPI are implemented in Rust. The existing egui/eframe GUI contains 24 screens and
fully covers the functionality, but does not provide the required visual language, CSS themes,
DOM accessibility and visual regression tooling. The old interface was never released,
so there is no need to support it in parallel.

The constitution (the "Technological and platform constraints" principle) allows Tauri only
given a justified need for web technologies and requires that native functionality is not
reduced. This ADR records that justification.

## Decision

Move the entire GUI to **Tauri 2 + React 18 + TypeScript + Vite**:

- `apps/freespace-desktop` — the single GUI crate, binary `freespace-gui.exe`.
- React is responsible only for presentation and input; Rust remains the sole owner of
  domain logic, WinAPI, scanning, operations, the journal, quarantine and settings.
- `crates/freespace-app` — a thin application layer (DTOs, pagination, Treemap aggregation,
  UI settings migration), so as not to duplicate the egui view logic.
- `apps/freespace-gui` and the egui dependencies are deleted as part of 002. A second UI, beta flag,
  side-by-side and a separate cutover phase are not created.
- All 24 screens, dark/light/system themes, 100–200% DPI, RU/EN, accessibility and
  offline delivery are part of 002.

## Rationale

1. **Exact transfer of the interface language.** The `web/` mockup defines the palette, typography, density,
   surfaces, radii, Treemap and functional motion. React/CSS allow it to be transferred
   1:1 without inventing equivalents in an immediate-mode GUI.
2. **Two themes and a system theme.** CSS custom properties and `prefers-color-scheme` provide
   instant switching without a restart and without two different designs.
3. **Accessibility.** DOM semantics, ARIA, focus trap, `aria-live` and Narrator are supported by
   WebView2/UIA directly; in egui an equivalent level requires significantly more
   work.
4. **Tests.** Playwright, Vitest/RTL, axe and visual regression are mature tools for
   24 screens; Tauri E2E verifies real IPC and packaging.
5. **Native functionality is not reduced.** All WinAPI calls remain in
   `freespace-platform`; Tauri commands do not call WinAPI and do not gain access to the FS bypassing
   `freespace-ops`. NTFS, MFT/USN, long paths, permissions, elevation, the journal and quarantine
   are preserved.
6. **Performance.** 10 million nodes are handled by paging, virtualization,
   aggregated Treemap tiles and event throttling; a full JSON tree is forbidden.
   The 200 ms / 30 fps / 4 GB budgets are verified separately.

## Alternatives Considered

| Alternative | Why rejected |
|-------------|--------------|
| Stay on egui/eframe | No required visual control, CSS themes, DOM accessibility and Playwright; transferring the mockup would require duplicating the design system. |
| Slint | Smaller ecosystem and worse transfer of the React mockup; accessibility and visual regression are weaker. |
| WinUI 3 | Heavy Rust + C++/C# integration; complicates the build and parity with the CLI. |
| Web application in a browser | Violates the native window, offline delivery and access to system capabilities. |
| React 19 | Does not match the approved mockup and adds migration risk with no benefit for 002. |

## Consequences

### Positive

- A single visual language across 24 screens; tokens instead of hardcoding.
- Both themes, system theme, DPI and reduced motion — with standard CSS facilities.
- Accessibility and visual regression are verified automatically.
- One GUI crate instead of two; egui is removed, support is not doubled.
- React components are reused across screens, tests and the gallery.

### Negative and risks

- A WebView2 runtime appears, along with a dependency on its version; an offline fixed runtime is needed.
- Node.js/Vite/TypeScript add a second build chain and CI jobs.
- The IPC contract and chunked data delivery require discipline and tests.
- WebView2 memory is part of the 4 GB budget; measurements of the whole process tree are needed.
- The installer size grows because of WebView2; the decision is recorded in research/spike.
- Loss of user settings during migration is unacceptable; a backup and tests are needed.

### Mitigations

- WebView2 fixed runtime in the distribution; `offlineInstaller` — a fallback if the budget is exceeded.
- CSP `default-src 'self'`, minimal capabilities, no fs/shell/http plugins.
- WebView2 profile in `data/webview2`; settings in `data/settings.toml`; no localStorage.
- Pagination, virtualization, Canvas Treemap, throttle 10–20 Hz.
- CI: Rust + frontend + Tauri E2E + visual + a11y + network guard.
- Full egui replacement: one UI, no beta flag and no side-by-side.

## Constitution Check

| Principle | Compliance | Justification |
|-----------|------------|---------------|
| I. Safe destructive operations | PASS | The UI only calls commands; preview/confirmation/journal/quarantine/undo remain in `freespace-ops`. |
| II. Accurate space accounting | PASS | Rust computes all sizes; the UI shows the result and the metric type. |
| III. Performance | PASS | The core does not change; paging, virtualization, Canvas, budgets and benchmarks. |
| IV. Native Windows accuracy | PASS | WebView2 is a Windows component; WinAPI only in `freespace-platform`; NTFS, long paths, elevation. |
| V. Test-first approach | PASS | Rust tests + Vitest/RTL/Playwright/tauri-driver/axe/visual; fixtures only temporary/VHD. |
| VI. CLI/GUI parity | PASS | One core; the CLI contract is protected by golden tests; the parity matrix. |
| VII. Offline/privacy | PASS | No remote/telemetry; CSP; fixed WebView2; data in `data`. |
| Tech constraints: Tauri with justification | PASS | This ADR records the need for web technologies and the absence of loss of native functionality. |

## Security

- CSP: `default-src 'self'; connect-src ipc: http://ipc.localhost; img-src 'self' data:;
  style-src 'self' 'unsafe-inline'; script-src 'self'`.
- Capabilities: `core:default` + targeted window/event + single-instance; no fs/shell/http.
- `withGlobalTauri: false`; devtools disabled in release.
- All paths are canonicalized; escaping the selected area is forbidden.
- Commands do not accept shell/URL/SQL; network calls are forbidden.
- Logs are local, without file contents or secrets.

## WebView2 and offline

- The fixed runtime is included in the installer and the portable build; the version is pinned at release.
- If the installer size exceeds the budget, the installer switches to `offlineInstaller`,
  while the portable build keeps the fixed runtime.
- The WebView2 profile is forced into `data/webview2`.
- Background Edge network components are disabled by arguments; the network guard in E2E confirms
  the absence of requests.

## Migration and replacement

- The old egui interface was never released; 002 fully replaces it.
- `apps/freespace-gui` and the egui dependencies are deleted; the sources remain in git history.
- The name `freespace-gui.exe` is preserved; shortcuts, scripts and the context menu do not break.
- Existing settings, journal, quarantine and snapshots are read by the new client; before migration
  a backup is created.
- The CLI, exit codes, report and snapshot formats do not change.
- Installer: Tauri NSIS; Inno is retired after the `data` removal and menu scenarios are ported.

## Open questions closed by the spike

- The exact Tauri 2 API for the WebView2 `data_directory` in the chosen version (fallback —
  `WEBVIEW2_USER_DATA_FOLDER`).
- The size of the fixed runtime vs `offlineInstaller` and the installer budget.
- The stability of `tauri-driver`/Edge WebDriver on Windows CI.
- The exact light palette values based on the contrast test results.

## References

- `specs/002-native-ui-redesign/spec.md`
- `specs/002-native-ui-redesign/plan.md`
- `specs/002-native-ui-redesign/research.md`
- `specs/002-native-ui-redesign/contracts/ui.md`
- `specs/002-native-ui-redesign/contracts/ipc.md`
- `docs/ui-redesign-plan.md`
- `.specify/memory/constitution.md`
