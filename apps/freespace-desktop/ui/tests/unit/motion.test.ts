import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import type { UiSettings } from '../../src/api/dto';
import { applySettingsToDocument } from '../../src/stores/settingsStore';

const globalCss = readFileSync(
  resolve(process.cwd(), 'src/styles/global.css'),
  'utf8',
);
const tokensCss = readFileSync(
  resolve(process.cwd(), 'src/styles/tokens.css'),
  'utf8',
);

function settings(
  reduced: UiSettings['reduced_motion'],
): UiSettings {
  return {
    theme: 'dark',
    language: 'ru',
    density: 'comfortable',
    units: 'binary',
    reduced_motion: reduced,
    last_screen: 'dashboard',
    sidebar_collapsed: false,
    column_layouts: {},
    saved_filters: {},
    schema_version: 1,
  };
}

function withSystemMotion(reduced: boolean): () => void {
  // eslint-disable-next-line @typescript-eslint/unbound-method
  const original = window.matchMedia;
  window.matchMedia = ((query: string) =>
    ({
      matches: query.includes('reduced-motion') ? reduced : true,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false,
    }));

  return () => {
    window.matchMedia = original;
  };
}

describe('functional motion', () => {
  it('honours an explicit reduced-motion choice over the OS', () => {
    const restore = withSystemMotion(false);
    try {
      applySettingsToDocument(settings('on'));
      expect(document.documentElement.dataset.reducedMotion).toBe('true');
      applySettingsToDocument(settings('off'));
      expect(document.documentElement.dataset.reducedMotion).toBe('false');
    } finally {
      restore();
    }
  });

  it('follows the OS when the preference is "system"', () => {
    const reduce = withSystemMotion(true);
    try {
      applySettingsToDocument(settings('system'));
      expect(document.documentElement.dataset.reducedMotion).toBe('true');
    } finally {
      reduce();
    }

    const noReduce = withSystemMotion(false);
    try {
      applySettingsToDocument(settings('system'));
      expect(document.documentElement.dataset.reducedMotion).toBe('false');
    } finally {
      noReduce();
    }
  });

  it('ships reduced-motion and high-contrast CSS fallbacks', () => {
    expect(tokensCss).toContain('--fs-motion-reduced');
    expect(globalCss).toContain('@media (prefers-reduced-motion: reduce)');
    expect(globalCss).toContain('@media (prefers-contrast: more)');
    expect(globalCss).toContain('data-reduced-motion="true"');
  });
});
