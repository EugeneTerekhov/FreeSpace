import { afterEach, describe, expect, it, vi } from 'vitest';

import { createMockTransport } from '../../src/api/mockTransport';
import type { EventEnvelope, ScanCompletedEvent, ScanProgressEvent, ScanStarted } from '../../src/api/dto';

afterEach(() => {
  vi.useRealTimers();
});

describe('MockTransport', () => {
  it('answers volume and tree commands with realistic data', async () => {
    const transport = createMockTransport();

    const volumes = await transport.invoke<{ mount_point: string }[]>('volumes_list');
    expect(volumes).toHaveLength(2);
    expect(volumes[0]?.mount_point).toBe('C:\\');

    const root = await transport.invoke<{ rows: { name: string }[] }>('tree_root', {
      generation: 1,
    });
    expect(root.rows.map((row) => row.name)).toEqual(['C:\\']);

    const children = await transport.invoke<{ rows: { name: string }[] }>('tree_children', {
      generation: 1,
      parent_id: 0,
    });
    expect(children.rows.map((row) => row.name)).toContain('Windows');
  });

  it('runs a simulated scan with monotonic progress and a final generation', async () => {
    vi.useFakeTimers();
    const transport = createMockTransport();
    const progress: EventEnvelope<ScanProgressEvent>[] = [];
    const completed: EventEnvelope<ScanCompletedEvent>[] = [];
    await transport.listen<ScanProgressEvent>('scan-progress', (event) => progress.push(event));
    await transport.listen<ScanCompletedEvent>('scan-completed', (event) => completed.push(event));

    const started = await transport.invoke<ScanStarted>('scan_start', { path: 'C:\\' });
    vi.advanceTimersByTime(1_000);
    await vi.runOnlyPendingTimersAsync();

    expect(progress.length).toBeGreaterThanOrEqual(4);
    for (let index = 1; index < progress.length; index += 1) {
      expect(progress[index]?.sequence ?? 0).toBeGreaterThan(
        progress[index - 1]?.sequence ?? 0,
      );
    }
    expect(completed).toHaveLength(1);
    expect(completed[0]?.payload.generation).toBe(started.generation);
    expect(transport.generation).toBe(started.generation);
  });

  it('records every volume of a multi-path scan for snapshot_volumes', async () => {
    const transport = createMockTransport();

    await transport.invoke<ScanStarted>('scan_start', {
      request: { paths: ['C:\\', 'D:\\'] },
    });
    const volumes = await transport.invoke<{ mount_point: string }[]>(
      'snapshot_volumes',
      { generation: 2 },
    );

    expect(volumes.map((volume) => volume.mount_point)).toEqual(['C:\\', 'D:\\']);

    const root = await transport.invoke<{ rows: { name: string }[] }>('tree_root', {
      generation: 2,
    });
    expect(root.rows.map((row) => row.name)).toEqual(
      expect.arrayContaining(['C:\\', 'D:\\']),
    );
  });

  it('cancels a running scan', async () => {
    vi.useFakeTimers();
    const transport = createMockTransport();
    const completed: EventEnvelope<ScanCompletedEvent>[] = [];
    await transport.listen<ScanCompletedEvent>('scan-completed', (event) => completed.push(event));

    const started = await transport.invoke<ScanStarted>('scan_start', { path: 'C:\\' });
    await transport.invoke('scan_cancel', { task_id: started.task_id });

    expect(completed[0]?.payload.state).toBe('cancelled');
    const task = await transport.invoke<{ state: string }>('scan_status', {
      task_id: started.task_id,
    });
    expect(task.state).toBe('cancelled');
  });

  it('propagates a UiError envelope', async () => {
    const transport = createMockTransport().failCommand('volumes_list', {
      code: 'E_IO',
      category: 'io',
      message_key: 'error.io',
      details: 'disk gone',
      retryable: true,
      task_id: null,
      journal_entry_id: null,
    });

    await expect(transport.invoke('volumes_list')).rejects.toMatchObject({
      code: 'E_IO',
      details: 'disk gone',
    });
  });
});
