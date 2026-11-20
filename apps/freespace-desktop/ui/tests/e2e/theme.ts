import type { Page } from '@playwright/test';

/**
 * Applies a resolved theme without touching the saved preference.
 *
 * Waits two frames first so the asynchronous settings bootstrap has already
 * written the saved theme; otherwise it would overwrite the requested one.
 */
export async function setTheme(page: Page, theme: 'dark' | 'light'): Promise<void> {
  await page.evaluate(async () => {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          resolve();
        });
      });
    });
  });
  await page.evaluate((value) => {
    const root = document.documentElement;
    // Match the runtime: a programmatic palette swap is instantaneous, so the
    // audit never samples colours halfway through a transition.
    root.dataset.themeSwitching = 'true';
    root.dataset.theme = value;
    root.style.colorScheme = value;
    requestAnimationFrame(() => {
      delete root.dataset.themeSwitching;
    });
  }, theme);
}
