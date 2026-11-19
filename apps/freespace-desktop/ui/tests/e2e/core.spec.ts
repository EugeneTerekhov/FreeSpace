import { expect, test } from '@playwright/test';

import { SCREEN_IDS, screens } from './pages/screens';

/**
 * The analyze button is the only control that starts the embedded scan from a
 * volume card. The accessible name is matched loosely so the spec does not
 * depend on the exact wording of the dashboard copy.
 */
const ANALYZE = /анализировать/i;

test.describe('core screens', () => {
  test('overview lists volumes and shows the embedded scan progress', async ({ page }) => {
    await page.goto('/');

    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();

    const volumes = page.getByRole('region', { name: 'Тома' });
    await expect(volumes).toBeVisible();
    await expect(volumes.getByRole('heading', { level: 2 }).first()).toBeVisible();
    await expect(volumes.getByText('C:\\', { exact: true }).first()).toBeVisible();
    await expect(volumes.getByRole('button', { name: ANALYZE }).first()).toBeEnabled();

    await volumes.getByRole('button', { name: ANALYZE }).first().click();

    // The scan no longer has a screen of its own: progress, the current path
    // and the completed banner live inside the overview.
    await expect(page.getByText('Сканирование завершено')).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByRole('heading', { level: 2, name: 'Текущий анализ' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Открыть дерево' })).toBeVisible();
    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();
  });

  test('the embedded scan opens the tree', async ({ page }) => {
    await page.goto('/');

    await screens(page).runScan();
    await page.getByRole('button', { name: 'Открыть дерево' }).click();

    await expect(page.getByRole('heading', { level: 1, name: 'Дерево' })).toBeVisible();
    await expect(page.getByRole('treegrid', { name: 'Дерево' })).toBeVisible();
    await expect(page.locator('[data-screen="tree"]')).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  test('tree drills into a directory and shows details', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot();

    await expect(page.getByRole('treegrid', { name: 'Дерево' })).toBeVisible();
    await expect(page.getByRole('row', { name: /C:\\/ }).first()).toBeVisible();

    await app.openTreeRoot();
    await expect(page.getByRole('row', { name: /Windows(?!\.)/ })).toBeVisible();

    await page.getByRole('row', { name: /Windows(?!\.)/ }).click();
    await expect(page.getByText('Размер на диске', { exact: true })).toBeVisible();
    await expect(page.getByText('Каталог').first()).toBeVisible();
  });

  test('tree search finds a nested object', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot();

    await page.getByRole('searchbox', { name: 'Найти' }).fill('explorer');
    await page.getByRole('button', { name: 'Найти' }).click();
    await expect(page.getByRole('row', { name: /explorer.exe/ })).toBeVisible();
  });

  test('every screen is reachable from the sidebar', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);

    for (const id of SCREEN_IDS) {
      await app.open(id);
      await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
      await expect(page.locator(`[data-screen="${id}"]`)).toHaveAttribute(
        'aria-current',
        'page',
      );
    }
  });
});
