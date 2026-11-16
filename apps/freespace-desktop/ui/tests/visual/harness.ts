import { expect, type Page } from '@playwright/test';

import type { ScreenId } from '../../src/api/dto';
import { screens, type UiLanguage } from '../e2e/pages/screens';

export type { UiLanguage };

export const VIEWPORTS = {
  desktop: { width: 1440, height: 900 },
  narrow: { width: 1024, height: 768 },
} as const;

export const THEMES = ['dark', 'light'] as const;

export type ThemeName = (typeof THEMES)[number];

/** Device scale factors of the full visual matrix. */
export const DPIS = [100, 125, 150, 200] as const;

export type Dpi = (typeof DPIS)[number];

/** Interface languages of the full visual matrix. */
export const LANGUAGES = ['ru', 'en'] as const;

export interface BootState {
  theme: ThemeName;
  language: UiLanguage;
  reducedMotion?: 'on' | 'off' | 'system';
}

/**
 * Injects the state the Tauri shell usually reads from `settings.toml`.
 *
 * Must be called before `page.goto()`: `boot.js` applies it synchronously and
 * the mock transport returns the same values, so the shell never re-applies a
 * different theme after the first paint.
 */
export async function applyBoot(page: Page, state: BootState): Promise<void> {
  await page.addInitScript(
    (boot: { theme: string; language: string; reduced_motion: string }) => {
      (window as unknown as { __FREESPACE_BOOT__: unknown }).__FREESPACE_BOOT__ =
        boot;
    },
    {
      theme: state.theme,
      language: state.language,
      reduced_motion: state.reducedMotion ?? 'on',
    },
  );
}

/**
 * Applies a resolved theme without touching the saved preference.
 *
 * Waits two frames first so the asynchronous settings bootstrap has already
 * written the saved theme; otherwise it would overwrite the requested one.
 */
export async function setTheme(page: Page, theme: ThemeName): Promise<void> {
  await page.evaluate(async () => {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          resolve();
        });
      });
    });
  });
  await page.evaluate((value) => {
    document.documentElement.dataset.theme = value;
    document.documentElement.style.colorScheme = value;
  }, theme);
}

/** Disables motion and waits for fonts so screenshots are deterministic. */
export async function stabilize(page: Page): Promise<void> {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  await page.waitForTimeout(150);
}

/** Waits for the shell to finish bootstrapping. */
export async function waitForShell(page: Page): Promise<void> {
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
}

/** Names a baseline screenshot. */
export function baselineName(
  screen: string,
  theme: ThemeName,
  suffix = 'desktop',
): string {
  return `${screen}-${theme}-${suffix}.png`;
}

/** Path of one cell of the full visual matrix. */
export function matrixSnapshotPath(
  screen: ScreenId,
  theme: ThemeName,
  dpi: Dpi,
  language: UiLanguage,
): string {
  return `${screen}-${theme}-${String(dpi)}-${language}.png`;
}

/** Screens that render their data only after a finished scan. */
export const NEEDS_SCAN: readonly ScreenId[] = [
  'size_details',
  'long_paths',
  'rename',
  'permissions',
  'metadata',
  'owners',
  'export',
];

/** Screens that need a scanned generation. */
export const ANALYSIS_SCREENS: readonly ScreenId[] = [
  'top_files',
  'types',
  'duplicates',
  'cleanup',
  'snapshots',
];

/**
 * Brings a screen into the state the matrix records.
 *
 * The shell must already be loaded; pass `skipScan` when the shared page has a
 * finished scan for the current language.
 */
export async function prepareScreen(
  page: Page,
  id: ScreenId,
  language: UiLanguage,
  options: { skipScan?: boolean } = {},
): Promise<void> {
  const app = screens(page);
  const scanFirst = options.skipScan !== true;

  if (id === 'dashboard') {
    return;
  }

  if (id === 'tree' || id === 'treemap') {
    if (scanFirst) {
      await app.runScan(language);
    }
    if (id === 'tree') {
      await app.open('tree');
      await app.openTreeRoot();
    } else {
      await app.open('treemap');
    }
    return;
  }

  if (ANALYSIS_SCREENS.includes(id) || NEEDS_SCAN.includes(id)) {
    if (scanFirst) {
      await app.runScan(language);
    }
  }

  await app.open(id);
}
