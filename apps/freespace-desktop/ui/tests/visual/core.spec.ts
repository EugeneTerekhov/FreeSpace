import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import {
  THEMES,
  applyBoot,
  baselineName,
  prepareScreen,
  stabilize,
  waitForShell,
} from './harness';

for (const theme of THEMES) {
  test(`dashboard ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('dashboard', theme));
  });

  test(`scan ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await screens(page).open('scan');
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('scan', theme));
  });

  test(`tree ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await prepareScreen(page, 'tree', 'ru');
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('tree', theme));
  });

  test(`treemap ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await prepareScreen(page, 'treemap', 'ru');
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('treemap', theme), {
      maxDiffPixelRatio: 0.02,
    });
  });
}
