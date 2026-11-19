import { expect, test, type Page } from '@playwright/test';

import { screens } from './pages/screens';

const TABS = 'Разделы экрана';

/** Active tab panels by id; hidden panels stay mounted but are not displayed. */
const TOP_PANEL = '#analysis-panel-top';
const TYPES_PANEL = '#analysis-panel-types';
const OWNERS_PANEL = '#analysis-panel-owners';

/** Runs a scan, opens the analysis screen and selects its owners tab. */
async function openOwners(page: Page): Promise<void> {
  const app = screens(page);
  await app.withAnalysis('analysis');
  await page.getByRole('tab', { name: 'Владельцы' }).click();

  const panel = page.locator(OWNERS_PANEL);
  // The toolbar and the empty-state CTA both start the owner report.
  await panel.getByRole('button', { name: 'Построить отчёт по владельцам' }).first().click();
  await expect(panel.getByText('CORP\\alice')).toBeVisible({ timeout: 15_000 });
}

/** Opens the owners report and the global export action it exposes. */
async function openOwnersExport(page: Page): Promise<void> {
  await openOwners(page);
  // The screen head and the owners tab both carry an export trigger; the head
  // one is the global action under test.
  await page.getByRole('button', { name: 'Экспортировать' }).first().click();
  await expect(page.getByRole('dialog')).toBeVisible();
}

test.describe('analysis screens', () => {
  test('analysis tabs switch with the mouse and the arrow keys', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('analysis');

    const tablist = page.getByRole('tablist', { name: TABS });
    await expect(tablist).toBeVisible();

    const top = tablist.getByRole('tab', { name: 'Крупнейшие файлы' });
    const types = tablist.getByRole('tab', { name: 'Типы файлов' });
    const owners = tablist.getByRole('tab', { name: 'Владельцы' });

    await expect(top).toHaveAttribute('aria-selected', 'true');

    await types.click();
    await expect(types).toHaveAttribute('aria-selected', 'true');
    await expect(top).toHaveAttribute('aria-selected', 'false');
    await expect(page.getByRole('tabpanel', { name: 'Типы файлов' })).toBeVisible();

    // Arrow keys move selection and focus together (roving tabindex).
    await types.press('ArrowRight');
    await expect(owners).toHaveAttribute('aria-selected', 'true');
    await expect(owners).toBeFocused();
    await expect(page.getByRole('tabpanel', { name: 'Владельцы' })).toBeVisible();

    await owners.press('ArrowLeft');
    await expect(types).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Типы файлов' })).toBeVisible();
  });

  test('largest files filters and previews a deletion', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('analysis');

    const panel = page.locator(TOP_PANEL);
    await expect(panel.getByRole('grid', { name: 'Крупнейшие файлы' })).toBeVisible();
    await expect(panel.getByText('pagefile.sys', { exact: true })).toBeVisible();

    await panel.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await expect(panel.getByText('pagefile.sys', { exact: true })).toBeHidden();
    await expect(panel.getByText('crash.dmp', { exact: true })).toBeVisible();

    await panel.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    await panel.getByRole('button', { name: 'Удалить выбранные (1)' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/Объектов: 1/)).toBeVisible();
    await dialog.getByRole('button', { name: 'Отмена' }).click();
    await expect(dialog).toBeHidden();
  });

  test('file types drill from an extension into files', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('analysis');
    await page.getByRole('tab', { name: 'Типы файлов' }).click();

    const panel = page.locator(TYPES_PANEL);
    await expect(panel.getByRole('table', { name: 'Типы файлов' })).toBeVisible();
    await panel.getByRole('button', { name: '.zip' }).click();
    await expect(
      panel.getByRole('grid', { name: 'Типы файлов' }).getByText('archive.zip', { exact: true }),
    ).toBeVisible();

    await panel.getByRole('radio', { name: 'По категориям' }).click();
    await expect(
      panel.getByRole('table', { name: 'Типы файлов' }).getByRole('button', { name: 'Архивы' }),
    ).toBeVisible();
  });

  test('owners page, drill into files and expose the export action', async ({ page }) => {
    await page.goto('/');
    await openOwners(page);

    const panel = page.locator(OWNERS_PANEL);
    await expect(panel.getByText('S-1-5-21-1001')).toBeVisible();

    await panel.getByRole('button', { name: 'Загрузить ещё' }).click();
    await expect(panel.getByText('CORP\\dmitri')).toBeVisible();

    await panel.getByRole('button', { name: 'Объекты' }).first().click();
    await expect(
      panel.getByText('C:\\Users\\alice\\Documents\\owned-1.dat'),
    ).toBeVisible();

    // Export is a global action, not a screen: the screen head carries it.
    await page.getByRole('button', { name: 'Экспортировать' }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('checkbox', { name: 'Владельцы' })).toBeChecked();
    await expect(dialog.getByRole('checkbox', { name: 'Дерево' })).not.toBeChecked();
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  });

  test('the export action previews and completes an offline report', async ({ page }) => {
    await page.goto('/');
    await openOwnersExport(page);

    const dialog = page.getByRole('dialog');
    await dialog.getByLabel('Файл отчёта').fill('C:\\Temp\\report.csv');
    await dialog.getByRole('button', { name: 'Предпросмотр' }).click();
    await expect(dialog.getByText('C:\\Temp\\report.csv')).toBeVisible();

    await dialog.getByRole('button', { name: 'Экспортировать', exact: true }).click();
    await expect(dialog.getByText('Отчёт записан: C:\\Temp\\report.csv')).toBeVisible({
      timeout: 15_000,
    });
  });

  test('the export action cancels a running report', async ({ page }) => {
    await page.goto('/');
    await openOwnersExport(page);

    const dialog = page.getByRole('dialog');
    await dialog.getByLabel('Файл отчёта').fill('C:\\Temp\\cancel.csv');
    await dialog.getByRole('button', { name: 'Предпросмотр' }).click();
    await expect(dialog.getByText('C:\\Temp\\cancel.csv')).toBeVisible();

    await dialog.getByRole('button', { name: 'Экспортировать', exact: true }).click();
    await dialog.getByRole('button', { name: 'Отменить операцию' }).click();
    await expect(dialog.getByText('Отменено').first()).toBeVisible({ timeout: 15_000 });
  });

  test('duplicates keeps one copy of every group', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('duplicates');

    await page.getByRole('button', { name: 'Найти дубликаты' }).click();
    await expect(page.getByText('backup-1.bin')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('backup-2.bin')).toBeVisible();

    await page.getByRole('table', { name: 'Дубликаты' }).getByRole('checkbox').first().click();
    await expect(
      page.getByText('Хотя бы одна копия в каждой группе должна остаться.'),
    ).toBeVisible();
  });

  test('cleanup recommends candidates and routes them to the preview', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('cleanup');

    await expect(page.getByText('crash.dmp')).toBeVisible();
    await app.openFirstPreview('Рекомендации');
    await expect(page.getByText(/Объектов: 1/)).toBeVisible();
  });

  test('snapshots live in the cleanup tab and compare two states', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('cleanup');

    const snapshots = page.getByRole('tab', { name: 'Снимки' });
    await snapshots.click();
    await expect(snapshots).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('tabpanel', { name: 'Снимки' })).toBeVisible();

    await page.getByRole('button', { name: 'Создать снимок' }).click();
    await expect(page.getByText(/Снимок записан/)).toBeVisible();
    await page.getByRole('button', { name: 'Создать снимок' }).click();
    await page.getByRole('button', { name: 'Сравнить' }).click();

    await expect(page.getByText('Users\\Downloads\\archive.zip')).toBeVisible();
    await expect(page.getByText('Прирост по каталогам')).toBeVisible();
  });
});
