import { expect, test, type Page } from '@playwright/test';

import { SCREEN_GROUPS, SCREEN_IDS } from './pages/screens';

/** Visible group headings, in plan order (plan.md, «UX-перекомпоновка»). */
const GROUP_LABELS: Record<(typeof SCREEN_GROUPS)[number]['id'], string> = {
  overview: 'Обзор',
  analysis: 'Анализ',
  cleanup: 'Очистка',
  operations: 'Операции',
  tools: 'Инструменты',
  system: 'Система',
};

const GROUPS = SCREEN_GROUPS.map((group) => ({
  ...group,
  label: GROUP_LABELS[group.id],
}));

/** Screens that the recomposition absorbed into tabs, sections or actions. */
const ABSORBED = [
  'dashboard',
  'scan',
  'treemap',
  'size_details',
  'top_files',
  'types',
  'owners',
  'snapshots',
  'metadata',
  'archives',
  'unlock',
  'long_paths',
  'rename',
  'permissions',
  'elevation',
  'about',
  'export',
] as const;

const RUNTIME_MODULE = '/src/api/runtime.ts';
const ROUTING_MODULE = '/src/app/routing.ts';

interface ShellState {
  lastScreen: string | null;
  startupScreen: string;
}

/**
 * Reads the persisted shell settings and the startup rule from the running
 * application bundle.
 *
 * The browser preview keeps settings in memory. This helper checks that a
 * navigation does not overwrite `last_screen` any more and that every launch
 * resolves to the overview.
 */
async function readShellState(page: Page): Promise<ShellState> {
  return page.evaluate(
    async ({ runtimeModule, routingModule }) => {
      const runtime = (await import(/* @vite-ignore */ runtimeModule)) as {
        api: { appSettingsGet: () => Promise<{ last_screen: string | null }> };
      };
      const routing = (await import(/* @vite-ignore */ routingModule)) as {
        resolveInitialScreen: () => string;
      };
      const settings = await runtime.api.appSettingsGet();

      return {
        lastScreen: settings.last_screen,
        startupScreen: routing.resolveInitialScreen(),
      };
    },
    { runtimeModule: RUNTIME_MODULE, routingModule: ROUTING_MODULE },
  );
}

test.describe('shell', () => {
  test('boots on the overview with the grouped sidebar and the app icon', async ({ page }) => {
    await page.goto('/');

    await expect(page.getByRole('heading', { level: 1, name: 'Обзор' })).toBeVisible();
    await expect(page.getByRole('navigation', { name: 'Экраны' })).toBeVisible();

    // The brand block carries the application image, which is decorative.
    const sidebar = page.locator('aside').first();
    await expect(sidebar.getByText('FreeSpace', { exact: true })).toBeVisible();
    await expect(sidebar.locator('img[src="/icon.png"]')).toBeVisible();

    await expect(page.locator('[data-nav-group]')).toHaveCount(GROUPS.length);
    await expect(page.locator('[data-screen]')).toHaveCount(SCREEN_IDS.length);

    for (const group of GROUPS) {
      const block = page.locator(`[data-nav-group="${group.id}"]`);
      await expect(block).toContainText(group.label);
      await expect(block.locator('[data-screen]')).toHaveCount(group.screens.length);
    }

    await expect(page.locator('[data-screen="overview"]')).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  test('renders the six groups in plan order with their screens', async ({ page }) => {
    await page.goto('/');

    const layout = await page.locator('[data-nav-group]').evaluateAll((nodes) =>
      nodes.map((node) => ({
        group: node.getAttribute('data-nav-group'),
        screens: Array.from(node.querySelectorAll('[data-screen]')).map((item) =>
          item.getAttribute('data-screen'),
        ),
      })),
    );

    expect(layout).toEqual(
      GROUPS.map((group) => ({ group: group.id, screens: [...group.screens] })),
    );

    const order = await page.locator('[data-screen]').evaluateAll((nodes) =>
      nodes.map((node) => node.getAttribute('data-screen')),
    );
    expect(order).toEqual([...SCREEN_IDS]);
  });

  test('marks exactly one active screen with aria-current', async ({ page }) => {
    await page.goto('/');

    await page.locator('[data-screen="journal"]').click();
    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();
    await expect(page.locator('[data-screen="journal"]')).toHaveAttribute(
      'aria-current',
      'page',
    );
    await expect(page.locator('[data-screen="overview"]')).not.toHaveAttribute(
      'aria-current',
      'page',
    );
    await expect(
      page.locator('nav[aria-label="Экраны"] [aria-current="page"]'),
    ).toHaveCount(1);
  });

  test('absorbed screens are not navigation items any more', async ({ page }) => {
    await page.goto('/');

    for (const id of ABSORBED) {
      await expect(page.locator(`[data-screen="${id}"]`)).toHaveCount(0);
    }
  });

  test('exposes landmarks, the skip link and the status bar', async ({ page }) => {
    await page.goto('/');

    await expect(
      page.getByRole('link', { name: 'Перейти к основному содержимому' }),
    ).toHaveCount(1);
    await expect(page.getByRole('main')).toBeVisible();
    await expect(page.getByRole('contentinfo')).toBeVisible();

    const status = page.locator('[data-status-bar]');
    await expect(status).toBeVisible();
    await expect(status).toContainText('Готово');
  });

  test('the command palette lists all 11 screens and filters them', async ({ page }) => {
    await page.goto('/');
    await page.keyboard.press('Control+K');

    const palette = page.getByRole('dialog');
    await expect(palette).toBeVisible();
    await expect(palette.getByRole('option')).toHaveCount(SCREEN_IDS.length);

    const input = page.getByRole('combobox', { name: 'Введите команду…' });
    await input.fill('Журнал');

    const filtered = palette.getByRole('option');
    await expect(filtered).toHaveCount(1);
    await expect(filtered.first()).toContainText('Журнал');
    await expect(filtered.first()).toHaveAttribute('aria-selected', 'true');

    await page.keyboard.press('Enter');
    await expect(palette).toBeHidden();
    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();
  });

  test('starts on the overview and does not persist the last screen', async ({ page }) => {
    await page.goto('/');

    await page.locator('[data-screen="journal"]').click();
    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();

    const state = await readShellState(page);
    expect(state.startupScreen).toBe('overview');
    // Navigation no longer overwrites the stored value: the next launch also
    // opens the overview, and legacy values stay harmless.
    expect(state.lastScreen).toBe('overview');
  });
});
