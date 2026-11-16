import { expect, test, type Page } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import {
  THEMES,
  applyBoot,
  baselineName,
  stabilize,
  waitForShell,
  type ThemeName,
} from './harness';

const CRITICAL = ['dashboard', 'tree', 'settings', 'about'] as const;

type CriticalScreen = (typeof CRITICAL)[number];

async function prepare(
  page: Page,
  screen: CriticalScreen,
  theme: ThemeName,
): Promise<void> {
  await applyBoot(page, { theme, language: 'ru' });
  await page.goto('/');
  await waitForShell(page);

  if (screen === 'tree') {
    const app = screens(page);
    await app.withSnapshot('tree');
    await app.openTreeRoot();
    return;
  }

  if (screen !== 'dashboard') {
    await screens(page).open(screen);
  }

  if (screen === 'settings') {
    await expect(page.getByLabel('Язык')).toBeVisible();
  }
  if (screen === 'about') {
    await expect(page.getByRole('grid', { name: 'Лицензии' })).toBeVisible();
  }
}

for (const theme of THEMES) {
  for (const screen of CRITICAL) {
    test(`${screen} ${theme} desktop`, async ({ page }) => {
      await prepare(page, screen, theme);
      await stabilize(page);
      await expect(page).toHaveScreenshot(baselineName(screen, theme));
    });

    test(`${screen} ${theme} narrow`, async ({ page }) => {
      await page.setViewportSize({ width: 1024, height: 768 });
      await prepare(page, screen, theme);
      await stabilize(page);
      await expect(page).toHaveScreenshot(baselineName(screen, theme, 'narrow'));
    });
  }

  test(`dashboard ${theme} at 200% DPI`, async ({ browser }) => {
    const context = await browser.newContext({
      viewport: { width: 720, height: 450 },
      deviceScaleFactor: 2,
    });
    const page = await context.newPage();
    try {
      await prepare(page, 'dashboard', theme);
      await stabilize(page);
      await expect(page).toHaveScreenshot(
        baselineName('dashboard', theme, 'dpi200'),
      );
    } finally {
      await context.close();
    }
  });
}
