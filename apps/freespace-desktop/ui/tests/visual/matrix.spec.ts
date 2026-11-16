import { expect, test, type Browser, type Page } from '@playwright/test';

import { SCREEN_IDS, screens } from '../e2e/pages/screens';
import {
  DPIS,
  LANGUAGES,
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
 * Full visual matrix: 24 screens × 2 themes × 4 DPI × 2 languages.
 *
 * One test captures a whole combination so the mock scan and the page load are
 * shared by 24 screenshots. Baselines live in `tests/visual/baselines/` and are
 * referenced by `pr.spec.ts` as well.
 */
const TOLERANCE: Record<string, number> = { treemap: 0.02 };

async function captureMatrix(
  browser: Browser,
  theme: ThemeName,
  dpi: Dpi,
  language: UiLanguage,
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

    // One finished scan feeds every screen that needs a generation.
    await screens(page).runScan(language);

    for (const id of SCREEN_IDS) {
      await prepareScreen(page, id, language, { skipScan: true });
      await stabilize(page);
      await expect(page).toHaveScreenshot(
        matrixSnapshotPath(id, theme, dpi, language),
        {
          maxDiffPixelRatio: TOLERANCE[id] ?? 0.01,
        },
      );
    }
  } finally {
    await context.close();
  }
}

for (const dpi of DPIS) {
  for (const theme of THEMES) {
    for (const language of LANGUAGES) {
      test(`matrix ${theme} ${String(dpi)}% ${language}`, async ({ browser }) => {
        test.setTimeout(180_000);
        await captureMatrix(browser, theme, dpi, language);
      });
    }
  }
}
