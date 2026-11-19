import { afterEach, describe, expect, it } from 'vitest';

import type { UiSettings } from '../../src/api/dto';
import { resolveTheme, startSystemThemeSync, systemPrefersDark } from '../../src/app/theme';
import { useSettingsStore } from '../../src/stores/settingsStore';

function settings(theme: UiSettings['theme']): UiSettings {
  return {
    theme,
    language: 'ru',
    density: 'comfortable',
    units: 'binary',
    reduced_motion: 'system',
    last_screen: 'overview',
    sidebar_collapsed: false,
    column_layouts: {},
    saved_filters: {},
    schema_version: 1,
  };
}

function installMatchMedia(initialDark: boolean) {
  let dark = initialDark;
  const listeners = new Set<() => void>();
  // eslint-disable-next-line @typescript-eslint/unbound-method
  const original = window.matchMedia;

  window.matchMedia = ((query: string) =>
    ({
      get matches() {
        return query.includes('color-scheme') ? dark : false;
      },
      media: query,
      onchange: null,
      addEventListener: (_event: string, listener: () => void) => {
        listeners.add(listener);
      },
      removeEventListener: (_event: string, listener: () => void) => {
        listeners.delete(listener);
      },
      addListener: (listener: () => void) => {
        listeners.add(listener);
      },
      removeListener: (listener: () => void) => {
        listeners.delete(listener);
      },
      dispatchEvent: () => false,
    }) as unknown) as typeof window.matchMedia;

  return {
    setDark(value: boolean) {
      dark = value;
      for (const listener of listeners) listener();
    },
    restore() {
      window.matchMedia = original;
    },
  };
}

afterEach(() => {
  useSettingsStore.setState({ settings: null });
});

describe('system theme synchronisation', () => {
  it('follows OS changes when the preference is "system"', () => {
    const media = installMatchMedia(true);
    try {
      useSettingsStore.getState().setSettings(settings('system'));
      expect(document.documentElement.dataset.theme).toBe('dark');

      const stop = startSystemThemeSync();
      media.setDark(false);
      expect(document.documentElement.dataset.theme).toBe('light');
      media.setDark(true);
      expect(document.documentElement.dataset.theme).toBe('dark');
      stop();
    } finally {
      media.restore();
    }
  });

  it('keeps an explicit theme when the OS changes', () => {
    const media = installMatchMedia(true);
    try {
      useSettingsStore.getState().setSettings(settings('light'));
      const stop = startSystemThemeSync();

      media.setDark(true);
      expect(document.documentElement.dataset.theme).toBe('light');
      media.setDark(false);
      expect(document.documentElement.dataset.theme).toBe('light');
      stop();
    } finally {
      media.restore();
    }
  });

  it('resolves preferences and reports the OS value', () => {
    const media = installMatchMedia(false);
    try {
      expect(systemPrefersDark()).toBe(false);
      expect(resolveTheme('system', true)).toBe('dark');
      expect(resolveTheme('system', false)).toBe('light');
      expect(resolveTheme('dark', false)).toBe('dark');
      expect(resolveTheme('light', true)).toBe('light');
    } finally {
      media.restore();
    }
  });
});
