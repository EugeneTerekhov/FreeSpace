import { describe, expect, it } from 'vitest';

import {
  SCREENS,
  SCREEN_BY_ID,
  isScreenId,
  resolveInitialScreen,
  screensInBatch,
} from '../../src/app/routing';

describe('screen registry', () => {
  it('contains exactly 24 unique screens', () => {
    expect(SCREENS).toHaveLength(24);
    expect(new Set(SCREENS.map((screen) => screen.id)).size).toBe(24);

    for (const screen of SCREENS) {
      expect(SCREEN_BY_ID[screen.id]).toBe(screen);
      expect(screen.titleKey).toMatch(/^nav\./);
      expect(isScreenId(screen.id)).toBe(true);
    }
  });

  it('restores the last screen only when it is known', () => {
    expect(resolveInitialScreen('tree', true)).toBe('tree');
    expect(resolveInitialScreen('unknown' as never, true)).toBe('dashboard');
    expect(resolveInitialScreen(null, true)).toBe('dashboard');
    expect(resolveInitialScreen('tree', false)).toBe('dashboard');
  });

  it('groups screens by delivery batch', () => {
    expect(screensInBatch('core').map((screen) => screen.id)).toEqual([
      'dashboard',
      'scan',
      'tree',
      'treemap',
    ]);
    expect(screensInBatch('settings')).toHaveLength(5);
  });

  it('keeps a CLI analogue for every non-shell capability', () => {
    for (const screen of SCREENS) {
      if (screen.batch === 'settings' || screen.batch === 'inspection') {
        expect(screen.cliAnalog === null || typeof screen.cliAnalog === 'string').toBe(
          true,
        );
      }
    }
    expect(SCREEN_BY_ID.tree.cliAnalog).toBe('tree');
    expect(SCREEN_BY_ID.dashboard.cliAnalog).toBe('drives');
  });
});
