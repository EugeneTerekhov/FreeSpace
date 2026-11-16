# Tauri end-to-end tests

These specs drive the real `freespace-gui.exe` through `tauri-driver` and verify
IPC, packaging and the WebView2 host — things Playwright cannot reach from a
plain browser.

## Prerequisites

```powershell
cargo install tauri-driver --locked
npm --prefix apps/freespace-desktop/ui ci
cargo build -p freespace-desktop
```

`edgedriver` (an npm dev dependency) provides Microsoft Edge WebDriver. Start the
driver and then the tests:

```powershell
tauri-driver
cd apps/freespace-desktop/ui
npm run test:tauri
```

Set `FREESPACE_ADMIN_TESTS=1` only when the machine may run administrative
scenarios; destructive specs must use temporary directories and test VHDs.
