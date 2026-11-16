import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('filesystem screens', () => {
  test('size details shows exact metrics for a selected path', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();
    await app.open('size_details');

    await page.getByLabel('Объект').fill('C:\\Windows\\explorer.exe');
    await page.getByRole('button', { name: 'Проверить' }).click();

    await expect(page.getByText('Логический размер')).toBeVisible();
    await expect(page.getByText('Размер на диске').first()).toBeVisible();
    await expect(page.getByText('Физический размер')).toBeVisible();
    await expect(page.getByText('жёсткая ссылка')).toBeVisible();
    await expect(page.getByText('Альтернативные потоки').first()).toBeVisible();
    await expect(
      page.getByText('C:\\Windows\\WinSxS\\explorer.exe', { exact: true }),
    ).toBeVisible();
  });

  test('long paths validates, previews and confirms a delete', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();
    await app.open('long_paths');

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

  test('rename previews old and new names and applies once', async ({ page }) => {
    await page.goto('/');
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
    await expect(page.getByText('C:\\Users\\Downloads\\media.zip')).toBeVisible();
    await expect(page.getByText('C:\\Users\\Downloads\\media-1.bin')).toBeVisible();

    const apply = page.getByRole('button', { name: 'Переименовать' });
    await expect(apply).toBeDisabled();
    await page
      .getByRole('checkbox', {
        name: 'Я просмотрел предпросмотр и подтверждаю переименование',
      })
      .check();
    await expect(apply).toBeEnabled();
    await apply.click();

    await expect(page.getByText('Переименовано объектов: 2')).toBeVisible();
  });

  test('permissions previews an attribute diff and applies it', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();
    await app.open('permissions');

    await page.getByLabel('Объект').fill('C:\\Users\\Documents');
    await page.getByRole('button', { name: 'Проверить' }).click();

    await expect(page.getByText('DOMAIN\\alex')).toBeVisible();
    await expect(
      page.getByText('S-1-5-21-1004336348-1177238915-682003330-1001'),
    ).toBeVisible();
    await page.getByRole('switch', { name: 'Скрытый' }).click();
    await page.getByRole('button', { name: 'Предпросмотр' }).click();
    await expect(page.getByText(/Снять.*Установить/)).toBeVisible();

    const save = page.getByRole('button', { name: 'Сохранить' });
    await expect(save).toBeDisabled();
    await page
      .getByRole('checkbox', {
        name: 'Я понимаю, что изменения затрагивают доступ и могут требовать прав администратора',
      })
      .check();
    await expect(save).toBeEnabled();
    await save.click();

    await expect(page.getByText('Обновлено: 1, ошибок: 0')).toBeVisible();
  });

  test('journal lists the operation, filters it and offers undo', async ({ page }) => {
    await page.goto('/');
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
    await expect(page.getByText('mock-user').first()).toBeVisible();
    await expect(page.getByRole('grid', { name: 'Журнал' }).getByText('Удаление')).toBeVisible();

    await page.getByRole('combobox', { name: 'Операция' }).selectOption('delete');
    await expect(page.getByRole('grid', { name: 'Журнал' }).getByText('Удаление')).toBeVisible();
    await page.getByRole('combobox', { name: 'Результат' }).selectOption('success');
    await expect(page.getByRole('grid', { name: 'Журнал' })).toBeVisible();

    await page.getByRole('button', { name: 'Сбросить' }).click();
    await page.getByRole('button', { name: /Восстановить|Отменить/ }).first().click();
    await expect(page.getByText(/Восстановлено/)).toBeVisible();
  });
});
