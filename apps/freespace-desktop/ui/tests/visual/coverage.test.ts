import { existsSync, readdirSync } from 'node:fs';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { SCREENS } from '../../src/app/routing';

/**
 * A screen is accepted only when the full visual matrix has a reference for it.
 *
 * The check is intentionally filesystem-based: adding a screen to the registry
 * without regenerating the matrix (`npm run test:visual:update`) fails here.
 */

const VISUAL_DIR = path.resolve(__dirname);
const BASELINES = path.join(VISUAL_DIR, 'baselines');

const THEMES = ['dark', 'light'] as const;
const DPIS = [100, 125, 150, 200] as const;
const LANGUAGES = ['ru', 'en'] as const;

/** Playwright replaces underscores with dashes in snapshot file names. */
function snapshotName(screen: string, theme: string, dpi: number, language: string): string {
  return `${screen.replaceAll('_', '-')}-${theme}-${String(dpi)}-${language}.png`;
}

describe('visual coverage', () => {
  it('keeps the registry at 24 screens', () => {
    expect(SCREENS).toHaveLength(24);
    expect(new Set(SCREENS.map((screen) => screen.id)).size).toBe(24);
  });

  it('has every matrix cell for every screen', () => {
    const missing: string[] = [];

    for (const screen of SCREENS) {
      for (const theme of THEMES) {
        for (const dpi of DPIS) {
          for (const language of LANGUAGES) {
            const file = path.join(
              BASELINES,
              snapshotName(screen.id, theme, dpi, language),
            );
            if (!existsSync(file)) {
              missing.push(path.basename(file));
            }
          }
        }
      }
    }

    expect(missing).toEqual([]);
  });

  it('has no matrix baseline for an unknown screen', () => {
    const ids = new Set<string>(SCREENS.map((screen) => screen.id));
    const matrixName = /^(?<screen>.+)-(?:dark|light)-(?:100|125|150|200)-(?:ru|en)\.png$/;
    const orphans = readdirSync(BASELINES)
      .filter((name) => matrixName.test(name))
      .filter((name) => {
        const match = matrixName.exec(name);
        const screen = match?.groups?.screen;
        return screen !== undefined && !ids.has(screen.replaceAll('-', '_'));
      });

    expect(orphans).toEqual([]);
  });

  it('keeps the curated delivery batches in the same tree', () => {
    // The batch specs add states the matrix does not record (dialogs, previews,
    // search results), so at least one flat baseline must exist per batch.
    const files = readdirSync(BASELINES).filter((name) => name.endsWith('.png'));
    for (const prefix of ['operation-preview', 'operation-progress', 'shred-preview', 'filesystem-journal']) {
      expect(files.some((name) => name.startsWith(`${prefix}-`))).toBe(true);
    }
  });
});
