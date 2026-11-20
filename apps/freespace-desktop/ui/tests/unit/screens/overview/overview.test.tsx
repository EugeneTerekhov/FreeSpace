import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UiError } from '../../../../src/api/dto';
import type { CommandName } from '../../../../src/api/transport';
import { t } from '../../../../src/i18n';
import { Overview } from '../../../../src/screens/Overview';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { useTasksStore } from '../../../../src/stores/tasksStore';
import { renderWithI18n } from '../../../helpers/render';
import { sharedTransport } from '../../../helpers/mockTransport';

const GB = 1024 * 1024 * 1024;

const volume = {
  volume_id: '11111111-1111-1111-1111-111111111111',
  mount_point: 'C:\\',
  label: 'System',
  file_system: 'ntfs',
  total_bytes: 1024 * GB,
  free_bytes: 512 * GB,
  used_bytes: 512 * GB,
  is_removable: false,
  status: 'available',
  scan_method: 'mft',
  is_elevated: true,
};

type Transport = ReturnType<typeof sharedTransport>;

function interceptInvoke(
  transport: Transport,
  intercept: (
    command: CommandName,
    args: Record<string, unknown> | undefined,
  ) => Promise<unknown> | null,
): void {
  const original = transport.invoke.bind(transport);
  vi.spyOn(transport, 'invoke').mockImplementation(
    <T,>(command: CommandName, args?: Record<string, unknown>): Promise<T> => {
      const intercepted = intercept(command, args);
      if (intercepted !== null) return intercepted as Promise<T>;

      return original(command, args);
    },
  );
}

function failure(code: UiError['code'] = 'E_ACCESS_DENIED'): UiError {
  return {
    code,
    category: 'access',
    message_key: 'error.access_denied',
    details: null,
    retryable: true,
    task_id: null,
    journal_entry_id: null,
  };
}

beforeEach(() => {
  useSnapshotStore.setState({ generation: null, volume: null, volumes: [] });
});

afterEach(() => {
  useTasksStore.setState({ tasks: [] });
  vi.restoreAllMocks();
});

describe('overview screen', () => {
  it('shows mounts and totals for every volume', async () => {
    renderWithI18n(<Overview />);

    expect(await screen.findByText('C', { exact: true })).toBeInTheDocument();
    expect(screen.getByText('System')).toBeInTheDocument();
    expect(screen.getByText(t('dashboard.total'))).toBeInTheDocument();
    expect(screen.getByText(t('dashboard.free'))).toBeInTheDocument();

    const card = screen.getByRole('button', {
      name: /^C\b/,
    });
    expect(card).toHaveAttribute('aria-pressed', 'false');
  });

  it('selects and deselects a volume by clicking its block', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Overview />);

    const card = await screen.findByRole('button', {
      name: /^C\b/,
    });
    await user.click(card);
    expect(card).toHaveAttribute('aria-pressed', 'true');
    expect(
      screen.getByText(t('overview.selected.title', { list: 'C' })),
    ).toBeInTheDocument();

    // The keyboard toggles the same selection.

    card.focus();
    await user.keyboard(' ');
    expect(card).toHaveAttribute('aria-pressed', 'false');
    expect(
      screen.queryByText(t('overview.selected.title', { list: 'C' })),
    ).not.toBeInTheDocument();
  });

  it('offers the analyze action once a volume is selected after a finished scan', async () => {
    const task = {
      task_id: 'finished-task',
      kind: 'scan' as const,
      state: 'completed' as const,
      volume_id: volume.volume_id,
      volume_ids: [volume.volume_id],
      progress: {
        processed: 15_125,
        total: 15_125,
        current_path: 'C:\\',
        bytes_per_second: null,
        items_per_second: null,
        phase_key: 'scan.phase.done',
        updated_at: Date.now(),
      },
      started_at: Date.now(),
      finished_at: Date.now(),
      cancelable: false,
      error: null,
    };

    const transport = sharedTransport();
    const user = userEvent.setup();
    interceptInvoke(transport, (command) =>
      command === 'scan_status' ? Promise.resolve(task) : null,
    );
    useTasksStore.setState({ tasks: [task] });

    renderWithI18n(<Overview />);

    expect(await screen.findByText(t('scan.completed'))).toBeInTheDocument();
    const card = screen.getByRole('button', {
      name: /^C\b/,
    });
    await user.click(card);
    expect(
      screen.getByRole('button', {
        name: t('overview.selected.start'),
      }),
    ).toBeEnabled();
  });

  it('filters the volume grid and shows an empty state without matches', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Overview />);

    await screen.findByText('C', { exact: true });
    await user.type(screen.getByRole('searchbox', { name: t('overview.search') }), 'zzz');

    expect(await screen.findByText(t('overview.no_matches'))).toBeInTheDocument();
  });

  it('starts a scan and shows the embedded progress with the current path', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Overview />);

    await user.click(
      await screen.findByRole('button', {
        name: /^C\b/,
      }),
    );
    await user.click(
      screen.getByRole('button', {
        name: t('overview.selected.start'),
      }),
    );

    await waitFor(() => {
      expect(screen.getByRole('progressbar')).toBeInTheDocument();
    });
    expect(screen.getByText(t('overview.scan.title'))).toBeInTheDocument();
  });

  it('renders the current analysis below the volume grid', async () => {
    const task = {
      task_id: 'below-task',
      kind: 'scan' as const,
      state: 'completed' as const,
      volume_id: volume.volume_id,
      volume_ids: [volume.volume_id],
      progress: null,
      started_at: Date.now(),
      finished_at: Date.now(),
      cancelable: false,
      error: null,
    };

    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'scan_status' ? Promise.resolve(task) : null,
    );
    useTasksStore.setState({ tasks: [task] });

    renderWithI18n(<Overview />);

    const grid = await screen.findByRole('region', { name: t('dashboard.volumes') });
    const title = screen.getByText(t('overview.scan.title'));
    expect(
      grid.compareDocumentPosition(title) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });

  it('starts one scan for several selected volumes', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    let request: Record<string, unknown> | undefined;
    interceptInvoke(transport, (command, args) => {
      if (command === 'scan_start') {
        request = args?.request as Record<string, unknown> | undefined;
        return Promise.resolve({ task_id: 'multi-scan', generation: 2 });
      }
      if (command === 'scan_status') {
        return Promise.resolve({
          task_id: 'multi-scan',
          kind: 'scan',
          state: 'completed',
          volume_id: volume.volume_id,
          volume_ids: [volume.volume_id, '22222222-2222-2222-2222-222222222222'],
          progress: null,
          started_at: Date.now(),
          finished_at: Date.now(),
          cancelable: false,
          error: null,
        });
      }

      return null;
    });

    renderWithI18n(<Overview />);
    await user.click(
      await screen.findByRole('button', {
        name: /^C\b/,
      }),
    );
    await user.click(
      screen.getByRole('button', {
        name: /^D\b/,
      }),
    );
    expect(
      screen.getByText(t('overview.selected.title', { list: 'C, D' })),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole('button', {
        name: t('overview.selected.start'),
      }),
    );

    await waitFor(() => {
      expect(request?.paths).toEqual(['C:\\', 'D:\\']);
    });
    expect(request?.method).toBe('auto');
    expect(request?.exclusions).toEqual([]);
    expect(request?.merge).toBe(true);
  });

  it('asks the backend to merge a new volume into the current snapshot', async () => {
    useSnapshotStore.setState({ generation: 1, volume, volumes: [volume] });

    const user = userEvent.setup();
    const transport = sharedTransport();
    let request: Record<string, unknown> | undefined;
    interceptInvoke(transport, (command, args) => {
      if (command === 'scan_start') {
        request = args?.request as Record<string, unknown> | undefined;
        return Promise.resolve({ task_id: 'add-scan', generation: 2 });
      }
      if (command === 'scan_status') {
        return Promise.resolve({
          task_id: 'add-scan',
          kind: 'scan',
          state: 'completed',
          volume_id: volume.volume_id,
          volume_ids: [volume.volume_id, '22222222-2222-2222-2222-222222222222'],
          progress: null,
          started_at: Date.now(),
          finished_at: Date.now(),
          cancelable: false,
          error: null,
        });
      }

      return null;
    });

    renderWithI18n(<Overview />);
    await user.click(
      await screen.findByRole('button', {
        name: /^D\b/,
      }),
    );
    await user.click(
      screen.getByRole('button', {
        name: t('overview.selected.start'),
      }),
    );

    await waitFor(() => {
      expect(request?.paths).toEqual(['D:\\']);
    });
    expect(request?.merge).toBe(true);
  });

  it('applies the scan type chosen next to the selection action', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    let request: Record<string, unknown> | undefined;
    interceptInvoke(transport, (command, args) => {
      if (command === 'scan_start') {
        request = args?.request as Record<string, unknown> | undefined;
        return Promise.resolve({ task_id: 'options-scan', generation: 2 });
      }
      if (command === 'scan_status') {
        return Promise.resolve({
          task_id: 'options-scan',
          kind: 'scan',
          state: 'completed',
          volume_id: volume.volume_id,
          volume_ids: [volume.volume_id],
          progress: null,
          started_at: Date.now(),
          finished_at: Date.now(),
          cancelable: false,
          error: null,
        });
      }

      return null;
    });

    renderWithI18n(<Overview />);

    expect(screen.queryByText(t('overview.scan.settings'))).not.toBeInTheDocument();

    await user.click(
      await screen.findByRole('button', {
        name: /^C\b/,
      }),
    );
    await user.selectOptions(screen.getByLabelText(t('scan.method')), 'walk');
    await user.click(
      screen.getByRole('button', {
        name: t('overview.selected.start'),
      }),
    );

    await waitFor(() => {
      expect(request?.method).toBe('walk');
    });
    expect(request?.exclusions).toEqual([]);
  });

  it('renders a recoverable error when volumes cannot be read', async () => {
    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'volumes_refresh'
        ? new Promise<never>((_resolve, reject) => {
            // The transport rejects with the typed error envelope, not a JS Error.
            // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors
            reject(failure());
          })
        : null,
    );

    renderWithI18n(<Overview />);

    expect(await screen.findByText(t('error.access_denied'))).toBeInTheDocument();
    expect(screen.getByRole('button', { name: t('error.retry') })).toBeInTheDocument();
  });

  it('marks a partial scan and offers the tree', async () => {
    const task = {
      task_id: 'partial-task',
      kind: 'scan' as const,
      state: 'partial' as const,
      volume_id: volume.volume_id,
      volume_ids: [volume.volume_id],
      progress: {
        processed: 120,
        total: 200,
        current_path: 'C:\\Users',
        bytes_per_second: 1024,
        items_per_second: null,
        phase_key: 'scan.phase.walking',
        updated_at: Date.now(),
      },
      started_at: Date.now(),
      finished_at: Date.now(),
      cancelable: false,
      error: null,
    };

    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'scan_status' ? Promise.resolve(task) : null,
    );
    useTasksStore.setState({ tasks: [task] });

    renderWithI18n(<Overview />);

    expect(await screen.findByText(t('overview.scan.partial'))).toBeInTheDocument();
    expect(screen.getByText('1 KiB/s')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: t('scan.view_tree') })).toBeInTheDocument();
  });

  it('omits the speed counter until the scanner reports a rate', async () => {
    const task = {
      task_id: 'no-speed-task',
      kind: 'scan' as const,
      state: 'completed' as const,
      volume_id: volume.volume_id,
      volume_ids: [volume.volume_id],
      progress: {
        processed: 10,
        total: null,
        current_path: null,
        bytes_per_second: null,
        items_per_second: null,
        phase_key: 'scan.phase.preparing',
        updated_at: Date.now(),
      },
      started_at: Date.now(),
      finished_at: Date.now(),
      cancelable: false,
      error: null,
    };

    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'scan_status' ? Promise.resolve(task) : null,
    );
    useTasksStore.setState({ tasks: [task] });

    renderWithI18n(<Overview />);

    expect(await screen.findByText(t('overview.scan.title'))).toBeInTheDocument();
    expect(screen.queryByText(/\/s$/)).toBeNull();
  });
});
