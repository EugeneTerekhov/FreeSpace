import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import { THEMES, applyBoot, baselineName, stabilize, waitForShell } from './harness';

const SCREENS = ['shred', 'monitor', 'elevation', 'settings', 'about'] as const;

for (const theme of THEMES) {
  for (const screen of SCREENS) {
    test(`${screen} ${theme}`, async ({ page }) => {
      await applyBoot(page, { theme, language: 'ru' });
      await page.goto('/');
      await waitForShell(page);
      const app = screens(page);
      await app.open(screen);

      if (screen === 'settings') {
        await expect(page.getByLabel('Язык')).toBeVisible();
      }
      if (screen === 'monitor') {
        await page.getByRole('button', { name: 'Запустить мониторинг' }).click();
        await expect(page.getByText('explorer.exe')).toBeVisible();
      }
      if (screen === 'about') {
        await expect(page.getByRole('grid', { name: 'Лицензии' })).toBeVisible();
      }
      if (screen === 'elevation') {
        await expect(
          page.getByText('Обычный пользователь (ограниченный режим)'),
        ).toBeVisible();
      }

      await stabilize(page);
      await expect(page).toHaveScreenshot(baselineName(screen, theme));
    });
  }

  test(`shred-preview ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.open('shred');
    await page
      .getByLabel('Файлы (по одному в строке)')
      .fill('C:\\Temp\\visual-secret.bin');
    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('shred-preview', theme));
  });
}
