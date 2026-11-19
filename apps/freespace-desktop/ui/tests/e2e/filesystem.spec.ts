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

  test('file operation tabs keep rename and permissions reachable', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('fileops');

    const tablist = page.getByRole('tablist', { name: 'Разделы экрана' });
    const rename = tablist.getByRole('tab', { name: 'Переименование' });
    const permissions = tablist.getByRole('tab', { name: 'Права доступа' });

    await expect(rename).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Переименование' })).toBeVisible();

    await permissions.click();
    await expect(permissions).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Права доступа' })).toBeVisible();

    await permissions.press('ArrowLeft');
    await expect(rename).toHaveAttribute('aria-selected', 'true');
  });

  test('rename previews old and new names and applies once', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('fileops');

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
    await app.withAnalysis('fileops');
    await page.getByRole('tab', { name: 'Права доступа' }).click();

    await page.getByRole('textbox', { name: 'Объект', exact: true }).fill('C:\\Users\\Documents');
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
});
