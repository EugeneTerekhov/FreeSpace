import { expect, test } from '@playwright/test';

import { SCREEN_IDS, screens } from './pages/screens';

test.describe('themes, DPI and motion', () => {
  test('the boot script applies the saved theme before the shell paints', async ({ page }) => {
    await page.addInitScript(() => {
      (window as unknown as { __FREESPACE_BOOT__: unknown }).__FREESPACE_BOOT__ = {
        theme: 'light',
        language: 'ru',
        reduced_motion: 'off',
      };
    });
    await page.goto('/');

    const html = page.locator('html');
    // `data-theme-boot` proves the synchronous boot script picked the injected
    // theme before React; the shell then applies the stored settings.
    await expect(html).toHaveAttribute('data-theme-boot', 'light');
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  });

  test('an explicit theme survives navigation across all 24 screens', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('settings');

    await page
      .getByRole('radiogroup', { name: 'Тема' })
      .getByRole('radio', { name: 'Светлая' })
      .click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');

    for (const id of SCREEN_IDS) {
      await page.locator(`[data-screen="${id}"]`).click();
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
      await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    }
  });

  test('the system preference follows the OS colour scheme', async ({ page }) => {
    await page.emulateMedia({ colorScheme: 'dark' });
    await page.goto('/');
    const app = screens(page);
    await app.open('settings');

    await page
      .getByRole('radiogroup', { name: 'Тема' })
      .getByRole('radio', { name: 'Системная' })
      .click();

    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

    await page.emulateMedia({ colorScheme: 'light' });
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');

    await page.emulateMedia({ colorScheme: 'dark' });
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  });

  for (const scale of [1, 1.25, 1.5, 2]) {
    test(`layout survives ${String(scale * 100)}% DPI without clipping`, async ({
      browser,
    }) => {
      const context = await browser.newContext({
        viewport: {
          width: Math.round(1440 / scale),
          height: Math.round(900 / scale),
        },
        deviceScaleFactor: scale,
      });
      const page = await context.newPage();
      try {
        await page.goto('/');
        const app = screens(page);
        await app.open('settings');

        const overflow = await page.evaluate(() => ({
          scrollWidth: document.documentElement.scrollWidth,
          innerWidth: window.innerWidth,
        }));
        expect(overflow.scrollWidth).toBeLessThanOrEqual(overflow.innerWidth);

        // High DPI shrinks the CSS viewport; the sidebar collapses to icons so
        // the content keeps its usable width.
        if (scale >= 2) {
          const width = await page
            .locator('aside')
            .first()
            .evaluate((element) => getComputedStyle(element).width);
          expect(Number.parseFloat(width)).toBeLessThanOrEqual(64);

          for (const id of SCREEN_IDS) {
            await page.locator(`[data-screen="${id}"]`).click();
            await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
            const screenOverflow = await page.evaluate(() => ({
              scrollWidth: document.documentElement.scrollWidth,
              innerWidth: window.innerWidth,
            }));
            expect(
              screenOverflow.scrollWidth,
              `${id} overflows at ${String(scale * 100)}%`,
            ).toBeLessThanOrEqual(screenOverflow.innerWidth);
          }
        }

        await expect(page.getByRole('main')).toBeVisible();
      } finally {
        await context.close();
      }
    });
  }
});
