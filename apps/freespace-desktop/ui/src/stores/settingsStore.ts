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

/**
 * Swaps the palette without animating between the two themes.
 *
 * Colour transitions exist for interaction (hover, focus). During a
 * programmatic theme change they make every surface wash to the new palette at
 * its own pace and can leave text below AA for a frame. The root therefore
 * carries a one-frame flag that disables transitions inside the swap.
 */
function applyDocumentTheme(root: HTMLElement, theme: 'dark' | 'light'): void {
  root.style.colorScheme = theme;
  if (root.dataset.theme === theme) return;

  root.dataset.themeSwitching = 'true';
  root.dataset.theme = theme;

  const release = () => {
    delete root.dataset.themeSwitching;
  };
  if (typeof requestAnimationFrame === 'function') {
    requestAnimationFrame(release);
  } else {
    setTimeout(release, 0);
  }
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

  applyDocumentTheme(root, resolvedTheme(settings.theme, prefersDark));
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
