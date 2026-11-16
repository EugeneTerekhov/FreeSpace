import { defineConfig, devices } from '@playwright/test';

const isCi = Boolean(process.env.CI);

// Playwright drives the Vite build in a browser, not the Tauri window: it checks
// layout, keyboard, themes and accessibility without the WebView2 runtime. The
// real IPC and packaging path is covered by the WebdriverIO suite.
export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: isCi,
  retries: 1,
  ...(isCi ? { workers: 1 } : {}),
  timeout: 60_000,
  reporter: isCi
    ? [['github'], ['html', { open: 'never' }]]
    : [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://127.0.0.1:1420',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  expect: {
    timeout: 10_000,
    toHaveScreenshot: {
      maxDiffPixelRatio: 0.01,
    },
  },
  // Every reference lives in one reviewable tree: the matrix adds a directory
  // per screen, the delivery batches add flat names. The product is Windows
  // only, so the platform suffix is not part of the name.
  snapshotPathTemplate: '{testDir}/baselines/{arg}{ext}',
  projects: [
    {
      name: 'e2e',
      testDir: './tests/e2e',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
      },
    },
    {
      name: 'visual',
      testDir: './tests/visual',
      testIgnore: ['**/pr.spec.ts', '**/*.test.ts'],
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
      },
    },
    {
      name: 'pr',
      testDir: './tests/visual',
      testMatch: ['**/pr.spec.ts'],
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
      },
    },
    {
      name: 'a11y',
      testDir: './tests/a11y',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
      },
    },
  ],
  webServer: {
    command: 'npm run dev',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: !isCi,
    timeout: 120_000,
  },
});
