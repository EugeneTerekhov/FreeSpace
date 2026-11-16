import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { COMMANDS, EVENTS } from '../../src/api/transport';

interface CapabilityFixture {
  command: string;
  result: {
    commands: string[];
    events: string[];
    csp_version: number;
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
});
