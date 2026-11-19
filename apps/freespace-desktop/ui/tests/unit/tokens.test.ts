import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const themesCss = readFileSync(
  resolve(process.cwd(), 'src/styles/themes.css'),
  'utf8',
);
const tokensCss = readFileSync(
  resolve(process.cwd(), 'src/styles/tokens.css'),
  'utf8',
);

function blockFor(theme: 'dark' | 'light'): string {
  const marker = `:root[data-theme="${theme}"] {`;
  const start = themesCss.indexOf(marker);
  if (start === -1) throw new Error(`missing ${theme} theme`);
  const end = themesCss.indexOf('}', start);
  return themesCss.slice(start + marker.length, end);
}

function variables(block: string): Map<string, string> {
  const map = new Map<string, string>();
  for (const match of block.matchAll(/(--fs-[\w-]+)\s*:\s*([^;]+);/g)) {
    const name = match[1];
    const value = match[2];
    if (name !== undefined && value !== undefined) {
      map.set(name, value.trim());
    }
  }
  return map;
}

function luminance(hex: string): number {
  let value = hex.replace('#', '');
  if (value.length === 3) {
    value = value
      .split('')
      .map((char) => char + char)
      .join('');
  }
  const channels = [0, 2, 4].map((offset) => {
    const channel = Number.parseInt(value.slice(offset, offset + 2), 16) / 255;
    return channel <= 0.03928
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4;
  });
  const [red = 0, green = 0, blue = 0] = channels;
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function contrast(foreground: string, background: string): number {
  const first = luminance(foreground);
  const second = luminance(background);
  const lighter = Math.max(first, second);
  const darker = Math.min(first, second);
  return (lighter + 0.05) / (darker + 0.05);
}

const PAIRS: [string, string, number][] = [
  ['--fs-text', '--fs-bg', 4.5],
  ['--fs-text', '--fs-surface', 4.5],
  ['--fs-text', '--fs-surface-overlay', 4.5],
  ['--fs-text-muted', '--fs-surface', 4.5],
  ['--fs-text-subtle', '--fs-surface-raised', 4.5],
  ['--fs-accent-ink', '--fs-accent', 4.5],
  ['--fs-accent-strong', '--fs-surface', 4.5],
  ['--fs-danger', '--fs-surface', 4.5],
  ['--fs-success', '--fs-surface', 4.5],
  ['--fs-warning', '--fs-surface', 4.5],
  ['--fs-info', '--fs-surface', 4.5],
];

describe('design tokens', () => {
  for (const theme of ['dark', 'light'] as const) {
    const variablesInTheme = variables(blockFor(theme));

    it(`${theme}: text and status pairs pass WCAG AA`, () => {
      for (const [foreground, background, minimum] of PAIRS) {
        const fg = variablesInTheme.get(foreground);
        const bg = variablesInTheme.get(background);
        expect(fg, `${foreground} is missing`).toBeDefined();
        expect(bg, `${background} is missing`).toBeDefined();
        const ratio = contrast(fg ?? '#000', bg ?? '#fff');
        expect(
          ratio,
          `${theme} ${foreground} on ${background}: ${ratio.toFixed(2)}`,
        ).toBeGreaterThanOrEqual(minimum);
      }
    });

    it(`${theme}: defines the full surface palette`, () => {
      for (const token of [
        '--fs-bg',
        '--fs-surface',
        '--fs-surface-raised',
        '--fs-surface-overlay',
        '--fs-text',
        '--fs-selection',
      ]) {
        expect(variablesInTheme.get(token), `${theme} ${token}`).toBeDefined();
      }
    });
  }

  it('defines spacing, radius, density, layering and motion tokens', () => {
    for (const token of [
      '--fs-space-1',
      '--fs-space-10',
      '--fs-radius-sm',
      '--fs-radius-pill',
      '--fs-row-comfortable',
      '--fs-row-compact',
      '--fs-header',
      '--fs-sidebar',
      '--fs-sidebar-collapsed',
      '--fs-status',
      '--fs-z-dialog',
      '--fs-motion-hover',
      '--fs-ease',
    ]) {
      expect(tokensCss, token).toContain(token);
    }
  });
});
