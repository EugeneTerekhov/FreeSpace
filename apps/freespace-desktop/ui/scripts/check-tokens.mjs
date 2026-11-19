#!/usr/bin/env node
// Rejects hard-coded colours outside the token and theme stylesheets.
//
// Components must consume CSS custom properties from `src/styles/tokens.css`
// and `src/styles/themes.css`; this keeps both themes, contrast and future
// palette changes in one place.

import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const srcDir = resolve(scriptDir, '../src');

const ALLOWED = new Set([
  ['styles', 'tokens.css'].join(sep),
  ['styles', 'themes.css'].join(sep),
]);
const IGNORED_DIRECTORIES = new Set(['i18n/generated']);
const HEX_COLOUR = /#[0-9a-fA-F]{3,8}\b/g;
const COLOUR_FUNCTION = /\b(?:rgba?|hsla?|oklch|lab|lch)\(/g;
const EXTENSIONS = new Set(['.css', '.ts', '.tsx']);

/** Recursively lists source files. */
function walk(directory) {
  const entries = readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = join(directory, entry.name);

    if (entry.isDirectory()) {
      const rel = relative(srcDir, absolute).split(sep).join('/');
      return IGNORED_DIRECTORIES.has(rel) ? [] : walk(absolute);
    }

    return entry.isFile() ? [absolute] : [];
  });

  return entries;
}

const violations = [];

for (const file of walk(srcDir)) {
  const relativePath = relative(srcDir, file);

  if (ALLOWED.has(relativePath) || relativePath.startsWith('i18n/generated')) {
    continue;
  }

  if (!EXTENSIONS.has(relativePath.slice(relativePath.lastIndexOf('.')))) {
    continue;
  }

  const lines = readFileSync(file, 'utf8').split(/\r?\n/);

  lines.forEach((line, index) => {
    const hex = line.match(HEX_COLOUR);
    const functions = line.match(COLOUR_FUNCTION);

    if (hex !== null || functions !== null) {
      violations.push(
        `${relative(srcDir, file)}:${index + 1}: hard-coded colour (${[...(hex ?? []), ...(functions ?? [])].join(', ')})`,
      );
    }
  });
}

if (violations.length > 0) {
  console.error(violations.join('\n'));
  console.error('use CSS custom properties from src/styles/tokens.css instead');
  process.exitCode = 1;
}
