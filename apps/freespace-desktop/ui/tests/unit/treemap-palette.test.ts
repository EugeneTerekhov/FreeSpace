import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import {
  TREEMAP_FALLBACK_TOKEN,
  TREEMAP_PALETTE,
  treemapToken,
} from '../../src/treemap/palette';

const themesCss = readFileSync(
  resolve(process.cwd(), 'src/styles/themes.css'),
  'utf8',
);

function blockFor(theme: 'dark' | 'light'): Map<string, string> {
  const marker = `:root[data-theme="${theme}"] {`;
  const start = themesCss.indexOf(marker);
  if (start === -1) throw new Error(`missing ${theme} theme`);
  const end = themesCss.indexOf('}', start);
  const map = new Map<string, string>();
  for (const match of themesCss
    .slice(start + marker.length, end)
    .matchAll(/(--fs-[\w-]+)\s*:\s*(#[0-9a-f]{6});/gi)) {
    const name = match[1];
    const value = match[2];
    if (name !== undefined && value !== undefined) map.set(name, value);
  }

  return map;
}

function luminance(hex: string): number {
  const value = hex.replace('#', '');
  const channels = [0, 2, 4].map((offset) => {
    const channel = Number.parseInt(value.slice(offset, offset + 2), 16) / 255;
    return channel <= 0.03928
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4;
  });
  const [red = 0, green = 0, blue = 0] = channels;

  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

function contrast(first: string, second: string): number {
  const a = luminance(first);
  const b = luminance(second);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

describe('Treemap palette', () => {
  it('maps every category to a theme token and has a fallback', () => {
    const categories = TREEMAP_PALETTE.map(([category]) => category);
    expect(categories).toEqual([
      'video',
      'image',
      'audio',
      'document',
      'archive',
      'code',
      'executable',
      'folder',
      'other',
    ]);
    expect(new Set(TREEMAP_PALETTE.map(([, token]) => token)).size).toBe(
      TREEMAP_PALETTE.length,
    );
    expect(treemapToken('video')).toBe('--fs-tm-1');
    expect(treemapToken('unknown-category')).toBe(TREEMAP_FALLBACK_TOKEN);
  });

  for (const theme of ['dark', 'light'] as const) {
    const variables = blockFor(theme);

    it(`${theme}: every tile is distinguishable from the background`, () => {
      const background = variables.get('--fs-bg') ?? '#000000';
      for (const [, token] of TREEMAP_PALETTE) {
        const colour = variables.get(token);
        expect(colour, `${theme} ${token}`).toBeDefined();
        const ratio = contrast(colour ?? '#000000', background);
        expect(
          ratio,
          `${theme} ${token} against the background: ${ratio.toFixed(2)}`,
        ).toBeGreaterThanOrEqual(3);
      }
    });

    it(`${theme}: tile labels keep AA contrast`, () => {
      const darkInk = variables.get('--fs-tile-ink') ?? '#000000';
      const lightInk = variables.get('--fs-tile-ink-alt') ?? '#ffffff';
      for (const [, token] of TREEMAP_PALETTE) {
        const colour = variables.get(token) ?? '#000000';
        const ratio = Math.max(
          contrast(darkInk, colour),
          contrast(lightInk, colour),
        );
        expect(
          ratio,
          `${theme} ${token} label: ${ratio.toFixed(2)}`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    });

    it(`${theme}: the ten category colours are unique`, () => {
      const colours = Array.from({ length: 10 }, (_, index) =>
        variables.get(`--fs-tm-${index + 1}`),
      );
      expect(colours.every((colour) => colour !== undefined)).toBe(true);
      expect(new Set(colours).size).toBe(10);
    });
  }
});
