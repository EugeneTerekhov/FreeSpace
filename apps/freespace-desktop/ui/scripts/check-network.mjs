#!/usr/bin/env node
// Network guard: FreeSpace is fully offline.
//
// The script rejects remote URLs in the frontend sources and web assets, a CSP
// that enables anything besides `self` and the local IPC endpoint, and Tauri
// capability permissions for the fs/shell/http plugins.

import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const uiDir = resolve(scriptDir, '..');
const tauriDir = resolve(uiDir, '../src-tauri');

const ALLOWED_URLS = [
  'http://ipc.localhost',
  'http://127.0.0.1:1420',
  'http://localhost:1420',
];
const REMOTE_URL = /\b(?:https?|wss?|ftp):\/\/[^\s"'`)]+/g;
// XML namespace declarations are identifiers, not fetches; the data URI chevron
// in themes.css embeds one and must not fail the offline guard.
const XML_NAMESPACE = /\bxmlns\s*=\s*(["'])[^"']*\1/g;
const FORBIDDEN_PERMISSIONS = /(?:^|["':])(?:fs|shell|http)[:.]/;
const TEXT_EXTENSIONS = new Set(['.css', '.html', '.js', '.mjs', '.ts', '.tsx']);

/** Recursively lists files of the given directories. */
function walk(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const absolute = join(directory, entry.name);
    return entry.isDirectory() ? walk(absolute) : [absolute];
  });
}

const violations = [];
const scanned = [
  ...walk(join(uiDir, 'src')),
  ...walk(join(uiDir, 'public')),
  join(uiDir, 'index.html'),
];

for (const file of scanned) {
  if (!TEXT_EXTENSIONS.has(file.slice(file.lastIndexOf('.')))) {
    continue;
  }

  const text = readFileSync(file, 'utf8').replace(XML_NAMESPACE, '');
  for (const match of text.matchAll(REMOTE_URL)) {
    const url = match[0];

    if (ALLOWED_URLS.some((allowed) => url.startsWith(allowed))) {
      continue;
    }

    violations.push(`${relative(uiDir, file)}: remote URL ${url}`);
  }
}

const configPath = join(tauriDir, 'tauri.conf.json');
const config = JSON.parse(readFileSync(configPath, 'utf8'));
const csp = config.app?.security?.csp ?? '';

if (!csp.includes("default-src 'self'")) {
  violations.push('tauri.conf.json: CSP must start from default-src \'self\'');
}

for (const match of String(csp).matchAll(REMOTE_URL)) {
  if (!ALLOWED_URLS.some((allowed) => match[0].startsWith(allowed))) {
    violations.push(`tauri.conf.json: remote CSP source ${match[0]}`);
  }
}

if (config.app?.withGlobalTauri === true) {
  violations.push('tauri.conf.json: withGlobalTauri must be disabled');
}

const capabilitiesDir = join(tauriDir, 'capabilities');
for (const file of walk(capabilitiesDir)) {
  if (!file.endsWith('.json')) {
    continue;
  }

  const text = readFileSync(file, 'utf8');

  if (FORBIDDEN_PERMISSIONS.test(text)) {
    violations.push(`${relative(tauriDir, file)}: fs/shell/http permission`);
  }
}

if (violations.length > 0) {
  console.error(violations.join('\n'));
  process.exitCode = 1;
}
