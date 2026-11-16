import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('core screens', () => {
  test('dashboard lists volumes and starts a quick scan', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);

    await expect(page.getByRole('heading', { level: 1, name: 'Диски' })).toBeVisible();
    await expect(page.getByText('C:\\', { exact: true }).first()).toBeVisible();
    await expect(page.getByRole('button', { name: 'Анализировать этот том' }).first()).toBeVisible();

    await page.getByRole('button', { name: 'Анализировать этот том' }).first().click();
    await expect(page.getByRole('heading', { level: 1, name: 'Сканирование' })).toBeVisible();
    await expect(page.getByText('Сканирование завершено')).toBeVisible({
      timeout: 15_000,
    });
    expect(await app.heading()).toBe('Сканирование');
  });

  test('tree drills into a directory and shows details', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot('tree');

    await expect(page.getByRole('treegrid', { name: 'Дерево' })).toBeVisible();
    await expect(page.getByRole('row', { name: /C:\\/ }).first()).toBeVisible();

    await app.openTreeRoot();
    await expect(page.getByRole('row', { name: /Windows/ })).toBeVisible();

    await page.getByRole('row', { name: /Windows/ }).click();
    await expect(page.getByText('Размер на диске', { exact: true })).toBeVisible();
    await expect(page.getByText('Каталог').first()).toBeVisible();
  });

  test('tree search finds a nested object', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot('tree');

    await page.getByRole('searchbox', { name: 'Найти' }).fill('explorer');
    await page.getByRole('button', { name: 'Найти' }).click();
    await expect(page.getByRole('row', { name: /explorer.exe/ })).toBeVisible();
  });

  test('treemap renders tiles and selects one', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot('treemap');

    const canvas = page.getByRole('img', { name: /Щёлкните плитку/ });
    await expect(canvas).toBeVisible();
    await canvas.click({ position: { x: 200, y: 120 } });
    await expect(page.getByText('Размер на диске')).toBeVisible();
  });

  test('command palette navigates by keyboard', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1, name: 'Диски' })).toBeVisible();
    await page.keyboard.press('Control+K');

    const palette = page.getByRole('dialog', { name: 'Command palette' });
    await expect(palette).toBeVisible();
    await page.getByRole('combobox', { name: 'Введите команду…' }).fill('Карта');
    await page.keyboard.press('Enter');

    await expect(page.getByRole('heading', { level: 1, name: 'Карта' })).toBeVisible();
  });

  test('every screen is reachable from the sidebar', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);

    for (const id of ['journal', 'settings', 'about', 'unlock', 'export'] as const) {
      await app.open(id);
      await expect(page.locator(`[data-screen="${id}"]`)).toHaveAttribute(
        'aria-current',
        'page',
      );
    }
  });
});
