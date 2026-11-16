import { expect, test, type Browser, type Page } from '@playwright/test';

import type { ScreenId } from '../../src/api/dto';
import { SCREEN_IDS, screens } from '../e2e/pages/screens';
import {
  THEMES,
  applyBoot,
  matrixSnapshotPath,
  prepareScreen,
  stabilize,
  waitForShell,
  type Dpi,
  type ThemeName,
  type UiLanguage,
} from './harness';

/**
 * Pull-request subset of the visual matrix.
 *
 * Eight critical screens × 2 themes × 100/150% × 2 languages, plus every screen
 * in dark theme, 100% DPI and Russian. It compares against the same baselines
 * as `matrix.spec.ts`, so the full run owns generation and the PR gate only
 * reads.
 */
const CRITICAL: readonly ScreenId[] = [
  'dashboard',
  'scan',
  'tree',
  'treemap',
  'top_files',
  'duplicates',
  'cleanup',
  'settings',
];

const PR_DPI: readonly Dpi[] = [100, 150];

async function capture(
  browser: Browser,
  theme: ThemeName,
  dpi: Dpi,
  language: UiLanguage,
  ids: readonly ScreenId[],
): Promise<void> {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: dpi / 100,
  });
  const page: Page = await context.newPage();

  try {
    await applyBoot(page, { theme, language });
    await page.goto('/');
    await waitForShell(page);
    await screens(page).runScan(language);

    for (const id of ids) {
      await prepareScreen(page, id, language, { skipScan: true });
      await stabilize(page);
      await expect(page).toHaveScreenshot(
        matrixSnapshotPath(id, theme, dpi, language),
        { maxDiffPixelRatio: id === 'treemap' ? 0.02 : 0.01 },
      );
    }
  } finally {
    await context.close();
  }
}

for (const dpi of PR_DPI) {
  for (const theme of THEMES) {
    for (const language of (['ru', 'en'] as const)) {
      test(`pr ${theme} ${String(dpi)}% ${language}`, async ({ browser }) => {
        test.setTimeout(120_000);
        await capture(browser, theme, dpi, language, CRITICAL);
      });
    }
  }
}

test('pr dark 100% ru full set', async ({ browser }) => {
  test.setTimeout(180_000);
  await capture(browser, 'dark', 100, 'ru', SCREEN_IDS);
});
