import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import { expectNoViolations } from './axe';

const SCREENS = ['top_files', 'types', 'duplicates', 'cleanup', 'snapshots'] as const;

test.describe('analysis accessibility', () => {
  for (const screen of SCREENS) {
    test(screen, async ({ page }) => {
      await page.goto('/');
      const app = screens(page);
      await app.withAnalysis(screen);
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
      if (screen === 'duplicates') {
        await page.getByRole('button', { name: 'Найти дубликаты' }).click();
        await expect(page.getByText('backup-1.bin')).toBeVisible({ timeout: 15_000 });
      }
      await expectNoViolations(page);
    });
  }

  test('preview dialog', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('top_files');
    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(page.getByText('crash.dmp', { exact: true })).toBeVisible();
    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await page.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expectNoViolations(page);
  });

  test('journal', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('journal');
    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();
    await expectNoViolations(page);
  });
});
