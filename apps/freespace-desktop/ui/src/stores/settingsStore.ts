import { create } from 'zustand';

import type { ApiClient } from '../api/client';
import type { ThemePreference, UiError, UiSettings, UiSettingsPatch } from '../api/dto';
import { normalizeError } from '../app/errors';

interface SettingsState {
  settings: UiSettings | null;
  busy: boolean;
  error: UiError | null;
  setSettings: (settings: UiSettings) => void;
  save: (api: ApiClient, patch: UiSettingsPatch) => Promise<void>;
  saveTheme: (api: ApiClient, theme: ThemePreference) => Promise<void>;
}

/** Interface settings shared by the shell and the settings screen. */
export const useSettingsStore = create<SettingsState>()((set, get) => ({
  settings: null,
  busy: false,
  error: null,

  setSettings(settings) {
    applySettingsToDocument(settings);
    set({ settings });
  },

  async save(api, patch) {
    set({ busy: true, error: null });
    try {
      const settings = await api.appSettingsSet(patch);
      applySettingsToDocument(settings);
      set({ settings, busy: false });
    } catch (error) {
      set({ busy: false, error: normalizeError(error) });
    }
  },

  async saveTheme(api, theme) {
    await get().save(api, { theme });
  },
}));

/** Resolves `system` against the OS preference. */
export function resolvedTheme(
  preference: ThemePreference,
  prefersDark: boolean,
): 'dark' | 'light' {
  if (preference === 'system') return prefersDark ? 'dark' : 'light';
  return preference;
}

/** Applies theme, language, density and motion to `<html>`. */
export function applySettingsToDocument(settings: UiSettings): void {
  if (typeof document === 'undefined') return;

  const root = document.documentElement;
  const prefersDark =
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches;
  const prefersReduced =
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  root.dataset.theme = resolvedTheme(settings.theme, prefersDark);
  root.dataset.density = settings.density;
  root.dataset.reducedMotion =
    settings.reduced_motion === 'on'
      ? 'true'
      : settings.reduced_motion === 'off'
        ? 'false'
        : prefersReduced
          ? 'true'
          : 'false';
  root.lang = settings.language;
  root.style.colorScheme = root.dataset.theme;
}

/**
 * Follows OS theme and motion changes without overwriting the saved preference.
 * Returns an unsubscribe function.
 */
export function watchSystemPreferences(
  getSettings: () => UiSettings | null,
): () => void {
  if (typeof window.matchMedia !== 'function') return () => undefined;

  const dark = window.matchMedia('(prefers-color-scheme: dark)');
  const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
  const apply = () => {
    const settings = getSettings();
    if (settings !== null) applySettingsToDocument(settings);
  };

  dark.addEventListener('change', apply);
  reduce.addEventListener('change', apply);

  return () => {
    dark.removeEventListener('change', apply);
    reduce.removeEventListener('change', apply);
  };
}
