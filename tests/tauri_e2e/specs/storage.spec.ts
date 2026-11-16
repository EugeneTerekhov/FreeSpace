// Storage contract for the packaged application.
//
// Every writable path must stay inside `<exe_dir>\data`: settings, the journal
// database, the quarantine, the hash cache and the WebView2 profile. The test
// inspects the real filesystem because no frontend API can prove the absence of
// `%APPDATA%`/`%LOCALAPPDATA%` writes.

import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

import { browser, expect } from '@wdio/globals';

const repository = path.resolve(__dirname, '../../..');
const executableDir = path.join(repository, 'target', 'debug');
const executable = path.join(executableDir, 'freespace-gui.exe');
const dataDir = path.join(executableDir, 'data');

/** Waits until `check` is true or the timeout expires. */
async function waitFor(
  label: string,
  check: () => boolean,
  timeout = 20_000,
): Promise<void> {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (check()) return;
    await browser.pause(250);
  }
  throw new Error(`timed out waiting for ${label}`);
}

describe('application-local storage', () => {
  it('keeps every writable path inside data next to the executable', async () => {
    expect(existsSync(executable)).toBe(true);
    expect(existsSync(dataDir)).toBe(true);

    // The WebView2 profile is created asynchronously by the runtime.
    await waitFor('data/webview2', () =>
      existsSync(path.join(dataDir, 'webview2')),
    );
    await waitFor('the store database', () =>
      existsSync(path.join(dataDir, 'freespace.db')),
    );

    expect(statSync(dataDir).isDirectory()).toBe(true);
  });

  it('persists settings inside data after navigation', async () => {
    await browser.$('[data-screen="settings"]').click();

    const settings = path.join(dataDir, 'settings.toml');
    await waitFor('data/settings.toml', () => existsSync(settings));

    const text = readFileSync(settings, 'utf8');
    expect(text).toContain('last_screen');
    expect(text).toContain('settings');
  });

  it('never writes to the roaming or local user profile', () => {
    const roaming = process.env.APPDATA ?? '';
    const local = process.env.LOCALAPPDATA ?? '';

    if (roaming !== '') {
      expect(existsSync(path.join(roaming, 'com.freespace.desktop'))).toBe(false);
      expect(existsSync(path.join(roaming, 'FreeSpace'))).toBe(false);
    }

    if (local !== '') {
      // The default WebView2 profile location for an unprepared process.
      expect(
        existsSync(path.join(local, 'freespace-gui.exe.WebView2')),
      ).toBe(false);
      expect(existsSync(path.join(local, 'com.freespace.desktop'))).toBe(false);
    }
  });
});
