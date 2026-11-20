import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { COMMANDS, EVENTS } from '../../src/api/transport';
import type { PlatformCapabilities } from '../../src/api/dto';

interface CapabilityFixture {
  command: string;
  result: {
    commands: string[];
    events: string[];
    csp_version: number;
    platform_capabilities: PlatformCapabilities;
  };
}

const fixture = JSON.parse(
  readFileSync(
    resolve(
      process.cwd(),
      '../../../tests/contract/fixtures/app_capabilities.success.json',
    ),
    'utf8',
  ),
) as CapabilityFixture;

describe('IPC contract', () => {
  it('matches the Rust capability list', () => {
    expect(fixture.command).toBe('app_capabilities');
    expect([...fixture.result.commands].sort()).toEqual([...COMMANDS].sort());
    expect([...fixture.result.events].sort()).toEqual([...EVENTS].sort());
    expect(fixture.result.csp_version).toBe(1);
  });

  it('uses snake_case command and kebab-case event names', () => {
    for (const command of COMMANDS) {
      expect(command).toMatch(/^[a-z][a-z0-9_]*$/);
    }
    for (const event of EVENTS) {
      expect(event).toMatch(/^[a-z][a-z0-9-]*$/);
    }
  });

  it('advertises the platform capabilities of the backend', () => {
    const platform = fixture.result.platform_capabilities;

    expect(platform.platform).toBe('windows');
    expect(platform.scan_methods).toEqual({ mft: true, usn: true, walk: true });
    expect(platform.trash).toBe(true);
    expect(platform.permanent).toBe(true);
    expect(platform.shred).toBe(true);
    expect(platform.acl).toBe(true);
    expect(platform.attrs).toBe(true);
    expect(platform.ads).toBe(true);
    expect(platform.unlock).toBe(true);
    expect(platform.context_menu).toBe(true);
    expect(platform.monitor).toBe(true);
    expect(platform.elevation).toBe(true);
    expect(platform.long_paths).toBe(true);
    expect(platform.case_sensitive).toBe(false);
    expect(platform.max_path).toBeNull();
  });
});
