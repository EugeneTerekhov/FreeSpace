import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  CleanupCandidateView,
  FilterSet,
  SnapshotDiffPage,
  SnapshotSummary,
  UiError,
} from '../../../../src/api/dto';
import type { CommandName } from '../../../../src/api/transport';
import { t } from '../../../../src/i18n';
import { formatBytes } from '../../../../src/i18n/format';
import { Cleanup } from '../../../../src/screens/Cleanup';
import { Snapshots } from '../../../../src/screens/Snapshots';
import { useCleanupStore } from '../../../../src/stores/cleanupStore';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { renderWithI18n } from '../../../helpers/render';
import { sharedTransport } from '../../../helpers/mockTransport';

const GB = 1024 * 1024 * 1024;
const MB = 1024 * 1024;

const volume = {
  volume_id: '11111111-1111-1111-1111-111111111111',
  mount_point: 'C:\\',
  label: 'System',
  file_system: 'ntfs',
  total_bytes: 1024,
  free_bytes: 512,
  used_bytes: 512,
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

/** Value of the `Stat` block carrying the given label. */
function statValue(label: string): string {
  const labelNode = screen.getByText(label);

  return labelNode.nextElementSibling?.textContent ?? '';
}

const candidates: CleanupCandidateView[] = [
  {
    node_id: 30,
    path: 'C:\\Temp\\Freespace-cache',
    name: 'Freespace-cache',
    category: 'cache',
    reason_key: 'cleanup.reason.cache',
    reclaimable_bytes: 3 * GB,
    risk: 'low',
    recommended_action: 'delete',
  },
  {
    node_id: 31,
    path: 'C:\\Temp\\crash.dmp',
    name: 'crash.dmp',
    category: 'log',
    reason_key: 'cleanup.reason.log',
    reclaimable_bytes: 2 * GB,
    risk: 'medium',
    recommended_action: 'review',
  },
  {
    node_id: 47,
    path: 'C:\\Windows\\WinSxS\\Backup',
    name: 'Backup',
    category: 'backup',
    reason_key: 'cleanup.reason.backup',
    reclaimable_bytes: 25 * GB,
    risk: 'high',
    recommended_action: 'review',
  },
];

beforeEach(() => {
  useSnapshotStore.getState().setSnapshot(2, volume);
  useCleanupStore.getState().reset();
});

afterEach(() => {
  useSnapshotStore.getState().reset();
  useCleanupStore.getState().reset();
  vi.restoreAllMocks();
});

describe('cleanup recommendations', () => {
  it('labels every risk with a word', async () => {
    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'cleanup_candidates' ? Promise.resolve(candidates) : null,
    );

    renderWithI18n(<Cleanup />);

    const low = await screen.findByRole('row', { name: /Freespace-cache/ });
    const medium = screen.getByRole('row', { name: /crash\.dmp/ });
    const high = screen.getByRole('row', { name: /WinSxS\\Backup/ });

    expect(within(low).getByText('низкий')).toBeInTheDocument();
    expect(within(medium).getByText('средний')).toBeInTheDocument();
    expect(within(high).getByText('высокий')).toBeInTheDocument();

    // The word carries the status; colour only reinforces it (contract §5.2).
    expect(within(low).getByText('низкий').className).toContain('riskLow');
    expect(within(medium).getByText('средний').className).toContain('riskMedium');
    expect(within(high).getByText('высокий').className).toContain('riskHigh');
  });

  it('filters by reason with AND and leaves selection manual', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const captured: { filter: FilterSet | null } = { filter: null };
    interceptInvoke(transport, (command, args) => {
      if (command === 'cleanup_candidates') {
        const filter = (args?.filter ?? null) as FilterSet | null;
        captured.filter = filter;
        const reasons = filter?.categories ?? [];
        return Promise.resolve(
          candidates.filter((candidate) =>
            reasons.every((reason) => reason === candidate.category),
          ),
        );
      }
      return null;
    });

    renderWithI18n(<Cleanup />);

    expect(await screen.findByText('C:\\Temp\\crash.dmp')).toBeInTheDocument();
    expect(statValue(t('cleanup.panels.candidates'))).toBe('3');
    expect(statValue(t('cleanup.panels.potential'))).toBe(formatBytes(30 * GB));
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('0');

    // The bulk-selection panel is gone: reasons are filters, not actions.
    expect(screen.queryByRole('button', { name: 'Выбрать все' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Снять выбор' })).not.toBeInTheDocument();

    const reasonChip = (name: string) =>
      screen
        .getAllByRole('button', { name })
        .find((button) => button.hasAttribute('aria-pressed'))!;

    await user.click(reasonChip('Кеш'));
    await waitFor(() => {
      expect(screen.queryByText('C:\\Temp\\crash.dmp')).not.toBeInTheDocument();
    });
    expect(captured.filter?.categories).toContain('cache');
    expect(screen.getByText('C:\\Temp\\Freespace-cache')).toBeInTheDocument();
    expect(statValue(t('cleanup.panels.candidates'))).toBe('1');
    // Filtering never selects a candidate for deletion.
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('0');
    expect(
      within(screen.getByRole('row', { name: /Freespace-cache/ })).getByRole('checkbox'),
    ).not.toBeChecked();

    // A second reason is an additional condition, not an alternative: no
    // candidate belongs to two reasons at once.
    await user.click(reasonChip('Журнал'));
    await waitFor(() => {
      expect(statValue(t('cleanup.panels.candidates'))).toBe('0');
    });
    expect(captured.filter?.categories).toEqual(expect.arrayContaining(['cache', 'log']));
    expect(screen.getByText(t('cleanup.none'))).toBeInTheDocument();

    await user.click(reasonChip('Журнал'));
    await waitFor(() => {
      expect(statValue(t('cleanup.panels.candidates'))).toBe('1');
    });

    // Only the row checkbox changes the selection.
    await user.click(
      within(screen.getByRole('row', { name: /Freespace-cache/ })).getByRole('checkbox'),
    );
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('1');
    expect(statValue(t('cleanup.panels.selected_total'))).toBe(formatBytes(3 * GB));

    await user.click(screen.getByRole('button', { name: 'Удалить выбранные (1)' }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText(/Объектов: 1/)).toBeInTheDocument();
  });

  it('selects and clears every filtered row from the header checkbox', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'cleanup_candidates' ? Promise.resolve(candidates) : null,
    );
    renderWithI18n(<Cleanup />);

    const grid = await screen.findByRole('grid', { name: 'Рекомендации' });
    const header = within(grid).getByRole('checkbox', {
      name: t('cleanup.select_all_filtered'),
    });
    expect(header).not.toBeChecked();

    await user.click(header);
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('3');
    for (const row of within(grid).getAllByRole('row').slice(1)) {
      const box = within(row).queryByRole('checkbox');
      if (box !== null) expect(box).toBeChecked();
    }

    // Clearing one row by hand turns the header into a partial selection.
    const firstRow = within(grid).getByRole('row', { name: /Freespace-cache/ });
    await user.click(within(firstRow).getByRole('checkbox'));
    expect(header).not.toBeChecked();
    expect((header as HTMLInputElement).indeterminate).toBe(true);

    await user.click(header);
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('3');
    await user.click(header);
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('0');
  });

  it('keeps the error state recoverable', async () => {
    const failure: UiError = {
      code: 'E_IO',
      category: 'io',
      message_key: 'error.io',
      details: 'cleanup failed',
      retryable: true,
      task_id: null,
      journal_entry_id: null,
    };
    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'cleanup_candidates'
        ? Promise.reject(Object.assign(new Error(failure.details ?? ''), failure))
        : null,
    );

    renderWithI18n(<Cleanup />);

    expect(await screen.findByRole('alert')).toHaveTextContent('cleanup failed');
    expect(screen.getByRole('button', { name: 'Повторить' })).toBeInTheDocument();
  });

  it('uses the shared virtualized grid instead of a plain table', async () => {
    const many: CleanupCandidateView[] = Array.from({ length: 80 }, (_, index) => ({
      ...candidates[0]!,
      node_id: 100 + index,
      path: `C:\\Temp\\cache-${index}.tmp`,
      name: `cache-${index}.tmp`,
    }));
    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'cleanup_candidates' ? Promise.resolve(many) : null,
    );

    renderWithI18n(<Cleanup />);

    const grid = await screen.findByRole('grid', { name: 'Рекомендации' });
    expect(grid).toHaveAttribute('aria-rowcount', '80');
    // Only the visible window (plus overscan) is rendered, not all 80 rows.
    expect(within(grid).getAllByRole('row').length).toBeLessThan(80);
  });

  it('remembers recommendations and selection when the screen is left and reopened', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    let calls = 0;
    interceptInvoke(transport, (command) => {
      if (command === 'cleanup_candidates') {
        calls += 1;
        return Promise.resolve(candidates);
      }
      return null;
    });

    const first = renderWithI18n(<Cleanup />);
    expect(await screen.findByText('C:\\Temp\\crash.dmp')).toBeInTheDocument();
    expect(calls).toBe(1);

    await user.click(
      within(screen.getByRole('row', { name: /Freespace-cache/ })).getByRole('checkbox'),
    );
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('1');

    first.unmount();
    renderWithI18n(<Cleanup />);

    expect(await screen.findByText('C:\\Temp\\crash.dmp')).toBeInTheDocument();
    expect(calls).toBe(1);
    expect(statValue(t('cleanup.panels.selected_count'))).toBe('1');
  });

  it('reloads recommendations when the generation changes', async () => {
    const transport = sharedTransport();
    let calls = 0;
    interceptInvoke(transport, (command) => {
      if (command === 'cleanup_candidates') {
        calls += 1;
        return Promise.resolve(candidates);
      }
      return null;
    });

    const first = renderWithI18n(<Cleanup />);
    expect(await screen.findByText('C:\\Temp\\crash.dmp')).toBeInTheDocument();
    expect(calls).toBe(1);

    first.unmount();
    useSnapshotStore.getState().setSnapshot(3, volume);
    renderWithI18n(<Cleanup />);

    expect(await screen.findByText('C:\\Temp\\crash.dmp')).toBeInTheDocument();
    expect(calls).toBe(2);
  });
});

describe('snapshot comparison', () => {
  const snapshots: SnapshotSummary[] = [
    {
      snapshot_id: 'snap-a',
      path: 'C:\\FreeSpace\\data\\snapshots\\a.fssnap',
      volume_id: 'volume-1',
      mount_point: 'C:\\',
      created_at: '2026-11-01T10:00:00Z',
      node_count: 100,
      file_size: 12_800,
    },
    {
      snapshot_id: 'snap-b',
      path: 'C:\\FreeSpace\\data\\snapshots\\b.fssnap',
      volume_id: 'volume-1',
      mount_point: 'C:\\',
      created_at: '2026-11-10T10:00:00Z',
      node_count: 120,
      file_size: 15_360,
    },
  ];

  const diff: SnapshotDiffPage = {
    task_id: 'compare-1',
    cursor: null,
    next_cursor: null,
    total_estimate: 4,
    added: 1,
    removed: 1,
    changed: 2,
    total_growth_bytes: GB - 2 * GB + 4 * MB - 8 * MB,
    growth_by_directory: [
      { path: 'C:\\New', delta: GB },
      { path: 'C:\\Temp', delta: -2 * GB },
    ],
    rows: [
      {
        path: 'C:\\New\\video.mp4',
        change: 'added',
        is_directory: false,
        size_delta: GB,
        mtime_delta: 0,
        base_size: 0,
        target_size: GB,
      },
      {
        path: 'C:\\Temp\\crash.dmp',
        change: 'removed',
        is_directory: false,
        size_delta: -2 * GB,
        mtime_delta: 0,
        base_size: 2 * GB,
        target_size: 0,
      },
      {
        path: 'C:\\Users\\report.pdf',
        change: 'changed',
        is_directory: false,
        size_delta: 4 * MB,
        mtime_delta: 60,
        base_size: 4 * MB,
        target_size: 8 * MB,
      },
      {
        path: 'C:\\Users\\photo.jpg',
        change: 'changed',
        is_directory: false,
        size_delta: -8 * MB,
        mtime_delta: 60,
        base_size: 20 * MB,
        target_size: 12 * MB,
      },
    ],
  };

  it('compares two snapshots and totals added, removed, grown and shrunk', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    interceptInvoke(transport, (command) => {
      if (command === 'snapshots_list') return Promise.resolve(snapshots);
      if (command === 'snapshots_compare') {
        return Promise.resolve({ task_id: 'compare-1' });
      }
      if (command === 'snapshots_diff_page') return Promise.resolve(diff);

      return null;
    });

    renderWithI18n(<Snapshots />);

    expect(await screen.findByText(snapshots[0]!.created_at)).toBeInTheDocument();
    expect(statValue(t('cleanup.panels.snapshot_count'))).toBe('2');
    expect(statValue(t('cleanup.panels.snapshot_size'))).toBe(
      formatBytes(snapshots[0]!.file_size + snapshots[1]!.file_size),
    );

    const compareButton = screen.getByRole('button', { name: 'Сравнить' });
    await waitFor(() => {
      expect(compareButton).toBeEnabled();
    });
    await user.click(compareButton);

    const grid = await screen.findByRole('grid', { name: 'Сравнить' });
    expect(statValue(t('snapshots.added'))).toBe('1');
    expect(statValue(t('snapshots.removed'))).toBe('1');
    expect(statValue(t('cleanup.panels.grown'))).toBe('1');
    expect(statValue(t('cleanup.panels.shrunk'))).toBe('1');
    expect(statValue(t('snapshots.total_growth'))).toBe(
      `−${formatBytes(Math.abs(diff.total_growth_bytes))}`,
    );

    expect(within(grid).getByText('C:\\New\\video.mp4')).toBeInTheDocument();
    expect(within(grid).getByText('+1 GiB')).toBeInTheDocument();
    expect(within(grid).getByText('−2 GiB')).toBeInTheDocument();
    expect(within(grid).getByText('+4 MiB')).toBeInTheDocument();
    expect(within(grid).getByText('−8 MiB')).toBeInTheDocument();

    // Growth by directory keeps the same signed convention.
    const growthPanel = screen.getByText('Прирост по каталогам').closest('section');
    expect(growthPanel).not.toBeNull();
    expect(within(growthPanel!).getByText('C:\\New')).toBeInTheDocument();
    expect(within(growthPanel!).getByText('+1 GiB')).toBeInTheDocument();
    expect(within(growthPanel!).getByText('−2 GiB')).toBeInTheDocument();
  });
});
