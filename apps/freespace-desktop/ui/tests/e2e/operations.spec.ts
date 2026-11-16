import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('safe operations', () => {
  test('preview, confirm, journal and undo form one path', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('top_files');

    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(page.getByText('crash.dmp', { exact: true })).toBeVisible();
    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await page.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();

    await page.getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' }).check();
    await page.getByRole('button', { name: 'Удалить', exact: true }).click();

    await expect(page.getByText('Ход операции').first()).toBeVisible();
    await expect(page.getByText('завершено').first()).toBeVisible();

    await app.open('journal');
    await expect(page.getByRole('grid', { name: 'Журнал' })).toBeVisible();
    await expect(page.getByText('mock-user').first()).toBeVisible();
    await expect(
      page.getByRole('grid', { name: 'Журнал' }).getByText('Удаление'),
    ).toBeVisible();

    await page.getByRole('button', { name: /Восстановить|Отменить/ }).first().click();
    await expect(page.getByText(/Восстановлено/)).toBeVisible();
  });

  test('permanent deletion needs a separate confirmation', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('top_files');

    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(page.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(page.getByText('crash.dmp', { exact: true })).toBeVisible();
    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await page.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();

    await page.getByRole('radio', { name: /Безвозвратное удаление/ }).check();
    await page
      .getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' })
      .check();
    await expect(page.getByRole('button', { name: 'Удалить', exact: true })).toBeDisabled();

    await page
      .getByRole('checkbox', {
        name: 'Я понимаю, что путь защищён, и подтверждаю действие с повышенными правами',
      })
      .check();
    await expect(page.getByRole('button', { name: 'Удалить', exact: true })).toBeEnabled();
    await page.getByRole('button', { name: 'Отмена' }).click();
    await expect(page.getByRole('dialog')).toBeHidden();
  });
});
