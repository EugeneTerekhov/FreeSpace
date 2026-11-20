import { expect, test } from '@playwright/test';

import { SCREEN_IDS, screens } from './pages/screens';

/**
 * A volume block is the selection control; the selection bar starts the
 * embedded scan. The accessible names are matched loosely so the spec does not
 * depend on the exact wording of the dashboard copy.
 */
const SELECT = /^C\b/;
const ANALYZE = /^анализировать$/i;

test.describe('core screens', () => {
  test('overview lists volumes and shows the embedded scan progress', async ({ page }) => {
    await page.goto('/');

    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();

    const volumes = page.getByRole('region', { name: 'Тома' });
    await expect(volumes).toBeVisible();
    await expect(volumes.getByRole('heading', { level: 2 }).first()).toBeVisible();
    await expect(volumes.getByText('C', { exact: true }).first()).toBeVisible();
    await expect(volumes.getByRole('button', { name: SELECT }).first()).toBeEnabled();

    await volumes.getByRole('button', { name: SELECT }).first().click();
    await page.getByRole('button', { name: ANALYZE }).first().click();

    // The scan no longer has a screen of its own: progress, the current path
    // and the completed banner live inside the overview.
    await expect(page.getByText('Сканирование завершено')).toBeVisible({
      timeout: 15_000,
    });
    await expect(page.getByRole('heading', { level: 2, name: 'Текущий анализ' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Открыть дерево' })).toBeVisible();
    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();
  });

  test('keeps the previous volume when volumes are scanned one by one', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();

    // Select the second volume on its own: the current snapshot still holds C:.
    await page.getByRole('button', { name: 'Снять выбор' }).click();
    await page.getByRole('button', { name: /^D\b/ }).click();
    await page.getByRole('button', { name: ANALYZE }).click();
    await expect(page.getByText('Сканирование завершено')).toBeVisible({
      timeout: 15_000,
    });

    await page.getByRole('button', { name: 'Открыть дерево' }).click();
    await expect(page.getByRole('treegrid', { name: 'Дерево' })).toBeVisible();
    await expect(page.getByRole('row', { name: /C:\\/ }).first()).toBeVisible();
    await expect(page.getByRole('row', { name: /D:\\/ }).first()).toBeVisible();
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
    const row = page.getByRole('row', { name: /Windows(?!\.)/ });
    await expect(row).toBeVisible();
    await row.hover();

    const details = page.getByRole('tooltip', { name: 'Детали размера' });
    await expect(details).toBeVisible();
    await expect(details.getByText('Размер на диске', { exact: true })).toBeVisible();
    await expect(details.getByText('Каталог')).toBeVisible();
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
