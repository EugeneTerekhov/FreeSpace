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
  restored: string;
  legacyDashboard: string;
  legacyScan: string;
  unknown: string;
  beforeReady: string;
}

/**
 * Reads the persisted shell settings and the resolution rules from the running
 * application bundle.
 *
 * The browser preview keeps settings in memory, so a page reload always starts
 * from the seeded defaults. This helper verifies the two halves that make the
 * restore work in the packaged app: the shell persists `last_screen` after a
 * navigation, and the router maps both live and legacy identifiers to a screen.
 */
async function readShellState(page: Page): Promise<ShellState> {
  return page.evaluate(
    async ({ runtimeModule, routingModule }) => {
      const runtime = (await import(/* @vite-ignore */ runtimeModule)) as {
        api: { appSettingsGet: () => Promise<{ last_screen: string | null }> };
      };
      const routing = (await import(/* @vite-ignore */ routingModule)) as {
        resolveInitialScreen: (last: string | null | undefined, ready: boolean) => string;
      };
      const settings = await runtime.api.appSettingsGet();

      return {
        lastScreen: settings.last_screen,
        restored: routing.resolveInitialScreen(settings.last_screen, true),
        legacyDashboard: routing.resolveInitialScreen('dashboard', true),
        legacyScan: routing.resolveInitialScreen('scan', true),
        unknown: routing.resolveInitialScreen('no-such-screen', true),
        beforeReady: routing.resolveInitialScreen('dashboard', false),
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

  test('persists the last screen and resolves legacy identifiers', async ({ page }) => {
    await page.goto('/');

    await page.locator('[data-screen="journal"]').click();
    await expect(page.getByRole('heading', { level: 1, name: 'Журнал' })).toBeVisible();

    await expect
      .poll(async () => (await readShellState(page)).lastScreen)
      .toBe('journal');

    const state = await readShellState(page);
    expect(state.restored).toBe('journal');
    // Values written by an older build keep opening the closest live screen.
    expect(state.legacyDashboard).toBe('overview');
    expect(state.legacyScan).toBe('overview');
    // Unknown identifiers fall back to the overview instead of a blank shell.
    expect(state.unknown).toBe('overview');
    // The stored value is only honoured after the bootstrap has loaded.
    expect(state.beforeReady).toBe('overview');
  });
});
