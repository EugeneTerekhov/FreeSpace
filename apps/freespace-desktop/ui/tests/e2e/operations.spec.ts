import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('safe operations', () => {
  const TOP_PANEL = '#analysis-panel-top';

  test('preview, confirm, journal, filter and undo form one path', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('analysis');

    const panel = page.locator(TOP_PANEL);
    await panel.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(panel.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(panel.getByText('crash.dmp', { exact: true })).toBeVisible();
    await panel.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await panel.getByRole('button', { name: 'Удалить выбранные (1)' }).click();

    let dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog
      .getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' })
      .check();
    await dialog.getByRole('button', { name: 'Удалить', exact: true }).click();

    await expect(page.getByText('Ход операции').first()).toBeVisible();
    await expect(page.getByText('завершено').first()).toBeVisible();

    await app.open('journal');
    const journal = page.getByRole('grid', { name: 'Журнал' });
    await expect(journal).toBeVisible();
    await expect(page.getByText('mock-user').first()).toBeVisible();
    await expect(journal.getByText('Удаление')).toBeVisible();

    // Filters were part of the old filesystem suite; they stay with the journal.
    await page.getByRole('combobox', { name: 'Операция' }).selectOption('delete');
    await expect(journal.getByText('Удаление')).toBeVisible();
    await page.getByRole('combobox', { name: 'Результат' }).selectOption('success');
    await expect(journal).toBeVisible();
    await page.getByRole('button', { name: 'Сбросить' }).first().click();

    // Undo/restore stay on the entries panel: a selected row exposes them as
    // header actions and the confirmation dialog runs before the call.
    await journal.getByRole('row', { name: /Удаление/ }).first().click();
    const action = page.getByRole('button', { name: /^(Восстановить|Отменить)$/ });
    await expect(action).toBeVisible();
    const actionLabel = (await action.textContent())?.trim() ?? 'Отменить';
    await action.click();

    dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: actionLabel, exact: true }).click();
    await expect(page.getByText(/Восстановлено|Операция отменена/)).toBeVisible();
  });

  test('permanent deletion needs a separate confirmation', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('analysis');

    const panel = page.locator(TOP_PANEL);
    await panel.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(panel.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(panel.getByText('crash.dmp', { exact: true })).toBeVisible();
    await panel.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await panel.getByRole('button', { name: 'Удалить выбранные (1)' }).click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByRole('radio', { name: /Безвозвратное удаление/ }).check();
    await dialog
      .getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' })
      .check();
    await expect(dialog.getByRole('button', { name: 'Удалить', exact: true })).toBeDisabled();

    await dialog
      .getByRole('checkbox', {
        name: 'Я понимаю, что путь защищён, и подтверждаю действие с повышенными правами',
      })
      .check();
    await expect(dialog.getByRole('button', { name: 'Удалить', exact: true })).toBeEnabled();
    await dialog.getByRole('button', { name: 'Отмена' }).click();
    await expect(dialog).toBeHidden();
  });
});
