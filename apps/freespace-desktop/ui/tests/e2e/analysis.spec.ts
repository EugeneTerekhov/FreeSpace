import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('analysis screens', () => {
  test('top files filters, opens a preview and keeps one duplicate copy', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('top_files');

    await expect(page.getByRole('grid', { name: 'Крупнейшие файлы' })).toBeVisible();
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeVisible();

    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(page.getByText('crash.dmp', { exact: true })).toBeVisible();

    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await page.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByText(/Объектов: 1/)).toBeVisible();    await page.getByRole('button', { name: 'Отмена' }).click();
    await expect(page.getByRole('dialog')).toBeHidden();
  });

  test('types drills from an extension into files', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('types');

    await page.getByRole('button', { name: '.zip' }).click();
    await expect(page.getByText('archive.zip')).toBeVisible();
    await page.getByRole('radio', { name: 'По категориям' }).click();
    await expect(
      page.getByRole('table', { name: 'Типы файлов' }).getByRole('button', { name: 'Архивы' }),
    ).toBeVisible();
  });

  test('duplicates keeps one copy of every group', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('duplicates');

    await page.getByRole('button', { name: 'Найти дубликаты' }).click();
    await expect(page.getByText('backup-1.bin')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('backup-2.bin')).toBeVisible();

    await page.getByRole('table', { name: 'Дубликаты' }).getByRole('checkbox').first().click();
    await expect(page.getByText('Хотя бы одна копия в каждой группе должна остаться.')).toBeVisible();
  });

  test('cleanup recommends candidates and routes them to the preview', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('cleanup');

    await expect(page.getByText('crash.dmp')).toBeVisible();
    await app.openFirstPreview('Рекомендации');
    await expect(page.getByText(/Объектов: 1/)).toBeVisible();
  });

  test('snapshots compare two states and show growth', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('snapshots');

    await page.getByRole('button', { name: 'Создать снимок' }).click();
    await expect(page.getByText(/Снимок записан/)).toBeVisible();
    await page.getByRole('button', { name: 'Создать снимок' }).click();
    await page.getByRole('button', { name: 'Сравнить' }).click();

    await expect(page.getByText('Users\\Downloads\\archive.zip')).toBeVisible();
    await expect(page.getByText('Прирост по каталогам')).toBeVisible();
  });
});
