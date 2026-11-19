import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('inspection screen', () => {
  test('inspection tabs switch with the mouse and the arrow keys', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('inspect');

    const tablist = page.getByRole('tablist', { name: 'Разделы экрана' });
    const metadata = tablist.getByRole('tab', { name: 'Метаданные' });
    const archives = tablist.getByRole('tab', { name: 'Архивы' });
    const unlock = tablist.getByRole('tab', { name: 'Разблокировка' });
    const longPaths = tablist.getByRole('tab', { name: 'Длинные пути' });

    await expect(metadata).toHaveAttribute('aria-selected', 'true');

    await archives.click();
    await expect(archives).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Архивы' })).toBeVisible();

    await archives.press('ArrowRight');
    await expect(unlock).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Разблокировка' })).toBeVisible();

    await unlock.press('ArrowRight');
    await expect(longPaths).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Длинные пути' })).toBeVisible();

    await longPaths.press('ArrowLeft');
    await expect(unlock).toHaveAttribute('aria-selected', 'true');
  });

  test('metadata reads media records and filters by kind and dimensions', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('inspect');

    await page.getByRole('textbox', { name: 'Файл' }).fill('photo');
    await page.getByRole('button', { name: 'Прочитать метаданные' }).click();
    await expect(page.getByText('C:\\Users\\Pictures\\photo.jpg')).toBeVisible();
    await expect(page.getByText('6000').first()).toBeVisible();

    await page.getByLabel('Семейство').selectOption('audio');
    await expect(
      page.getByText(
        'Нажмите «Прочитать метаданные», чтобы извлечь теги из найденных файлов.',
      ),
    ).toBeVisible();

    await page.getByLabel('Семейство').selectOption('all');
    await page.getByLabel('Минимальная ширина').fill('7000');
    await expect(
      page.getByText(
        'Нажмите «Прочитать метаданные», чтобы извлечь теги из найденных файлов.',
      ),
    ).toBeVisible();
    await page.getByLabel('Минимальная ширина').fill('5000');
    await expect(page.getByText('C:\\Users\\Pictures\\photo.jpg')).toBeVisible();
  });

  test('archives page entries, export the list and report a protected archive', async ({
    page,
  }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('inspect');
    await page.getByRole('tab', { name: 'Архивы' }).click();

    const path = page.getByRole('textbox', { name: 'Архив или папка' });
    await path.fill('C:\\Downloads\\archive.zip');
    await page.getByRole('button', { name: 'Открыть' }).click();
    const grid = page.getByRole('grid', { name: 'Архив' });
    await expect(grid.getByText('data/backup.bin')).toBeVisible();
    await expect(page.getByText('Крупнейшие элементы')).toBeVisible();

    await page.getByRole('button', { name: 'Загрузить ещё' }).click();
    await expect(grid.getByText('readme.txt')).toBeVisible();

    await page
      .getByLabel('Файл экспорта (.csv, .json, .xml, .html)')
      .fill('C:\\Temp\\archive-list.csv');
    await page.getByRole('button', { name: 'Экспортировать список' }).click();
    await expect(
      page.getByText('Список экспортирован: C:\\Temp\\archive-list.csv'),
    ).toBeVisible();

    await path.fill('C:\\Downloads\\secret-password.7z');
    await page.getByRole('button', { name: 'Открыть' }).click();
    await expect(page.getByText('Операция не поддерживается.')).toBeVisible();
    await expect(
      page.getByText('archive.unsupported.password: the archive is password-protected'),
    ).toBeVisible();
  });

  test('unlock soft releases, force releases and schedules deletion on reboot', async ({
    page,
  }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('inspect');
    await page.getByRole('tab', { name: 'Разблокировка' }).click();

    const path = page.getByRole('textbox', { name: 'Файл' });
    await path.fill('C:\\Temp\\e2e-soft.bin');
    await page.keyboard.press('Enter');
    await expect(page.getByText('explorer.exe').first()).toBeVisible();
    await expect(page.getByText('критический')).toBeVisible();

    await page.getByRole('button', { name: 'Попросить приложения закрыть файл' }).click();
    const softDialog = page.getByRole('dialog');
    await softDialog
      .getByRole('button', { name: 'Попросить приложения закрыть файл' })
      .click();
    await expect(page.getByText('снято')).toBeVisible();

    await path.fill('C:\\Temp\\e2e-force.bin');
    await page.keyboard.press('Enter');
    await expect(page.getByText('explorer.exe').first()).toBeVisible();
    await page.getByRole('checkbox', { name: 'Процесс: explorer.exe' }).check();
    await page.getByRole('button', { name: 'Завершить удерживающие процессы' }).click();
    const forceDialog = page.getByRole('dialog');
    await forceDialog
      .getByRole('checkbox', { name: 'Я понимаю, что работающие процессы будут завершены' })
      .check();
    await forceDialog
      .getByRole('button', { name: 'Завершить удерживающие процессы' })
      .click();
    await expect(page.getByText('снято')).toBeVisible();

    await path.fill('C:\\Temp\\e2e-reboot.bin');
    await page.keyboard.press('Enter');
    await expect(page.getByText('explorer.exe').first()).toBeVisible();
    await page.getByRole('button', { name: 'Удалить при перезагрузке' }).click();
    const rebootDialog = page.getByRole('dialog');
    await rebootDialog.getByRole('checkbox', { name: 'Удалить до следующего входа' }).check();
    await rebootDialog.getByRole('button', { name: 'Удалить при перезагрузке' }).click();
    await expect(
      page.getByText('Удаление запланировано до следующего входа.'),
    ).toBeVisible();
    await expect(
      page.getByText('Для снятия блокировки требуется перезагрузка.'),
    ).toBeVisible();
  });

  test('unlock requires elevation for protected files', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('inspect');
    await page.getByRole('tab', { name: 'Разблокировка' }).click();

    await page.getByRole('textbox', { name: 'Файл' }).fill('C:\\Temp\\e2e-protected.bin');
    await page.keyboard.press('Enter');
    await expect(page.getByText('explorer.exe').first()).toBeVisible();

    await page.getByRole('checkbox', { name: 'Процесс: explorer.exe' }).check();
    await page.getByRole('button', { name: 'Завершить удерживающие процессы' }).click();
    const dialog = page.getByRole('dialog');
    await dialog
      .getByRole('checkbox', { name: 'Я понимаю, что работающие процессы будут завершены' })
      .check();
    await dialog
      .getByRole('button', { name: 'Завершить удерживающие процессы' })
      .click();

    await expect(
      page.getByText('Для этой операции нужны права администратора.'),
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Запросить повышение прав' })).toBeVisible();
  });
});
