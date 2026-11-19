// Synchronous boot script: runs before the first paint so the shell never
// flashes the wrong theme. The Tauri shell injects `window.__FREESPACE_BOOT__`
// from `settings.toml`; a plain browser falls back to the system preference.
(function applyBootState() {
  'use strict';

  var boot = window.__FREESPACE_BOOT__;
  var root = document.documentElement;
  var theme = boot && boot.theme ? boot.theme : 'system';
  var prefersDark =
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches;

  if (theme !== 'dark' && theme !== 'light') {
    theme = prefersDark ? 'dark' : 'light';
  }

  root.dataset.theme = theme;
  // Kept for the no-flash E2E: proves the theme was applied by this synchronous
  // script rather than by React after the first paint.
  root.dataset.themeBoot = theme;
  root.style.colorScheme = theme;

  if (boot && boot.language) {
    root.lang = boot.language;
  }

  var motion = boot && boot.reduced_motion ? boot.reduced_motion : 'system';
  var prefersReduced =
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  root.dataset.reducedMotion =
    motion === 'on' || (motion !== 'off' && prefersReduced) ? 'true' : 'false';
})();
