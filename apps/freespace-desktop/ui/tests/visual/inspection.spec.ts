import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import { THEMES, applyBoot, baselineName, stabilize, waitForShell } from './harness';

const INSPECTION = ['metadata', 'owners', 'archives', 'export', 'unlock'] as const;

for (const theme of THEMES) {
  for (const screen of INSPECTION) {
    test(`${screen} ${theme}`, async ({ page }) => {
      await applyBoot(page, { theme, language: 'ru' });
      await page.goto('/');
      await waitForShell(page);
      const app = screens(page);

      if (screen === 'metadata') {
        await app.runScan();
        await app.open('metadata');
        await page.getByRole('textbox', { name: 'Файл' }).fill('photo');
        await page.getByRole('button', { name: 'Прочитать метаданные' }).click();
        await expect(page.getByText('C:\\Users\\Pictures\\photo.jpg')).toBeVisible();
      } else if (screen === 'owners') {
        await app.runScan();
        await app.open('owners');
        await page.getByRole('button', { name: 'Построить отчёт по владельцам' }).click();
        await expect(page.getByText('CORP\\alice')).toBeVisible();
      } else if (screen === 'archives') {
        await app.open('archives');
        await page
          .getByRole('textbox', { name: 'Архив или папка' })
          .fill('C:\\Downloads\\archive.zip');
        await page.getByRole('button', { name: 'Открыть' }).click();
        await expect(
          page.getByRole('grid', { name: 'Архив' }).getByText('data/backup.bin'),
        ).toBeVisible();
      } else if (screen === 'export') {
        await app.runScan();
        await app.open('export');
        await page.getByLabel('Файл отчёта').fill('C:\\Temp\\report.csv');
        await page.getByRole('button', { name: 'Предпросмотр' }).click();
        await expect(page.getByText('C:\\Temp\\report.csv')).toBeVisible();
      } else {
        await app.open('unlock');
        await page
          .getByRole('textbox', { name: 'Файл' })
          .fill('C:\\Temp\\visual-locked.bin');
        await page.keyboard.press('Enter');
        await expect(page.getByText('explorer.exe').first()).toBeVisible();
      }

      await stabilize(page);
      await expect(page).toHaveScreenshot(baselineName(screen, theme));
    });
  }
}
