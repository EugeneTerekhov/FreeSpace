// Tests for scripts/version.mjs. Run with: node --test scripts/version.test.mjs
//
// Every test builds an isolated copy of the version-bearing files in a
// temporary directory, so the real checkout is never modified.

import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
  changelogSection,
  readVersions,
  setVersion,
  setWorkspaceVersion,
  versionMismatches,
  workspaceVersionFromCargo,
} from './version.mjs';

const CARGO_TOML = `[workspace]
resolver = "3"
members = ["crates/freespace-cli"]

[workspace.package]
version = "2.0.0"
edition = "2024"
rust-version = "1.85"

[workspace.dependencies]
# A dependency version must never be mistaken for the workspace version.
freespace-core = { path = "crates/freespace-core", version = "9.9.9" }
`;

function tauriConfig(version = '2.0.0') {
  return {
    $schema: 'https://schema.tauri.app/config/2',
    productName: 'FreeSpace',
    version,
    identifier: 'com.freespace.desktop',
    build: { frontendDist: '../ui/dist' },
    bundle: { active: false, icon: ['icons/icon.ico'] },
  };
}

function packageManifest(version = '2.0.0') {
  return {
    name: 'freespace-desktop-ui',
    private: true,
    version,
    type: 'module',
  };
}

function packageLock(version = '2.0.0') {
  return {
    name: 'freespace-desktop-ui',
    version,
    lockfileVersion: 3,
    requires: true,
    packages: {
      '': { name: 'freespace-desktop-ui', version },
    },
  };
}

function writeProject(root, versions = {}) {
  const {
    cargo = '2.0.0',
    tauri = cargo,
    manifest = cargo,
    lock = cargo,
  } = versions;

  mkdirSync(path.join(root, 'apps', 'freespace-desktop', 'src-tauri'), { recursive: true });
  mkdirSync(path.join(root, 'apps', 'freespace-desktop', 'ui'), { recursive: true });
  writeFileSync(
    path.join(root, 'Cargo.toml'),
    cargo === '2.0.0' ? CARGO_TOML : CARGO_TOML.replace('version = "2.0.0"', `version = "${cargo}"`),
  );
  writeFileSync(
    path.join(root, 'apps', 'freespace-desktop', 'src-tauri', 'tauri.conf.json'),
    `${JSON.stringify(tauriConfig(tauri), null, 2)}\n`,
  );
  writeFileSync(
    path.join(root, 'apps', 'freespace-desktop', 'ui', 'package.json'),
    `${JSON.stringify(packageManifest(manifest), null, 2)}\n`,
  );
  writeFileSync(
    path.join(root, 'apps', 'freespace-desktop', 'ui', 'package-lock.json'),
    `${JSON.stringify(packageLock(lock), null, 2)}\n`,
  );
  writeFileSync(path.join(root, 'CHANGELOG.md'), CHANGELOG);
}

const CHANGELOG = `# Changelog

## [Unreleased]

### Added

- Work in progress.

## [2.0.0] - 2026-11-19

### Added

- Linux portable archive.

### Removed

- Quarantine.

## [1.0.0] - 2026-01-01

### Added

- First release.
`;

function withProject(t, versions) {
  const root = mkdtempSync(path.join(tmpdir(), 'freespace-version-'));
  writeProject(root, versions);
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return root;
}

test('reads the workspace version and ignores dependency versions', () => {
  assert.equal(workspaceVersionFromCargo(CARGO_TOML), '2.0.0');
});

test('accepts a project where every version matches', (t) => {
  const root = withProject(t);
  const versions = readVersions(root);
  assert.deepEqual(versions, {
    cargo: '2.0.0',
    tauri: '2.0.0',
    packageJson: '2.0.0',
    packageLock: '2.0.0',
    packageLockRoot: '2.0.0',
  });
  assert.deepEqual(versionMismatches(versions), []);
});

test('detects a Tauri configuration mismatch', (t) => {
  const root = withProject(t, { tauri: '1.3.0' });
  const mismatches = versionMismatches(readVersions(root));
  assert.equal(mismatches.length, 1);
  assert.equal(mismatches[0][0], 'apps/freespace-desktop/src-tauri/tauri.conf.json');
  assert.equal(mismatches[0][1], '1.3.0');
});

test('detects a package-lock mismatch, including packages[""]', (t) => {
  const root = withProject(t, { lock: '1.3.0' });
  const mismatches = versionMismatches(readVersions(root));
  assert.equal(mismatches.length, 2);
  for (const [file, value] of mismatches) {
    assert.match(file, /package-lock\.json/);
    assert.equal(value, '1.3.0');
  }
});

test('detects a missing version instead of silently ignoring it', (t) => {
  const root = withProject(t);
  const file = path.join(root, 'apps', 'freespace-desktop', 'ui', 'package.json');
  const manifest = JSON.parse(readFileSync(file, 'utf8'));
  delete manifest.version;
  writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`);
  const mismatches = versionMismatches(readVersions(root));
  assert.equal(mismatches.length, 1);
  assert.equal(mismatches[0][0], 'apps/freespace-desktop/ui/package.json');
  assert.equal(mismatches[0][1], undefined);
});

test('setWorkspaceVersion preserves the rest of Cargo.toml', () => {
  const updated = setWorkspaceVersion(CARGO_TOML, '2.1.0');
  assert.match(updated, /\[workspace\.package\]\nversion = "2\.1\.0"/);
  assert.match(updated, /freespace-core = \{ path = "crates\/freespace-core", version = "9\.9\.9" \}/);
  assert.equal(workspaceVersionFromCargo(updated), '2.1.0');
});

test('setVersion synchronises Cargo.toml, Tauri and the UI package files', (t) => {
  const root = withProject(t);
  const versions = setVersion(root, '2.1.0', { runNpm: false });
  assert.equal(versions.cargo, '2.1.0');
  assert.deepEqual(versionMismatches(readVersions(root)), []);
  assert.match(readFileSync(path.join(root, 'Cargo.toml'), 'utf8'), /version = "2\.1\.0"/);
});

test('setVersion rejects a non-semantic version', (t) => {
  const root = withProject(t);
  assert.throws(() => setVersion(root, '2.1', { runNpm: false }), /not a semantic version/);
  assert.equal(readVersions(root).cargo, '2.0.0');
});

test('setVersion delegates the UI files to npm version', (t) => {
  const probe =
    process.platform === 'win32'
      ? spawnSync('npm.cmd --version', { stdio: 'ignore', shell: true })
      : spawnSync('npm', ['--version'], { stdio: 'ignore' });
  if (probe.error || probe.status !== 0) {
    t.skip('npm is not available');
    return;
  }

  const root = withProject(t);
  const versions = setVersion(root, '2.2.0');
  assert.equal(versions.cargo, '2.2.0');
  assert.equal(versions.packageJson, '2.2.0');
  assert.equal(versions.packageLock, '2.2.0');
  assert.equal(versions.packageLockRoot, '2.2.0');
  assert.deepEqual(versionMismatches(readVersions(root)), []);
});

test('changelogSection extracts only the requested version', () => {
  const { heading, body } = changelogSection(CHANGELOG, '2.0.0');
  assert.equal(heading, '2.0.0');
  assert.match(body, /Linux portable archive/);
  assert.doesNotMatch(body, /First release/);
  assert.doesNotMatch(body, /Work in progress/);
});

test('changelogSection falls back to Unreleased', () => {
  const { heading, body } = changelogSection(CHANGELOG, '3.0.0');
  assert.equal(heading, 'Unreleased');
  assert.match(body, /Work in progress/);
});

test('changelogSection fails when the version is missing everywhere', () => {
  assert.throws(
    () => changelogSection('# Changelog\n\n## [1.0.0] - 2026-01-01\n\n- Old.\n', '2.0.0'),
    /no section for 2\.0\.0/,
  );
});
