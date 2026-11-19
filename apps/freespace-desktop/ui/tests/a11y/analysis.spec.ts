import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import { expectNoViolations } from './axe';

/**
 * Screens whose absorbed capabilities live in tabs. Every tab is audited on its
 * own because the active panel changes the rendered content.
 */
const TABBED = [
  { screen: 'analysis', tabs: ['Крупнейшие файлы', 'Типы файлов', 'Владельцы'] },
  { screen: 'cleanup', tabs: ['Очистка', 'Снимки'] },
  { screen: 'inspect', tabs: ['Метаданные', 'Архивы', 'Разблокировка', 'Длинные пути'] },
  { screen: 'fileops', tabs: ['Переименование', 'Права доступа'] },
  { screen: 'settings', tabs: ['Настройки', 'Права доступа', 'О программе'] },
] as const;

test.describe('analysis and absorbed tabs accessibility', () => {
  for (const entry of TABBED) {
    test(`${entry.screen} tabs`, async ({ page }) => {
      await page.goto('/');
      const app = screens(page);

      if (entry.screen === 'settings') {
        await app.open('settings');
      } else {
        await app.withAnalysis(entry.screen);
      }

      for (const label of entry.tabs) {
        const tab = page.getByRole('tab', { name: label, exact: true });
        await tab.click();
        await expect(tab).toHaveAttribute('aria-selected', 'true');
        await expect(page.getByRole('tabpanel', { name: label })).toBeVisible();
        await expectNoViolations(page);
      }
    });
  }

  test('duplicates with search results', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('duplicates');

    await page.getByRole('button', { name: 'Найти дубликаты' }).click();
    await expect(page.getByText('backup-1.bin')).toBeVisible({ timeout: 15_000 });
    await expectNoViolations(page);
  });

  test('operation preview dialog', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('analysis');

    const panel = page.locator('#analysis-panel-top');
    await panel.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(panel.getByText('crash.dmp', { exact: true })).toBeVisible();
    await panel.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await panel.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
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
