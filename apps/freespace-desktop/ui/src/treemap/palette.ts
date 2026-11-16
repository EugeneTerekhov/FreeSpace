/**
 * Treemap category palette.
 *
 * The canvas cannot use CSS classes, so it reads the theme tokens at paint time.
 * Both palettes are checked for contrast against the label ink in
 * `tests/unit/treemap-palette.test.ts`.
 */

/** Categories in legend order, mapped to their theme tokens. */
export const TREEMAP_PALETTE = [
  ['video', '--fs-tm-1'],
  ['image', '--fs-tm-2'],
  ['audio', '--fs-tm-3'],
  ['document', '--fs-tm-4'],
  ['archive', '--fs-tm-5'],
  ['code', '--fs-tm-6'],
  ['executable', '--fs-tm-7'],
  ['folder', '--fs-tm-8'],
  ['other', '--fs-tm-9'],
] as const;

/** Category keys with a dedicated colour. */
export type TreemapCategory = (typeof TREEMAP_PALETTE)[number][0];

/** Fallback token for an unknown category. */
export const TREEMAP_FALLBACK_TOKEN = '--fs-tm-9';

/** Returns the CSS custom property that paints `category`. */
export function treemapToken(category: string): string {
  const entry = TREEMAP_PALETTE.find(([key]) => key === category);

  return entry?.[1] ?? TREEMAP_FALLBACK_TOKEN;
}

/** Returns the colour used for the "other" aggregate, kept for tests. */
export function treemapLegend(): readonly (readonly [string, string])[] {
  return TREEMAP_PALETTE;
}

function luminance(hex: string): number {
  const value = hex.replace('#', '');
  if (!/^[0-9a-f]{6}$/i.test(value)) return 0.5;
  const channels = [0, 2, 4].map((offset) => {
    const channel = Number.parseInt(value.slice(offset, offset + 2), 16) / 255;
    return channel <= 0.03928
      ? channel / 12.92
      : ((channel + 0.055) / 1.055) ** 2.4;
  });
  const [red = 0, green = 0, blue = 0] = channels;

  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
}

/** WCAG contrast ratio between two `#rrggbb` colours. */
export function contrastRatio(first: string, second: string): number {
  const a = luminance(first);
  const b = luminance(second);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/**
 * Picks the ink with the higher contrast for a tile.
 *
 * Bright and dark category colours alternate between the two themes, so a
 * luminance threshold is not reliable; the ratio itself decides.
 */
export function pickTileInk(tile: string, darkInk: string, lightInk: string): string {
  return contrastRatio(tile, darkInk) >= contrastRatio(tile, lightInk)
    ? darkInk
    : lightInk;
}
