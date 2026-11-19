/**
 * Theme runtime: resolves the saved preference against the operating system and
 * keeps it in sync without overwriting the user's choice.
 *
 * `boot.js` applies the same rules synchronously before the first paint; this
 * module only reacts to later OS changes.
 */

import type { ThemePreference, UiSettings } from '../api/dto';
import {
  applySettingsToDocument,
  useSettingsStore,
  watchSystemPreferences,
} from '../stores/settingsStore';

/** Media query for the operating-system colour scheme. */
export const DARK_SCHEME_QUERY = '(prefers-color-scheme: dark)';

/** Media query for the operating-system reduced-motion preference. */
export const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

/** Concrete theme applied to the document. */
export type ResolvedTheme = 'dark' | 'light';

/** Returns true when the OS currently prefers a dark colour scheme. */
export function systemPrefersDark(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return true;
  }

  return window.matchMedia(DARK_SCHEME_QUERY).matches;
}

/** Resolves `system` against the OS preference. */
export function resolveTheme(
  preference: ThemePreference,
  prefersDark: boolean = systemPrefersDark(),
): ResolvedTheme {
  return preference === 'system' ? (prefersDark ? 'dark' : 'light') : preference;
}

/** Applies theme, density, motion and language to `<html>`. */
export function applyTheme(settings: UiSettings): void {
  applySettingsToDocument(settings);
}

/**
 * Subscribes to OS theme and motion changes.
 *
 * Only a `system` preference follows the OS; explicit `dark`/`light` values stay
 * untouched. Returns an unsubscribe function.
 */
export function startSystemThemeSync(): () => void {
  return watchSystemPreferences(() => useSettingsStore.getState().settings);
}
