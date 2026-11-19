import { expect, test, type Page } from '@playwright/test';

import { screens } from './pages/screens';

async function focusedInsideDialog(page: Page): Promise<boolean> {
  return page.evaluate(() => {
    const dialog = document.querySelector('[role="dialog"]');
    const active = document.activeElement;

    return active !== null && dialog?.contains(active) === true;
  });
}

test.describe('keyboard only', () => {
  test('the skip link is the first tab stop and focuses main', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();

    await page.keyboard.press('Tab');
    const skip = page.getByRole('link', { name: 'Перейти к основному содержимому' });
    await expect(skip).toBeFocused();

    await page.keyboard.press('Enter');
    await expect(page.getByRole('main')).toBeFocused();
  });

  test('sidebar screens activate with Enter and keep aria-current', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();

    const journal = page.locator('[data-screen="journal"]');
    await journal.focus();
    await page.keyboard.press('Enter');

    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();
    await expect(journal).toHaveAttribute('aria-current', 'page');
  });

  test('command palette traps focus and returns it on Escape', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();

    const opener = page.getByRole('button', { name: 'Команды' });
    await opener.click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    const input = page.getByRole('combobox', { name: 'Введите команду…' });
    await expect(input).toBeFocused();

    await page.keyboard.press('Tab');
    await expect(page.getByRole('button', { name: 'Закрыть' })).toBeFocused();
    await page.keyboard.press('Tab');
    await expect(input).toBeFocused();

    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(opener).toBeFocused();
  });

  test('command palette filters and runs a screen from the keyboard', async ({ page }) => {
    await page.goto('/');
    await page.keyboard.press('Control+K');

    const input = page.getByRole('combobox', { name: 'Введите команду…' });
    await expect(input).toBeFocused();
    await input.fill('Журнал');
    await page.keyboard.press('Enter');

    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();
  });

  test('tree grid moves with a roving tabindex and activates rows', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withSnapshot();

    const grid = page.getByRole('treegrid', { name: 'Дерево' });
    await expect(grid).toBeVisible();

    const first = page.locator('[data-row-index="0"]');
    await expect(first).toHaveAttribute('tabindex', '0');
    await first.focus();

    await page.keyboard.press(' ');
    const details = page.locator('dl').filter({ hasText: 'Жёсткие ссылки' });
    await expect(details.getByText('C:\\', { exact: true })).toBeVisible();

    await page.keyboard.press('Enter');
    await expect(page.getByRole('row', { name: /Windows(?!\.)/ })).toBeVisible();

    await page.keyboard.press('ArrowDown');
    const second = page.locator('[data-row-index="1"]');
    await expect(second).toBeFocused();
    await expect(second).toHaveAttribute('tabindex', '0');

    await page.keyboard.press('Home');
    await expect(first).toBeFocused();
  });

  test('operation preview keeps focus inside and restores it', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.withAnalysis('analysis');

    await page.getByRole('searchbox', { name: 'Текст' }).fill('crash');
    await page.getByRole('checkbox', { name: 'Выбрано: crash.dmp' }).check();
    const trigger = page.getByRole('button', { name: 'Удалить выбранные (1)' });
    await trigger.click();

    await expect(page.getByRole('dialog')).toBeVisible();

    for (let index = 0; index < 8; index += 1) {
      await page.keyboard.press('Tab');
      expect(await focusedInsideDialog(page)).toBe(true);
    }

    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toBeHidden();
    await expect(trigger).toBeFocused();
  });

  test('Ctrl+1..6 open the first screen of every group', async ({ page }) => {
    await page.goto('/');

    const groups = [
      { key: '1', screen: 'overview', heading: 'Обзор' },
      { key: '2', screen: 'tree', heading: 'Дерево' },
      { key: '3', screen: 'cleanup', heading: 'Очистка' },
      { key: '4', screen: 'journal', heading: 'Журнал' },
      { key: '5', screen: 'inspect', heading: 'Инспекция' },
      { key: '6', screen: 'settings', heading: 'Настройки' },
    ] as const;

    for (const group of groups) {
      await page.keyboard.press(`Control+${group.key}`);
      await expect(
        page.getByRole('heading', { level: 1, name: group.heading }),
      ).toBeVisible();
      await expect(page.locator(`[data-screen="${group.screen}"]`)).toHaveAttribute(
        'aria-current',
        'page',
      );
    }
  });

  test('shell shortcuts open settings, help, search, status and history', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();

    await page.keyboard.press('Control+,');
    await expect(
      page.getByRole('heading', { level: 1, name: 'Настройки' }),
    ).toBeVisible();

    // F1 opens the reference screen, which is the settings screen now that
    // About is its tab.
    await page.keyboard.press('F1');
    await expect(
      page.getByRole('heading', { level: 1, name: 'Настройки' }),
    ).toBeVisible();

    await page.locator('[data-screen="journal"]').click();
    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();

    await page.keyboard.press('Alt+ArrowLeft');
    await expect(
      page.getByRole('heading', { level: 1, name: 'Настройки' }),
    ).toBeVisible();
    await page.keyboard.press('Alt+ArrowRight');
    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();

    await page.keyboard.press('Control+2');
    await expect(page.getByRole('heading', { level: 1, name: 'Дерево' })).toBeVisible();

    await app.withAnalysis('analysis');
    await page.keyboard.press('Control+F');
    await expect(page.getByRole('searchbox', { name: 'Текст' })).toBeFocused();

    await page.keyboard.press('F6');
    await expect(page.locator('[data-status-bar]')).toBeFocused();
  });
});
