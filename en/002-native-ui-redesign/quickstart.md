# Quickstart: verifying the 002 interface

**Feature**: `002-native-ui-redesign` | **Date**: 2026-11-16

The guide describes how to build and verify the new interface once the `apps/freespace-desktop`
skeleton exists. All destructive scenarios run only on temporary directories and test VHDs; real
user data is never touched.

## 1. Prerequisites

- Windows 10 21H2+ or Windows 11, x64.
- Rust stable 1.85+ (edition 2024) and the `rustfmt`/`clippy` components.
- Node.js 20.19+ and npm.
- WebView2: system or the fixed runtime from the distribution.
- For E2E: `tauri-driver` and Edge WebDriver compatible with the installed WebView2.
- For administrative scenarios: a separate test NTFS volume/VHD and
  `FREESPACE_ADMIN_TESTS=1`.
- Free space for `target/`, `node_modules/`, and test images.

Checking the environment:

```powershell
rustc --version
cargo --version
node --version
npm --version
```

## 2. Installing dependencies

```powershell
# Rust workspace
cargo fetch

# Frontend
cd apps/freespace-desktop/ui
npm ci

# Playwright browsers (once per machine)
npx playwright install chromium
```

## 3. Running in development mode

```powershell
cd apps/freespace-desktop/ui
npm run tauri dev
```

Expected:

- a native Windows window opens with the system title bar;
- the theme is applied before the first paint, with no flash;
- the shell shows 24 screens in the side navigation;
- the "Disks" screen loads the volume list;
- a WebView2 profile is created in `data/webview2`, not in `%LOCALAPPDATA%`;
- a `[ui]` section appears in `data/settings.toml`;
- there are no network requests.

## 4. Building

```powershell
cd apps/freespace-desktop/ui
npm run build

cd ..
cargo build --release -p freespace-desktop
```

Artifacts:

- `target/release/freespace-gui.exe` (the name is preserved);
- frontend assets inside the Tauri bundle;
- installer and portable ZIP — via the `npm run tauri build` command (see scenario 11).

## 5. Mandatory checks

```powershell
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Frontend
cd apps/freespace-desktop/ui
npm run typecheck
npm run lint
npm run test:unit
npm run test:e2e
npm run test:a11y
npm run test:visual
npm run check:i18n
npm run check:tokens
npm run check:network
```

Expected: all commands are green; `check:i18n` finds no diff in the generated catalogs;
`check:tokens` finds no hardcoded colors; `check:network` finds no network calls.

## 6. Scenario 1 — Core: disks, scanning, tree, map (US1)

1. Prepare a test directory with a known structure or a test VHD:

   ```powershell
   $root = Join-Path $env:TEMP "freespace-ui-fixture"
   New-Item -ItemType Directory -Force $root | Out-Null
   1..2000 | ForEach-Object {
       $dir = Join-Path $root ("dir{0:D3}" -f ($_ % 40))
       New-Item -ItemType Directory -Force $dir | Out-Null
       Set-Content -Path (Join-Path $dir ("file{0:D4}.bin" -f $_)) -Value ("x" * ($_ % 4096))
   }
   ```

2. Open "Disks", select the test volume/directory, and start the analysis.
3. Verify that progress, the current path, the object count, and the time are updating; the UI does
   not freeze.
4. Click "Cancel" during the analysis: the task stops in ≤ 2 s and the results are marked as
   incomplete.
5. Start the analysis again before it completes.
6. Open "Tree": rows appear in ≤ 1 s, expansion/collapse works, and the totals match the
   reference.
7. Open "Map": the areas are proportional, drilling down and the "breadcrumbs" work, and the
   selection is synchronized with the tree and the details panel.
8. On the 10 million node control set, verify 200 ms/30 fps/4 GB (see section 16).

Expected: no operation blocks the shell; at 10 million nodes the UI remains responsive.

## 7. Scenario 2 — Analysis and cleanup (US2)

1. On the test set, open "Largest Files", select N=100 and N=1000, and apply the type and path
   filter.
2. Open "Types", select an extension/category, and compare the total, count, and share.
3. Open "Duplicates", start the search, and apply the "keep one copy" rule, but do not confirm the
   deletion.
4. Open "Cleanup", select a recommendation, and verify the volume, risk, and transition to the
   preview.
5. Create two snapshots with known differences and run a comparison; verify sorting by growth.
6. Try to compare incompatible snapshots: a clear error must appear without modifying files.

Expected: the totals match the reference; no copy is deleted without confirmation.

## 8. Scenario 3 — Safe operations (US3)

Temporary directory only:

```powershell
$ops = Join-Path $env:TEMP "freespace-ui-ops"
New-Item -ItemType Directory -Force $ops | Out-Null
1..20 | ForEach-Object { Set-Content (Join-Path $ops "victim$_.txt") "test" }
```

1. Select files and initiate deletion.
2. Verify the preview: the list, total volume, deletion mode, and warnings.
3. Perform deletion to the Recycle Bin; verify the journal entry and restoration.
4. Repeat with quarantine; restore the objects.
5. Repeat with permanent deletion; verify that additional confirmation is required.
6. Cancel the operation before it completes; verify the actual result in the journal.
7. Try an operation on a system path without elevation: a rights request and a refusal without
   modifying data must occur.

Expected: 100% of operations go through preview, confirmation, and the journal; cancellation is
available where the operation is reversible.

## 9. Scenario 4 — File system and operations (US4)

1. Create a path > 260 characters (temporary directory, without system policy).
2. Open "Long Paths" and verify the display, full path copying, rename, and deletion.
3. Open "Size Details": verify logical/on-disk/physical size, ADS, compression, sparse, and the
   metric type label.
4. Open "Rename": set a template and verify the preview of old/new names, conflicts, and the object
   count.
5. Open "Permissions": verify ACL/attributes, the preview, and refusal without elevation.
6. Verify the journal: the full path, error code, and result.

Expected: long paths work without `LongPathsEnabled`; no operation is performed without a preview.

## 10. Scenario 5 — Inspection and reports (US5)

1. Open "Metadata" on an EXIF/audio/video/PE file; verify the source and empty values.
2. Open "Owners": verify the totals, counts, and navigation to the files.
3. Open "Archives": verify the content list, sizes, and share; a password-protected/corrupted
   archive must not be extracted to disk.
4. Export CSV, XML, HTML, XLSX; compare the totals with the UI.
5. Open a held file in "Unlock": verify the processes and release of the lock after confirmation.
6. Verify queuing for deletion on reboot (if available).

Expected: reading does not modify files; the export opens in the target tools.

## 11. Scenario 6 — Operations and settings (US6)

1. In "Settings", switch the theme, language, density, units, and reduced motion; verify that there
   is no restart and no state loss.
2. In "Shredder", select a temporary file, verify the method, passes, and warnings; run it only on a
   temporary file.
3. Enable "Write Monitoring", create a write to disk, and verify the event in ≤ 2 s and the filters.
4. Verify "Elevation": the reason, consent/refusal, and the absence of changes on refusal.
5. Open "About": version, build, WebView2, licenses, `data_dir` without network requests.

Expected: settings are applied instantly; the shredder does not start without separate
confirmation.

## 12. Scenario 7 — Themes, DPI, and motion (US7)

1. Switch the dark/light/system theme on every screen.
2. Restart the application: there is no flash of the wrong theme.
3. Change the Windows theme while the system theme is selected: the UI updates without a restart.
4. Verify 100/125/150/200% DPI: no clipping, overlapping, or inaccessible buttons.
5. Enable "Reduce motion" in Windows: optional transitions are disabled.

Expected: AA contrast, Treemap palettes for both themes, no state loss.

## 13. Scenario 8 — Accessibility (US8)

1. Disconnect the mouse/trackpad.
2. Walk through the scenarios: navigation, search, selection, opening a dialog, confirmation,
   cancellation, theme and language switching.
3. Verify the focus order and focus return after a dialog.
4. Enable Narrator: verify headings, progress, errors, `aria-live`.
5. Verify that status is not conveyed by color alone.

Expected: ten key scenarios are performed with the keyboard only; Narrator finds no blocking
defects.

## 14. Scenario 9 — Localization (US9)

1. Switch RU/EN on every screen.
2. Verify that filters, selection, progress, and scroll position are preserved.
3. Verify the absence of untranslated strings and of Cyrillic in `en`.
4. Verify numbers, dates, and units (binary/decimal).
5. Run `npm run check:i18n`: there is no diff in the generated catalogs.

Expected: 100% of strings are localized; there are no missing keys.

## 15. Scenario 10 — Visual regression (US10)

```powershell
cd apps/freespace-desktop/ui
npm run test:visual
npm run test:visual:update   # only after a deliberate review of the diff
```

Full matrix: 24 screens × 2 themes × 4 DPI × 2 languages. The PR set is the 8 critical screens and all
24 in the dark theme, 100%, RU. Expected: the baselines match; a deliberate token change breaks the
test.

## 16. Performance at 10 million nodes

1. Generate the synthetic set (temporary directory/VHD only):

   ```powershell
   cargo run -p freespace-app --example gen_fixture -- --nodes 10000000 --out $env:TEMP\freespace-10m
   ```

2. Start the analysis and verify:
   - the first interactive screen in ≤ 3 s;
   - the first screen with data in ≤ 5 s after readiness;
   - responsiveness ≤ 200 ms;
   - Treemap ≥ 30 fps;
   - memory of the entire process tree ≤ 4 GB.
3. Record the results in the performance report (FR-130).

## 17. Scenario 11 — Distribution, offline, and interface replacement (US11)

1. Build the installer and the portable ZIP on a clean Windows machine without network.
2. Install: WebView2 is installed offline, the application starts, and there are no network requests.
3. Verify `data` next to the exe, `data/webview2`, and the absence of entries in `%APPDATA%`,
   `%LOCALAPPDATA%`, `%ProgramData%`.
4. Verify the `freespace-gui.exe` name, shortcuts, and the context menu (disabled by default).
5. Uninstall the application: files, `data`, and menu entries are removed; user files outside the
   application are not affected.
6. Verify that there is no egui application in the distribution and the workspace; exactly one GUI is
   built.

Expected: install/launch/uninstall without network; one UI; the CLI contract is unchanged.

## 18. Acceptance criteria

- [ ] 24/24 screens are implemented and pass the scenarios.
- [ ] 0 network calls; CSP and capabilities are minimal.
- [ ] 200 ms / 30 fps / 4 GB on the control set.
- [ ] AA in both themes; DPI 100–200%; reduced motion.
- [ ] RU/EN without missing keys; 0 hardcoded strings and colors.
- [ ] 100% of destructive operations with preview, confirmation, and journal.
- [ ] Rust, unit, E2E, visual, and a11y tests are green.
- [ ] Install/uninstall without network; `data` only next to the exe.
- [ ] egui removed; one GUI; the CLI contract is unchanged.
- [ ] The parity matrix is filled in and verified.

## 19. Diagnostics

| Symptom | Check |
|---------|-------|
| The window does not open | WebView2 is installed/compatible; `data/webview2` is writable |
| The theme flashes | `boot.js` loads before CSS; `window.__FREESPACE_BOOT__` is populated |
| Settings are not saved | `data/settings.toml` is accessible; no permission error; version migration |
| Data does not appear | `app_capabilities`, `scan_status`, `generation`, `stale_cursor` |
| The Treemap is slow | `max_tiles`, `min_tile_px`, viewport sizes, Canvas DPR |
| E2E does not start | `tauri-driver`, Edge WebDriver, WebView2 version |
| The visual test is unstable | fonts, DPI, viewport, tolerance, Windows CI |
| Network detected | CSP, WebView2 args, network guard, absence of remote URLs |
