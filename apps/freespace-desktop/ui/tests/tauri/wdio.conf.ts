// WebdriverIO configuration for the real Tauri window.
//
// The suite drives the embedded-asset debug build of `freespace-gui.exe`
// through `tauri-driver` and Microsoft Edge WebDriver. `startDriver`/
// `stopDriver` bring the WebDriver stack up and down around the run, so no
// external process has to be prepared. Prerequisites are
// `cargo install tauri-driver --locked`, the `edgedriver` npm package and
// `npm run tauri -- build --debug --no-bundle` (a plain `cargo build` keeps the
// `devUrl` and needs a running Vite dev server).

import path from 'node:path';

import { startDriver, stopDriver } from './driver';

// tests/tauri -> tests -> ui -> freespace-desktop -> apps -> repository root.
const repository = path.resolve(import.meta.dirname, '../../../../..');
const application = path.join(repository, 'target', 'debug', 'freespace-gui.exe');

export const config = {
  hostname: '127.0.0.1',
  port: 4444,
  path: '/',
  specs: [path.join(import.meta.dirname, 'specs/**/*.spec.ts')],
  maxInstances: 1,
  capabilities: [
    {
      maxInstances: 1,
      'tauri:options': {
        application,
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
  onPrepare: async () => {
    await startDriver();
  },
  onComplete: () => {
    stopDriver();
  },
};
