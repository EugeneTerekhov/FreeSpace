import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('filesystem screens', () => {
  test('the tree shows the size details of a row on hover', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot();
    await app.openTreeRoot();

    const row = page.getByRole('row', { name: /Windows(?!\.)/ });
    await expect(row).toBeVisible();
    await row.hover();

    // "Size details" is no longer a screen or a side panel: a dense card opens
    // over the hovered row after a short delay.
    const card = page.getByRole('tooltip', { name: 'Детали размера' });
    await expect(card).toBeVisible();
    await expect(card.getByText('Логический размер')).toBeVisible();
    await expect(card.getByText('Размер на диске')).toBeVisible();
    await expect(card.getByText('Жёсткие ссылки')).toBeVisible();
    await expect(card.getByText('Потоки ADS')).toBeVisible();
    await expect(card.getByText('C:\\Windows', { exact: true })).toBeVisible();
    await expect(card.getByText('Каталог')).toBeVisible();
  });

  test('long paths validate, preview and confirm a delete from the inspect tab', async ({
    page,
  }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('inspect');

    const tab = page.getByRole('tab', { name: 'Длинные пути' });
    await tab.click();
    await expect(tab).toHaveAttribute('aria-selected', 'true');

    await expect(page.getByRole('grid', { name: 'Длинные пути' })).toBeVisible();
    const row = page.getByRole('row', { name: /long-path-01-/ }).first();
    await expect(row).toBeVisible();
    await row.click();
    await expect(page.getByLabel('Объект')).toHaveValue(/long-path-01-/);

    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(page.getByText(/Готово/).first()).toBeVisible();

    const grid = page.getByRole('grid', { name: 'Длинные пути' });
    await expect(grid).toHaveAttribute('aria-rowcount', '50');
    await page.getByRole('button', { name: 'Загрузить ещё' }).click();
    await expect(grid).toHaveAttribute('aria-rowcount', '60');

    await page.getByLabel('Новое имя').fill('long-path-01-renamed.pdf');
    await page.getByRole('button', { name: 'Переименовать' }).click();
    let dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Отмена' }).click();
    await expect(dialog).toBeHidden();

    await page.getByLabel('Целевая папка').fill('D:\\Archive');
    await page.getByRole('button', { name: 'Переместить' }).click();
    dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Отмена' }).click();
    await expect(dialog).toBeHidden();

    await page.getByRole('button', { name: 'Удалить' }).click();
    dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    const confirm = dialog.getByRole('button', { name: 'Удалить', exact: true });
    await expect(confirm).toBeDisabled();
    await dialog
      .getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' })
      .check();
    await expect(confirm).toBeEnabled();
    await confirm.click();

    await expect(page.getByText('Ход операции').first()).toBeVisible();
    await expect(page.getByText('завершено').first()).toBeVisible();
  });

});
