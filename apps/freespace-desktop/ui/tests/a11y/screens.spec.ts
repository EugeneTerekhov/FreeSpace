import { expect, test, type Page } from '@playwright/test';

import type { ScreenId } from '../../src/api/dto';
import { SCREEN_IDS, screens } from '../e2e/pages/screens';
import { setTheme } from '../visual/harness';
import { expectNoViolations } from './axe';

/** All 24 screens take part in the final accessibility matrix. */
const SCREENS = SCREEN_IDS;

type ScreenName = ScreenId;

const THEMES = ['dark', 'light'] as const;

/** Screens whose data requires a finished scan before they render. */
const NEEDS_SCAN: ScreenId[] = [
  'size_details',
  'long_paths',
  'rename',
  'permissions',
  'metadata',
  'owners',
  'export',
];

const ANALYSIS_SCREENS: ScreenId[] = [
  'top_files',
  'types',
  'duplicates',
  'cleanup',
  'snapshots',
];

async function openScreen(page: Page, id: ScreenName): Promise<void> {
  await page.goto('/');
  const app = screens(page);

  if (id === 'tree' || id === 'treemap') {
    await app.withSnapshot(id);
  } else if (ANALYSIS_SCREENS.includes(id)) {
    await app.withAnalysis(
      id as 'top_files' | 'types' | 'duplicates' | 'cleanup' | 'snapshots',
    );
  } else if (id === 'dashboard') {
    // Nothing else to prepare.
  } else if (NEEDS_SCAN.includes(id)) {
    await app.runScan();
    await app.open(id);
  } else {
    await app.open(id);
  }

  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
}

test.describe('screen accessibility', () => {
  for (const id of SCREENS) {
    test(`${id} meets WCAG AA in both themes`, async ({ page }) => {
      await openScreen(page, id);

      // Landmarks and the skip target must exist on every screen.
      await expect(page.getByRole('navigation', { name: 'Экраны' })).toBeVisible();
      await expect(page.getByRole('main')).toBeVisible();
      await expect(
        page.getByRole('link', { name: 'Перейти к основному содержимому' }),
      ).toHaveCount(1);

      for (const theme of THEMES) {
        await setTheme(page, theme);
        await expectNoViolations(page);
      }
    });
  }

  test('all 24 screens are part of the audit matrix', () => {
    expect(SCREENS).toHaveLength(24);
    expect(new Set(SCREENS).size).toBe(24);
  });
});
