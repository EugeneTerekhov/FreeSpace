#!/usr/bin/env node
// FreeSpace version helper.
//
// `[workspace.package].version` in the root Cargo.toml is the single source of
// truth. The Tauri configuration and the UI package files mirror it, and the
// release workflow additionally checks the requested git tag against it.
//
// Usage:
//   node scripts/version.mjs check
//   node scripts/version.mjs version
//   node scripts/version.mjs set 2.0.0
//   node scripts/version.mjs notes 2.0.0 [--output release-notes.md]
//
// Options:
//   --root <dir>   operate on another checkout (used by the tests)
//
// The script has no dependencies and uses only Node.js built-ins.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

export const DEFAULT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export const VERSION_FILES = {
  cargo: 'Cargo.toml',
  tauri: 'apps/freespace-desktop/src-tauri/tauri.conf.json',
  packageJson: 'apps/freespace-desktop/ui/package.json',
  packageLock: 'apps/freespace-desktop/ui/package-lock.json',
  changelog: 'CHANGELOG.md',
};

function projectPaths(root) {
  const at = (relative) => path.join(root, ...relative.split('/'));
  return {
    root,
    cargo: at(VERSION_FILES.cargo),
    tauri: at(VERSION_FILES.tauri),
    packageJson: at(VERSION_FILES.packageJson),
    packageLock: at(VERSION_FILES.packageLock),
    ui: at('apps/freespace-desktop/ui'),
    changelog: at(VERSION_FILES.changelog),
  };
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function readText(file, label) {
  try {
    return readFileSync(file, 'utf8');
  } catch (error) {
    throw new Error(`Cannot read ${label} (${file}): ${error.message}`);
  }
}

function readJson(file, label) {
  const text = readText(file, label);
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`Cannot parse ${label} (${file}): ${error.message}`);
  }
}

function writeJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}

// Returns the body of a TOML table between its header and the next table.
function tomlSectionBody(text, section) {
  const header = new RegExp(`^\\[${escapeRegExp(section)}\\][^\\r\\n]*$`, 'm');
  const match = header.exec(text);
  if (!match) {
    throw new Error(`Section [${section}] is missing from Cargo.toml`);
  }
  const start = match.index + match[0].length;
  const rest = text.slice(start);
  const next = /^\[/m.exec(rest);
  return next ? rest.slice(0, next.index) : rest;
}

export function workspaceVersionFromCargo(text) {
  const body = tomlSectionBody(text, 'workspace.package');
  const match = /^\s*version\s*=\s*"([^"]+)"/m.exec(body);
  if (!match) {
    throw new Error('Cargo.toml does not declare [workspace.package].version');
  }
  return match[1];
}

export function setWorkspaceVersion(text, version) {
  const body = tomlSectionBody(text, 'workspace.package');
  if (!/^\s*version\s*=\s*"[^"]*"/m.test(body)) {
    throw new Error('Cargo.toml does not declare [workspace.package].version');
  }
  const updatedBody = body.replace(/^(\s*version\s*=\s*")([^"]*)(")/m, `$1${version}$3`);
  const start = text.indexOf(body);
  return text.slice(0, start) + updatedBody + text.slice(start + body.length);
}

// Reads every mirrored version. `undefined` marks a missing value; only a
// Tauri configuration without a version field is skipped, everything else is
// reported as a mismatch.
export function readVersions(root = DEFAULT_ROOT) {
  const paths = projectPaths(root);

  const cargo = workspaceVersionFromCargo(readText(paths.cargo, VERSION_FILES.cargo));

  let tauri;
  if (existsSync(paths.tauri)) {
    const config = readJson(paths.tauri, VERSION_FILES.tauri);
    if (typeof config.version === 'string') {
      tauri = config.version;
    }
  }

  let packageJson;
  if (existsSync(paths.packageJson)) {
    const manifest = readJson(paths.packageJson, VERSION_FILES.packageJson);
    packageJson = manifest.version;
  }

  let packageLock;
  let packageLockRoot;
  if (existsSync(paths.packageLock)) {
    const lock = readJson(paths.packageLock, VERSION_FILES.packageLock);
    packageLock = lock.version;
    packageLockRoot = lock.packages?.['']?.version;
  }

  return { cargo, tauri, packageJson, packageLock, packageLockRoot };
}

// Returns the entries that disagree with the workspace version.
export function versionMismatches(versions) {
  const entries = [
    [VERSION_FILES.tauri, versions.tauri],
    [VERSION_FILES.packageJson, versions.packageJson],
    [VERSION_FILES.packageLock, versions.packageLock],
    [`${VERSION_FILES.packageLock} (packages[""])`, versions.packageLockRoot],
  ];
  return entries.filter(
    ([file, value]) =>
      !(file === VERSION_FILES.tauri && value === undefined) && value !== versions.cargo,
  );
}

export function checkVersions(root = DEFAULT_ROOT) {
  const versions = readVersions(root);
  const mismatches = versionMismatches(versions);
  return { versions, mismatches };
}

function updatePackageManifest(file, version) {
  const manifest = readJson(file, VERSION_FILES.packageJson);
  manifest.version = version;
  writeJson(file, manifest);
}

function updatePackageLock(file, version) {
  const lock = readJson(file, VERSION_FILES.packageLock);
  lock.version = version;
  if (lock.packages?.['']) {
    lock.packages[''].version = version;
  }
  writeJson(file, lock);
}

function runNpmVersion(uiDir, version) {
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const args = [
    'version',
    version,
    '--no-git-tag-version',
    '--allow-same-version',
    '--ignore-scripts',
  ];
  // Windows needs a shell to run `npm.cmd`; pass one command line there to keep
  // Node from warning about unescaped arguments on POSIX.
  const result =
    process.platform === 'win32'
      ? spawnSync([npm, ...args].join(' '), {
          cwd: uiDir,
          stdio: 'inherit',
          shell: true,
        })
      : spawnSync(npm, args, { cwd: uiDir, stdio: 'inherit' });
  if (result.error) {
    throw new Error(`Cannot run npm in ${uiDir}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`npm version ${version} failed with exit code ${result.status}`);
  }
}

export function setVersion(root, version, options = {}) {
  if (!SEMVER.test(version)) {
    throw new Error(`"${version}" is not a semantic version (expected MAJOR.MINOR.PATCH)`);
  }
  const paths = projectPaths(root);
  const runNpm = options.runNpm !== false;

  writeFileSync(
    paths.cargo,
    setWorkspaceVersion(readText(paths.cargo, VERSION_FILES.cargo), version),
    'utf8',
  );

  if (existsSync(paths.tauri)) {
    const config = readJson(paths.tauri, VERSION_FILES.tauri);
    config.version = version;
    writeJson(paths.tauri, config);
  }

  if (existsSync(paths.packageJson)) {
    if (runNpm) {
      runNpmVersion(paths.ui, version);
    } else {
      updatePackageManifest(paths.packageJson, version);
      if (existsSync(paths.packageLock)) {
        updatePackageLock(paths.packageLock, version);
      }
    }
  }

  const { versions, mismatches } = checkVersions(root);
  if (mismatches.length > 0) {
    const detail = mismatches.map(([file, value]) => `${file}=${String(value)}`).join(', ');
    throw new Error(`Versions are still inconsistent after set: ${detail}`);
  }
  return versions;
}

// Extracts the body of `## [<version>]` from CHANGELOG.md. Falls back to the
// `[Unreleased]` section so a release can be drafted before the changelog is
// renamed, and reports which heading was used.
export function changelogSection(markdown, version) {
  const lines = markdown.split(/\r?\n/);
  const isHeading = (line) => /^##\s+/.test(line);

  let start = lines.findIndex((line) =>
    new RegExp(`^##\\s+\\[${escapeRegExp(version)}\\]`).test(line),
  );
  let heading = version;

  if (start === -1) {
    start = lines.findIndex((line) => /^##\s+\[Unreleased\]/i.test(line));
    heading = 'Unreleased';
  }
  if (start === -1) {
    throw new Error(`CHANGELOG.md has no section for ${version} or [Unreleased]`);
  }

  let end = lines.findIndex((line, index) => index > start && isHeading(line));
  if (end === -1) {
    end = lines.length;
  }

  const body = lines.slice(start + 1, end).join('\n').trim();
  if (body === '') {
    throw new Error(`CHANGELOG.md section [${heading}] is empty`);
  }
  return { heading, body };
}

function parseArguments(argv) {
  const positional = [];
  const flags = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--root' || argument === '--output') {
      flags[argument.slice(2)] = argv[(index += 1)];
    } else if (argument.startsWith('--root=')) {
      flags.root = argument.slice('--root='.length);
    } else if (argument.startsWith('--output=')) {
      flags.output = argument.slice('--output='.length);
    } else {
      positional.push(argument);
    }
  }
  return { positional, flags };
}

const USAGE = `Usage:
  node scripts/version.mjs check
  node scripts/version.mjs version
  node scripts/version.mjs set <semver>
  node scripts/version.mjs notes <semver> [--output <file>]

Options:
  --root <dir>   operate on another checkout`;

function report(root) {
  const versions = readVersions(root);
  const mismatches = versionMismatches(versions);
  const labels = [
    [VERSION_FILES.cargo, versions.cargo],
    [VERSION_FILES.tauri, versions.tauri],
    [VERSION_FILES.packageJson, versions.packageJson],
    [VERSION_FILES.packageLock, versions.packageLock],
    [`${VERSION_FILES.packageLock} (packages[""])`, versions.packageLockRoot],
  ];
  for (const [label, value] of labels) {
    if (value !== undefined) {
      console.log(`${label}: ${value}`);
    }
  }
  if (mismatches.length === 0) {
    console.log(`OK: every version matches ${versions.cargo}`);
    return 0;
  }
  console.error(`Mismatch: expected ${versions.cargo} from ${VERSION_FILES.cargo}`);
  for (const [file, value] of mismatches) {
    console.error(`  ${file}: ${String(value)}`);
  }
  return 1;
}

function main(argv) {
  const { positional, flags } = parseArguments(argv);
  const command = positional[0];
  const root = flags.root ? path.resolve(flags.root) : DEFAULT_ROOT;

  try {
    switch (command) {
      case 'check':
        return report(root);
      case 'version': {
        const { cargo } = readVersions(root);
        console.log(cargo);
        return 0;
      }
      case 'set': {
        const version = positional[1];
        if (!version) {
          console.error('set requires a semantic version');
          return 2;
        }
        const versions = setVersion(root, version);
        console.log(`Updated every version to ${versions.cargo}`);
        return report(root);
      }
      case 'notes': {
        const version = positional[1];
        if (!version) {
          console.error('notes requires a semantic version');
          return 2;
        }
        const { heading, body } = changelogSection(readText(projectPaths(root).changelog, VERSION_FILES.changelog), version);
        if (heading !== version) {
          console.warn(`::warning::CHANGELOG.md has no [${version}] section; using [${heading}]`);
        }
        if (flags.output) {
          writeFileSync(path.resolve(flags.output), `${body}\n`, 'utf8');
          console.log(`Release notes for ${version} written to ${flags.output}`);
        } else {
          console.log(body);
        }
        return 0;
      }
      default:
        console.error(USAGE);
        return 2;
    }
  } catch (error) {
    console.error(`version: ${error.message}`);
    return 1;
  }
}

const entry = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : '';
if (import.meta.url === entry) {
  process.exitCode = main(process.argv.slice(2));
}
