import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import { expectNoViolations } from './axe';

test.describe('accessibility', () => {
  test('dashboard', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
    await expectNoViolations(page);
  });

  test('scan', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('scan');
    await expect(page.getByRole('heading', { level: 1, name: 'Сканирование' })).toBeVisible();
    await expectNoViolations(page);
  });

  test('tree', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot('tree');
    await expect(page.getByRole('treegrid', { name: 'Дерево' })).toBeVisible();
    await expectNoViolations(page);
  });

  test('treemap', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot('treemap');
    await expect(page.getByRole('img', { name: /Щёлкните плитку/ })).toBeVisible();
    await expectNoViolations(page);
  });
});
