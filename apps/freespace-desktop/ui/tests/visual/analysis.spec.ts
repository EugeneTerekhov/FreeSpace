import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import {
  THEMES,
  applyBoot,
  baselineName,
  stabilize,
  waitForShell,
} from './harness';

const ANALYSIS = ['top_files', 'types', 'duplicates', 'cleanup'] as const;

for (const theme of THEMES) {
  for (const screen of ANALYSIS) {
    test(`${screen} ${theme}`, async ({ page }) => {
      await applyBoot(page, { theme, language: 'ru' });
      await page.goto('/');
      await waitForShell(page);
      const app = screens(page);
      await app.withAnalysis(screen);
      if (screen === 'types') {
        await page.getByRole('button', { name: '.zip' }).click();
      }
      if (screen === 'duplicates') {
        await page.getByRole('button', { name: 'Найти дубликаты' }).click();
        await expect(page.getByText('backup-1.bin')).toBeVisible({ timeout: 15_000 });
      }
      await stabilize(page);
      await expect(page).toHaveScreenshot(baselineName(screen, theme));
    });
  }

  test(`snapshots ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.withAnalysis('snapshots');
    await page.getByRole('button', { name: 'Создать снимок' }).click();
    await page.getByRole('button', { name: 'Создать снимок' }).click();
    await page.getByRole('button', { name: 'Сравнить' }).click();
    await expect(page.getByText('Users\\Downloads\\archive.zip')).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('snapshots', theme));
  });

  test(`operation-preview ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.withAnalysis('top_files');
    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(page.getByText('crash.dmp', { exact: true })).toBeVisible();
    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await page.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('operation-preview', theme));
  });

  test(`operation-progress ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.withAnalysis('top_files');
    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(page.getByText('crash.dmp', { exact: true })).toBeVisible();
    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await page.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    await page.getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' }).check();
    await page.getByRole('button', { name: 'Удалить', exact: true }).click();
    await expect(page.getByText('Ход операции').first()).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('operation-progress', theme));
  });

  test(`journal ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.withAnalysis('top_files');
    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(page.getByText('crash.dmp', { exact: true })).toBeVisible();
    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await page.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await page.getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' }).check();
    await page.getByRole('button', { name: 'Удалить', exact: true }).click();
    await app.open('journal');
    await expect(page.getByRole('grid', { name: 'Журнал' })).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('journal', theme));
  });
}
