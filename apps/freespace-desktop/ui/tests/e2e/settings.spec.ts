import { expect, test } from '@playwright/test';

import { screens } from './pages/screens';

test.describe('settings and US6 screens', () => {
  test('applies preferences, validates paths and saves the legacy settings', async ({
    page,
  }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('settings');

    await expect(page.getByLabel('Язык')).toBeVisible();

    const theme = page.getByRole('radiogroup', { name: 'Тема' });
    await theme.getByRole('radio').nth(1).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
    await theme.getByRole('radio').nth(2).click();
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');

    const density = page.getByRole('radiogroup').nth(1);
    await density.getByRole('radio').nth(1).click();
    await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');

    const exclusions = page.locator('#settings-exclusions');
    await exclusions.fill('relative\\folder');
    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(
      page.getByText('Проверьте введённые значения.').first(),
    ).toBeVisible();

    await exclusions.fill('C:\\Temp; **\\node_modules');
    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(page.getByRole('main').getByText('Готово')).toBeVisible();

    const days = page.locator('#settings-outdated-days');
    await expect(page.getByRole('button', { name: 'Сохранить' })).toBeEnabled();
    await days.fill('');
    await expect(page.getByRole('button', { name: 'Сохранить' })).toBeDisabled();
    await days.fill('400');
    await page.getByRole('button', { name: 'Сохранить' }).click();
    await expect(page.getByText('Настройки сохранены')).toBeVisible();

    await page.getByRole('button', { name: 'Сбросить' }).click();
    await expect(page.locator('html')).toHaveAttribute('data-density', 'comfortable');
    await expect(days).toHaveValue('400');

    await page.locator('#settings-language').selectOption('en');
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
    await page.locator('#settings-language').selectOption('ru');
    await expect(page.locator('html')).toHaveAttribute('lang', 'ru');
  });

  test('shredder requires a preview and an explicit acknowledgement', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('shred');

    await expect(page.getByRole('button', { name: 'Проверить' })).toBeDisabled();
    await expect(page.getByText(/SSD/)).toBeVisible();

    await page
      .getByLabel('Файлы (по одному в строке)')
      .fill('C:\\Temp\\e2e-secret.bin');
    await page.getByRole('button', { name: 'Проверить' }).click();
    await expect(page.getByRole('dialog')).toBeVisible();

    const destroy = page.getByRole('button', { name: 'Уничтожить' });
    await expect(destroy).toBeDisabled();
    await page
      .getByRole('checkbox', {
        name: 'Я понимаю, что эти файлы будут уничтожены безвозвратно',
      })
      .check();
    await expect(destroy).toBeEnabled();
    await destroy.click();

    await expect(page.getByText('Ход операции').first()).toBeVisible();
    await expect(page.getByText('Уничтожено файлов: 1')).toBeVisible({
      timeout: 10_000,
    });
  });

  test('monitor starts, filters by process and stops', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('monitor');

    await expect(
      page.getByText('За выбранный период записи не обнаружены.'),
    ).toBeVisible();

    await page.getByRole('button', { name: 'Запустить мониторинг' }).click();
    await expect(page.getByText('explorer.exe')).toBeVisible();
    await expect(page.getByText(/Источник событий: ETW/)).toBeVisible();

    await page.getByRole('searchbox', { name: 'Процесс' }).fill('backup');
    await expect(page.getByText('backup-agent.exe')).toBeVisible();
    await expect(page.getByText('explorer.exe')).toBeHidden();

    await page.getByRole('button', { name: 'Остановить мониторинг' }).click();
    await expect(page.getByText('Мониторинг завершён')).toBeVisible();
  });

  test('elevation explains the reason and requests administrator rights', async ({
    page,
  }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('elevation');

    await expect(
      page.getByText('Обычный пользователь (ограниченный режим)'),
    ).toBeVisible();
    await expect(page.getByText('Доступен')).toBeVisible();

    await page.getByRole('button', { name: 'Запросить повышение прав' }).click();

    await expect(page.getByText('Администратор', { exact: true })).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Запросить повышение прав' }),
    ).toBeDisabled();
  });

  test('About shows build data, licences and never opens a link', async ({ page }) => {
    await page.goto('/');
    const app = screens(page);
    await app.open('about');

    const main = page.getByRole('main');
    await expect(main.getByText('1.1.0')).toBeVisible();
    await expect(main.getByText('mock-002')).toBeVisible();
    await expect(main.getByText('130.0.2849.68')).toBeVisible();
    await expect(main.getByText('C:\\FreeSpace\\data')).toBeVisible();
    await expect(page.getByRole('grid', { name: 'Лицензии' })).toBeVisible();
    await expect(main.locator('a')).toHaveCount(0);
  });
});
