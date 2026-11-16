// WebdriverIO configuration for the real Tauri window.
//
// The suite is driven by the Rust `tauri-driver` (installed with
// `cargo install tauri-driver --locked`) and Microsoft Edge WebDriver
// (`edgedriver`, an npm dev dependency). tauri-driver must listen on
// 127.0.0.1:4444 before `npm run test:tauri` starts.
//
// Paths below are resolved relative to this file; the application binary is the
// debug build produced by `cargo build -p freespace-desktop`.

const path = require('node:path');

export const config = {
  hostname: '127.0.0.1',
  port: 4444,
  path: '/',
  specs: [path.join(__dirname, 'specs/**/*.spec.ts')],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      'tauri:options': {
        application: path.resolve(
          __dirname,
          '../../../target/debug/freespace-gui.exe',
        ),
      },
    },
  ],
  logLevel: 'warn',
  bail: 0,
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 1,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 60_000,
  },
};
