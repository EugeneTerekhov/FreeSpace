import { expect, test } from '@playwright/test';

import { SCREEN_IDS, screens } from '../e2e/pages/screens';
import { expectNoViolations } from './axe';

test.describe('accessibility', () => {
  test('overview', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();
    await expect(page.getByRole('region', { name: 'Тома' })).toBeVisible();
    await expectNoViolations(page);
  });

  test('overview with the embedded scan progress', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();

    await expect(page.getByRole('heading', { level: 2, name: 'Текущий анализ' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Открыть дерево' })).toBeVisible();
    await expectNoViolations(page);
  });

  test('tree', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot();

    await expect(page.getByRole('treegrid', { name: 'Дерево' })).toBeVisible();
    await expectNoViolations(page);
  });

  test('command palette', async ({ page }) => {
    await page.goto('/');
    await page.keyboard.press('Control+K');

    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByRole('dialog').getByRole('option')).toHaveCount(
      SCREEN_IDS.length,
    );
    await expectNoViolations(page);
  });
});
