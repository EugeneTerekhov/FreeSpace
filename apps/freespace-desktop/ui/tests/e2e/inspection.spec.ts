import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('metadata screen', () => {
  test('reads media records and filters by kind and dimensions', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();
    await app.open('metadata');

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
});

test.describe('owners screen', () => {
  test('pages owners, drills into files and opens the export action', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();
    await app.open('owners');

    await page.getByRole('button', { name: 'Построить отчёт по владельцам' }).click();
    await expect(page.getByText('CORP\\alice')).toBeVisible();
    await expect(page.getByText('S-1-5-21-1001')).toBeVisible();

    await page.getByRole('button', { name: 'Загрузить ещё' }).click();
    await expect(page.getByText('CORP\\dmitri')).toBeVisible();

    await page.getByRole('button', { name: 'Объекты' }).first().click();
    await expect(
      page.getByText('C:\\Users\\alice\\Documents\\owned-1.dat'),
    ).toBeVisible();

    await page.getByRole('button', { name: 'Экспортировать' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('checkbox', { name: 'Владельцы' })).toBeChecked();
    await expect(dialog.getByRole('checkbox', { name: 'Дерево' })).not.toBeChecked();
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  });
});

test.describe('archives screen', () => {
  test('pages archive entries, exports the list and reports a protected archive', async ({
    page,
  }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('archives');

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
});

test.describe('export screen', () => {
  test('previews and completes an offline report', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();
    await app.open('export');

    await page.getByRole('checkbox', { name: 'Типы' }).uncheck();
    await page.getByLabel('Файл отчёта').fill('C:\\Temp\\report.csv');
    await page.getByRole('button', { name: 'Предпросмотр' }).click();
    await expect(page.getByText('C:\\Temp\\report.csv')).toBeVisible();

    await page.getByRole('button', { name: 'Экспортировать', exact: true }).click();
    await expect(page.getByText('Отчёт записан: C:\\Temp\\report.csv')).toBeVisible({
      timeout: 15_000,
    });
  });

  test('cancels a running report', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.runScan();
    await app.open('export');

    await page.getByLabel('Файл отчёта').fill('C:\\Temp\\cancel.csv');
    await page.getByRole('button', { name: 'Предпросмотр' }).click();
    await expect(page.getByText('C:\\Temp\\cancel.csv')).toBeVisible();

    await page.getByRole('button', { name: 'Экспортировать', exact: true }).click();
    await page.getByRole('button', { name: 'Отменить операцию' }).click();
    await expect(page.getByText('Отменено').first()).toBeVisible({ timeout: 15_000 });
  });
});

test.describe('unlock screen', () => {
  test('soft releases, force releases and schedules deletion on reboot', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('unlock');

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

  test('requires elevation for protected files', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('unlock');

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
