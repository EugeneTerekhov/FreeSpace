import { expect, test } from '@playwright/test';

import { screens } from '../e2e/pages/screens';
import { THEMES, applyBoot, baselineName, stabilize, waitForShell } from './harness';

for (const theme of THEMES) {
  test(`size-details ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.runScan();
    await app.open('size_details');
    await page.getByLabel('Объект').fill('C:\\Windows\\explorer.exe');
    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(page.getByText('Логический размер')).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('size-details', theme));
  });

  test(`long-paths ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.runScan();
    await app.open('long_paths');
    const row = page.getByRole('row', { name: /long-path-01-/ }).first();
    await expect(row).toBeVisible();
    await row.click();
    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(page.getByText(/Готово/).first()).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('long-paths', theme));
  });

  test(`rename ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.runScan();
    await app.open('rename');
    await page
      .getByLabel('Объекты (по одному в строке)')
      .fill('C:\\Users\\Downloads\\archive.zip\nC:\\Users\\Downloads\\backup-1.bin');
    await page.getByLabel('Шаблон').fill('archive|backup');
    await page.getByLabel('Замена').fill('media');
    await page.getByRole('button', { name: 'Предпросмотр' }).click();
    await expect(page.getByText(/Планируется переименований: 2, конфликтов: 0/)).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('rename', theme));
  });

  test(`permissions ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.runScan();
    await app.open('permissions');
    await page.getByLabel('Объект').fill('C:\\Users\\Documents');
    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(page.getByText('DOMAIN\\alex')).toBeVisible();
    await page.getByRole('switch', { name: 'Скрытый' }).click();
    await page.getByRole('button', { name: 'Предпросмотр' }).click();
    await expect(page.getByText(/Снять.*Установить/)).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('permissions', theme));
  });

  test(`filesystem-journal ${theme}`, async ({ page }) => {
    await applyBoot(page, { theme, language: 'ru' });
    await page.goto('/');
    await waitForShell(page);
    const app = screens(page);
    await app.runScan();
    await app.open('long_paths');
    const row = page.getByRole('row', { name: /long-path-02-/ }).first();
    await expect(row).toBeVisible();
    await row.click();
    await page.getByRole('button', { name: 'Удалить' }).click();
    const dialog = page.getByRole('dialog');
    await dialog
      .getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' })
      .check();
    await dialog.getByRole('button', { name: 'Удалить', exact: true }).click();
    await expect(page.getByText('Ход операции').first()).toBeVisible();
    await app.open('journal');
    await expect(page.getByRole('grid', { name: 'Журнал' })).toBeVisible();
    await stabilize(page);
    await expect(page).toHaveScreenshot(baselineName('filesystem-journal', theme));
  });
}
