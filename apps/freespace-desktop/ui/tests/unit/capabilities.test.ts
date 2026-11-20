import { describe, expect, it } from 'vitest';

import {
  LINUX_CAPABILITIES,
  WINDOWS_CAPABILITIES,
  canElevate,
  canRevealInFileManager,
  canUnlock,
  canUseContextMenu,
  canUseLongPaths,
  defaultDeleteMode,
  deleteModes,
  scanMethods,
} from '../../src/lib/capabilities';

describe('platform capabilities', () => {
  it('offers both deletion modes on Windows', () => {
    expect(deleteModes(WINDOWS_CAPABILITIES)).toEqual(['recycle_bin', 'permanent']);
    expect(defaultDeleteMode(WINDOWS_CAPABILITIES)).toBe('recycle_bin');
  });

  it('drops the Recycle Bin option when the platform has no system trash', () => {
    const noTrash = { ...WINDOWS_CAPABILITIES, trash: false };

    expect(deleteModes(noTrash)).toEqual(['permanent']);
    expect(defaultDeleteMode(noTrash)).toBe('permanent');
  });

  it('drops permanent deletion when the backend cannot delete irreversibly', () => {
    const noPermanent = { ...WINDOWS_CAPABILITIES, permanent: false };

    expect(deleteModes(noPermanent)).toEqual(['recycle_bin']);
  });

  it('offers no deletion mode when the platform supports neither', () => {
    const neither = { ...WINDOWS_CAPABILITIES, trash: false, permanent: false };

    expect(deleteModes(neither)).toEqual([]);
    expect(defaultDeleteMode(neither)).toBeNull();
  });

  it('keeps the full Windows set when capabilities are unknown', () => {
    expect(deleteModes(null)).toEqual(['recycle_bin', 'permanent']);
    expect(defaultDeleteMode(undefined)).toBe('recycle_bin');
    expect(canUseContextMenu(null)).toBe(true);
    expect(canElevate(null)).toBe(true);
    expect(canUnlock(null)).toBe(true);
    expect(canUseLongPaths(null)).toBe(true);
    expect(canRevealInFileManager(null)).toBe(true);
    expect(scanMethods(null)).toEqual(['auto', 'mft', 'walk']);
  });

  it('hides Windows-only features on Linux', () => {
    expect(deleteModes(LINUX_CAPABILITIES)).toEqual(['recycle_bin', 'permanent']);
    expect(canUseContextMenu(LINUX_CAPABILITIES)).toBe(false);
    expect(canUnlock(LINUX_CAPABILITIES)).toBe(false);
    expect(canUseLongPaths(LINUX_CAPABILITIES)).toBe(false);
    expect(canRevealInFileManager(LINUX_CAPABILITIES)).toBe(false);
    expect(scanMethods(LINUX_CAPABILITIES)).toEqual(['auto', 'walk']);
  });

  it('keeps elevation configurable per backend', () => {
    expect(canElevate({ ...LINUX_CAPABILITIES, elevation: false })).toBe(false);
    expect(canElevate(WINDOWS_CAPABILITIES)).toBe(true);
  });
});
