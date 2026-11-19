import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { runInNewContext } from 'node:vm';

import { describe, expect, it } from 'vitest';

const script = readFileSync(resolve(process.cwd(), 'public/boot.js'), 'utf8');

interface FakeRoot {
  dataset: Record<string, string>;
  lang: string;
  style: Record<string, string>;
}

function evaluate(boot: unknown, prefers: { dark?: boolean; reduced?: boolean } = {}) {
  const root: FakeRoot = { dataset: {}, lang: 'ru', style: {} };
  const context = {
    window: {
      __FREESPACE_BOOT__: boot,
      matchMedia: (query: string) => ({
        matches: query.includes('dark') ? (prefers.dark ?? false) : (prefers.reduced ?? false),
      }),
    },
    document: { documentElement: root },
  };

  runInNewContext(script, context);

  return root;
}

describe('boot script', () => {
  it('applies the injected theme, language and motion before paint', () => {
    const root = evaluate({ theme: 'dark', language: 'en', reduced_motion: 'off' });

    expect(root.dataset.theme).toBe('dark');
    expect(root.dataset.themeBoot).toBe('dark');
    expect(root.lang).toBe('en');
    expect(root.dataset.reducedMotion).toBe('false');
    expect(root.style.colorScheme).toBe('dark');
  });

  it('resolves the system theme and motion preference', () => {
    const root = evaluate(
      { theme: 'system', language: 'ru', reduced_motion: 'system' },
      { dark: true, reduced: true },
    );

    expect(root.dataset.theme).toBe('dark');
    expect(root.dataset.reducedMotion).toBe('true');
  });

  it('falls back to the OS theme without an injected object', () => {
    const root = evaluate(undefined, { dark: false, reduced: false });

    expect(root.dataset.theme).toBe('light');
    expect(root.dataset.reducedMotion).toBe('false');
  });

  it('honours an explicit reduced-motion setting over the OS', () => {
    const root = evaluate(
      { theme: 'light', language: 'ru', reduced_motion: 'on' },
      { dark: true, reduced: false },
    );

    expect(root.dataset.theme).toBe('light');
    expect(root.dataset.reducedMotion).toBe('true');
  });
});
