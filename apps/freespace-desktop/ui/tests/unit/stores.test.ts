import { beforeEach, describe, expect, it } from 'vitest';

import { ApiClient } from '../../src/api/client';
import { MockTransport } from '../../src/api/mockTransport';
import type { UiSettings } from '../../src/api/dto';
import { useSettingsStore, applySettingsToDocument, resolvedTheme } from '../../src/stores/settingsStore';
import { useShellStore } from '../../src/stores/shellStore';
import { useSnapshotStore } from '../../src/stores/snapshotStore';
import { useTasksStore } from '../../src/stores/tasksStore';

function settings(overrides: Partial<UiSettings> = {}): UiSettings {
  return {
    theme: 'dark',
    language: 'ru',
    density: 'comfortable',
    units: 'binary',
    reduced_motion: 'system',
    last_screen: 'dashboard',
    sidebar_collapsed: false,
    column_layouts: {},
    saved_filters: {},
    schema_version: 1,
    ...overrides,
  };
}

beforeEach(() => {
  document.documentElement.removeAttribute('data-theme');
  document.documentElement.removeAttribute('data-density');
  document.documentElement.removeAttribute('data-reduced-motion');
});

describe('settings store', () => {
  it('resolves the system theme', () => {
    expect(resolvedTheme('system', true)).toBe('dark');
    expect(resolvedTheme('system', false)).toBe('light');
    expect(resolvedTheme('light', true)).toBe('light');
    expect(resolvedTheme('dark', false)).toBe('dark');
  });

  it('applies theme, density, language and motion to the document', () => {
    applySettingsToDocument(
      settings({ theme: 'light', density: 'compact', language: 'en', reduced_motion: 'on' }),
    );

    const root = document.documentElement;
    expect(root.dataset.theme).toBe('light');
    expect(root.dataset.density).toBe('compact');
    expect(root.dataset.reducedMotion).toBe('true');
    expect(root.lang).toBe('en');
  });

  it('persists a patch through the transport', async () => {
    const client = new ApiClient(new MockTransport());
    await useSettingsStore.getState().save(client, { theme: 'light' });

    expect(useSettingsStore.getState().settings?.theme).toBe('light');
    expect(document.documentElement.dataset.theme).toBe('light');
  });
});

describe('shell and tasks stores', () => {
  it('loads the bootstrap and restores the last screen', async () => {
    const transport = new MockTransport();
    transport.settings = settings({ last_screen: 'tree' });
    const client = new ApiClient(transport);

    await useShellStore.getState().load(client);

    const state = useShellStore.getState();
    expect(state.ready).toBe(true);
    expect(state.error).toBeNull();
    expect(state.screen).toBe('tree');
    expect(state.bootstrap?.capabilities.commands.length).toBeGreaterThan(20);
  });

  it('tracks task progress and terminal states', () => {
    const store = useTasksStore.getState();
    store.upsert({
      task_id: 't1',
      kind: 'scan',
      state: 'running',
      volume_id: null,
      progress: null,
      started_at: 1,
      finished_at: null,
      cancelable: true,
      error: null,
    });
    store.applyProgress('t1', {
      processed: 10,
      total: 20,
      current_path: 'C:\\',
      bytes_per_second: null,
      items_per_second: null,
      phase_key: 'scan.phase.enumerating',
      updated_at: 2,
    });
    expect(useTasksStore.getState().tasks[0]?.progress?.processed).toBe(10);

    store.updateState('t1', 'completed', true);
    expect(useTasksStore.getState().tasks[0]?.state).toBe('completed');
    store.clearFinished();
    expect(useTasksStore.getState().tasks).toHaveLength(0);
  });

  it('keeps the snapshot generation', () => {
    useSnapshotStore.getState().setSnapshot(4, null);
    expect(useSnapshotStore.getState().generation).toBe(4);
    useSnapshotStore.getState().reset();
    expect(useSnapshotStore.getState().generation).toBeNull();
  });
});

describe('error logging', () => {
  it('keeps local log lines', async () => {
    const { logLocal, recentLogs } = await import('../../src/app/errors');
    logLocal('error', 'interface test');
    expect(recentLogs().at(-1)).toContain('interface test');
  });
});
