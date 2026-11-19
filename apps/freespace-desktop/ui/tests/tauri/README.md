# Tauri end-to-end tests

These specs drive the real `freespace-gui.exe` through `tauri-driver` and verify
IPC, packaging and the WebView2 host — things Playwright cannot reach from a
plain browser.

## Prerequisites

```powershell
cargo install tauri-driver --locked
npm --prefix apps/freespace-desktop/ui ci
cd apps/freespace-desktop/ui
npm run tauri -- build --debug --no-bundle
```

The debug build must embed the production frontend, so it is produced by the
Tauri CLI (`--no-bundle` skips the installer). A plain `cargo build` keeps the
`devUrl`, which points the window at the Vite dev server and fails without one.

`edgedriver` (an npm dev dependency) provides Microsoft Edge WebDriver. The
suite starts `tauri-driver` together with that driver automatically and stops
them when the run ends, so there is no process to prepare beforehand:

```powershell
npm run test:tauri
```

A driver that already listens on `127.0.0.1:4444` is reused and left running.
Set `FREESPACE_TAURI_DRIVER` to use a driver other than `tauri-driver` from
`PATH`, or `FREESPACE_EDGEDRIVER` to pin `msedgedriver.exe`.

Set `FREESPACE_ADMIN_TESTS=1` only when the machine may run administrative
scenarios; destructive specs must use temporary directories and test VHDs.
