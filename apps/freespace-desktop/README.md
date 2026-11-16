# FreeSpace desktop client

The only graphical client of FreeSpace: a Tauri 2 shell (`src-tauri/`) around the
Rust core and a React 18 + TypeScript + Vite interface (`ui/`).

## Layout

| Path | Purpose |
| ---- | ------- |
| `src-tauri/` | Tauri application, commands, state, events and security checks |
| `src-tauri/icons/icon.ico` | Windows executable, taskbar and NSIS installer icon |
| `src-tauri/icons/icon.png` | 256×256 window icon |
| `ui/` | React interface, design tokens, screens and frontend tests |
| `ui/public/splash.png` | Startup splash artwork, shown while settings load |
| `ui/public/boot.js` | Synchronous theme/language bootstrap executed before the first paint |

The artwork is derived from `free-space-image.png` in the repository root.
`icon.png` is a square crop around the black hole so the ring stays readable at
16×16; `splash.png` keeps the full artwork. The ICO contains PNG-compressed
frames for 16, 24, 32, 48, 64, 128 and 256 pixels. Tauri embeds the ICO through
`tauri-build`, so the historical `embed-resource` build script is not needed.

## Security and offline guarantees

- The CSP is `default-src 'self'` plus the local IPC endpoint; no remote assets,
  fonts or scripts are loaded.
- Capabilities are limited to `core:default` for the `main` window. The fs, shell
  and http plugins are never registered.
- `withGlobalTauri` is disabled. Devtools are enabled by Tauri only in debug
  builds and stay disabled in release builds.
- The WebView2 profile and all state move into `data` next to the executable in
  Phase 2 (T031); `%APPDATA%`, `%LOCALAPPDATA%` and `localStorage` are never used.
- `npm run check:network`, `npm run check:tokens` and `npm run check:i18n` enforce
  the offline, token and localization rules in CI.

## Parity source

The retired egui client (`apps/freespace-gui`) is not kept in the tree: its
sources remain in git history and are the reference for the 24 screens when the
React screens are ported. No second UI is built or shipped.

## Commands

```powershell
# Install the frontend toolchain once
cd apps/freespace-desktop/ui
npm ci

# Run the Tauri application in development mode
npm run tauri dev

# Build the production frontend and the desktop binary
npm run build
cargo build --release -p freespace-desktop
```

Plain `cargo build --workspace` does not need Node: the Tauri context compiles
with an empty asset directory, and development mode loads the Vite dev server.
Build the frontend first for a production binary, because `tauri build` embeds
`ui/dist` into the executable.
