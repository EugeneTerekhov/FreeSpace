// Offline and CSP contract of the real Tauri window.
//
// The application must never reach the network: the CSP only allows `self` and
// the local IPC origin, and the WebView2 profile disables background
// networking. The test inspects resource timings and tries a remote fetch that
// the CSP has to reject.

import { browser, expect } from '@wdio/globals';

describe('offline and content security policy', () => {
  it('loads no remote resources', async () => {
    const remote = await browser.execute(() => {
      const entries = performance.getEntriesByType('resource') as PerformanceResourceTiming[];
      const origin = window.location.origin;

      return entries
        .map((entry) => entry.name)
        .filter((name) => /^(?:https?|wss?):\/\//.test(name) && !name.startsWith(origin));
    });

    expect(remote).toEqual([]);
  });

  it('rejects a remote fetch at runtime', async () => {
    const outcome = await browser.execute(async () => {
      try {
        await fetch('https://example.com/', { mode: 'no-cors' });
        return 'allowed';
      } catch {
        return 'blocked';
      }
    });

    expect(outcome).toBe('blocked');
  });

  it('keeps the document language and theme resolved', async () => {
    const state = await browser.execute(() => ({
      language: document.documentElement.lang,
      theme: document.documentElement.dataset.theme ?? '',
    }));

    expect(['ru', 'en']).toContain(state.language);
    expect(['dark', 'light']).toContain(state.theme);
  });
});
