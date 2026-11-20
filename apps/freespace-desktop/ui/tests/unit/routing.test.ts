import { describe, expect, it } from 'vitest';

import {
  GROUPS,
  SCREENS,
  SCREEN_BY_ID,
  groupEntries,
  isScreenId,
  normalizeScreenId,
  resolveInitialScreen,
  screensInGroup,
} from '../../src/app/routing';

describe('screen registry', () => {
  it('contains exactly 9 unique screens', () => {
    expect(SCREENS).toHaveLength(9);
    expect(new Set(SCREENS.map((screen) => screen.id)).size).toBe(9);

    for (const screen of SCREENS) {
      expect(SCREEN_BY_ID[screen.id]).toBe(screen);
      expect(screen.titleKey).toMatch(/^nav\./);
      expect(isScreenId(screen.id)).toBe(true);
    }
  });

  it('covers every screen with exactly one sidebar group', () => {
    expect(GROUPS).toHaveLength(6);

    const grouped = GROUPS.flatMap((group) => group.screens);
    expect(grouped).toHaveLength(SCREENS.length);
    expect(new Set(grouped).size).toBe(SCREENS.length);

    for (const screen of SCREENS) {
      expect(screensInGroup(screen.group).map((item) => item.id)).toContain(screen.id);
    }

    expect(groupEntries().map(({ group }) => group.id)).toEqual([
      'overview',
      'analysis',
      'cleanup',
      'operations',
      'tools',
      'system',
    ]);
  });

  it('restores the last screen only when it is known', () => {
    expect(resolveInitialScreen('tree', true)).toBe('tree');
    expect(resolveInitialScreen('unknown', true)).toBe('overview');
    expect(resolveInitialScreen(null, true)).toBe('overview');
    expect(resolveInitialScreen('tree', false)).toBe('overview');
  });

  it('maps screens removed by the recomposition to their new owner', () => {
    expect(normalizeScreenId('dashboard')).toBe('overview');
    expect(normalizeScreenId('scan')).toBe('overview');
    expect(normalizeScreenId('treemap')).toBe('tree');
    expect(normalizeScreenId('size_details')).toBe('tree');
    expect(normalizeScreenId('top_files')).toBe('analysis');
    expect(normalizeScreenId('types')).toBe('analysis');
    expect(normalizeScreenId('owners')).toBe('analysis');
    expect(normalizeScreenId('snapshots')).toBe('cleanup');
    expect(normalizeScreenId('metadata')).toBe('inspect');
    expect(normalizeScreenId('archives')).toBe('inspect');
    expect(normalizeScreenId('unlock')).toBe('inspect');
    expect(normalizeScreenId('long_paths')).toBe('inspect');
    expect(normalizeScreenId('shred')).toBe('journal');
    expect(normalizeScreenId('fileops')).toBe('tree');
    expect(normalizeScreenId('rename')).toBe('tree');
    expect(normalizeScreenId('permissions')).toBe('tree');
    expect(normalizeScreenId('elevation')).toBe('settings');
    expect(normalizeScreenId('about')).toBe('settings');
    expect(normalizeScreenId('export')).toBe('analysis');
    expect(normalizeScreenId('duplicates')).toBe('duplicates');
    expect(normalizeScreenId('menu')).toBeNull();
  });

  it('keeps a CLI analogue for every screen', () => {
    for (const screen of SCREENS) {
      expect(typeof screen.cliAnalog).toBe('string');
      expect(screen.cliAnalog).not.toBe('');
    }

    expect(SCREEN_BY_ID.tree.cliAnalog).toBe('tree, filter');
    expect(SCREEN_BY_ID.overview.cliAnalog).toBe('drives, scan');
    expect(SCREEN_BY_ID.analysis.cliAnalog).toBe('top, types, owners');
  });
});
