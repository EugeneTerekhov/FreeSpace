import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  JournalEntryView,
  JournalFilter,
  MonitorEvent,
  OperationPreview,
  OperationProgress,
} from '../../../src/api/dto';
import type { CommandName } from '../../../src/api/transport';
import { OperationPreviewDialog } from '../../../src/components/OperationPreviewDialog';
import { OperationProgressView } from '../../../src/components/OperationProgress';
import { t } from '../../../src/i18n';
import { formatBytes } from '../../../src/i18n/format';
import { Journal } from '../../../src/screens/Journal';
import { Monitor } from '../../../src/screens/Monitor';
import { Shred } from '../../../src/screens/Shred';
import { needsElevation, elevationError } from '../../../src/operations/elevation';
import { sharedTransport } from '../../helpers/mockTransport';
import { renderWithI18n } from '../../helpers/render';

const DAY = 24 * 60 * 60 * 1000;

function preview(overrides: Partial<OperationPreview> = {}): OperationPreview {
  return {
    preview_id: 'preview-1',
    kind: 'delete',
    targets: [
      {
        path: 'C:\\Temp\\victim.txt',
        kind: 'file',
        size: 12,
        accessible: true,
        is_system: false,
      },
    ],
    total_size: 12,
    delete_mode: 'recycle_bin',
    reversible: true,
    requires_elevation: false,
    warnings: [],
    expires_at: Date.now() + 600_000,
    confirmation_token: 'token-1',
    ...overrides,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('operation preview', () => {
  it('requires an acknowledgement and an extra confirmation for permanent deletion', async () => {
    const user = userEvent.setup();
    const confirm = vi.fn();
    renderWithI18n(
      <OperationPreviewDialog
        open
        preview={preview()}
        error={null}
        onCancel={vi.fn()}
        onConfirm={confirm}
      />,
    );

    const apply = screen.getByRole('button', { name: 'Удалить' });
    expect(apply).toBeDisabled();

    await user.click(screen.getByRole('radio', { name: /Безвозвратное удаление/ }));
    await user.click(screen.getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' }));
    expect(apply).toBeDisabled();

    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я понимаю, что путь защищён, и подтверждаю действие с повышенными правами',
      }),
    );
    expect(apply).toBeEnabled();
    await user.click(apply);
    expect(confirm).toHaveBeenCalledWith('permanent', true);
  });

  it('shows targets and warnings from the preview', () => {
    renderWithI18n(
      <OperationPreviewDialog
        open
        preview={preview({
          requires_elevation: true,
          warnings: ['confirm.warning.protected'],
        })}
        error={null}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );

    expect(screen.getByText('C:\\Temp\\victim.txt')).toBeInTheDocument();
    expect(screen.getByText(/защищённые системные пути/)).toBeInTheDocument();
  });

  it('reports elevation requirements from the preview', () => {
    expect(needsElevation(preview())).toBe(false);
    expect(elevationError(preview())).toBeNull();
    const elevated = preview({ requires_elevation: true });
    expect(needsElevation(elevated)).toBe(true);
    expect(elevationError(elevated)?.code).toBe('E_ELEVATION_REQUIRED');
  });
});

describe('operation progress', () => {
  it('offers cancel while running and undo when available', async () => {
    const user = userEvent.setup();
    const cancel = vi.fn();
    const undo = vi.fn();
    const running: OperationProgress = {
      operation_id: 'operation-1',
      state: 'running',
      processed: 1,
      total: 3,
      current_target: 'C:\\Temp\\victim.txt',
      bytes_per_second: null,
      errors: [],
      undo_available: false,
      journal_entry_id: null,
    };
    const { rerender } = renderWithI18n(
      <OperationProgressView progress={running} onCancel={cancel} onUndo={undo} />,
    );
    await user.click(screen.getByRole('button', { name: 'Отменить операцию' }));
    expect(cancel).toHaveBeenCalled();

    rerender(
      <OperationProgressView
        progress={{ ...running, state: 'completed', undo_available: true }}
        onCancel={cancel}
        onUndo={undo}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Отменить изменения' }));
    expect(undo).toHaveBeenCalled();
  });
});

function journalEntry(overrides: Partial<JournalEntryView> = {}): JournalEntryView {
  return {
    id: '1',
    operation_id: 'operation-1',
    occurred_at: Date.now(),
    user: 'mock-user',
    kind: 'delete',
    targets_count: 2,
    total_size: 2048,
    result: 'success',
    reversible: true,
    quarantine_ids: [],
    error: null,
    ...overrides,
  };
}

/** Serves the given journal rows and records every command the screen sends. */
function serveJournal(rows: JournalEntryView[]) {
  const transport = sharedTransport();
  const original = transport.invoke.bind(transport);

  return vi.spyOn(transport, 'invoke').mockImplementation(((
    command: CommandName,
    args?: Record<string, unknown>,
  ) => {
    if (command === 'journal_page') {
      return Promise.resolve({
        cursor: null,
        next_cursor: null,
        rows,
        total_estimate: rows.length,
      });
    }
    if (command === 'journal_restore' || command === 'journal_undo') {
      return Promise.resolve({ operation_id: 'operation-restored' });
    }

    return original(command, args);
  }));
}

function lastJournalFilter(invoke: ReturnType<typeof serveJournal>): JournalFilter {
  const calls = invoke.mock.calls.filter(([command]) => command === 'journal_page');
  const args = calls.at(-1)?.[1] as { filter?: JournalFilter } | undefined;

  return args?.filter ?? { text: '', kinds: [], results: [] };
}

describe('journal filters', () => {
  it('filters by kind, result, period and text and records every facet', async () => {
    const user = userEvent.setup();
    const old = journalEntry({
      id: 'old',
      operation_id: 'operation-old',
      occurred_at: Date.now() - 40 * DAY,
      user: 'old-user',
    });
    const shred = journalEntry({
      id: 'shred',
      operation_id: 'operation-shred',
      kind: 'shred',
      result: 'failed',
      reversible: false,
      user: 'shred-user',
    });
    const invoke = serveJournal([old, shred, journalEntry()]);

    renderWithI18n(<Journal />);
    expect(await screen.findByText('old-user')).toBeInTheDocument();
    const grid = screen.getByRole('grid', { name: t('nav.journal') });
    expect(within(grid).getAllByText(t('journal.kind.delete'))).toHaveLength(2);

    await user.selectOptions(
      screen.getByLabelText(t('journal.kind_label')),
      'shred',
    );
    await waitFor(() => {
      expect(lastJournalFilter(invoke).kinds).toEqual(['shred']);
    });
    expect(screen.queryByText('old-user')).not.toBeInTheDocument();
    expect(screen.getByText('shred-user')).toBeInTheDocument();

    await user.selectOptions(
      screen.getByLabelText(t('journal.result_label')),
      'failed',
    );
    await waitFor(() => {
      expect(lastJournalFilter(invoke).results).toEqual(['failed']);
    });
    expect(screen.getByText('shred-user')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: t('filter.reset') }));
    await waitFor(() => {
      expect(lastJournalFilter(invoke)).toEqual({ text: '', kinds: [], results: [] });
    });
    expect(await screen.findByText('old-user')).toBeInTheDocument();

    await user.selectOptions(
      screen.getByLabelText(t('operations.journal.range')),
      'day',
    );
    await waitFor(() => {
      expect(screen.queryByText('old-user')).not.toBeInTheDocument();
    });
    expect(screen.getByText('mock-user')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: t('filter.reset') }));
    const search = screen.getByRole('searchbox', { name: t('journal.search') });
    await user.type(search, 'old-user');
    await waitFor(
      () => {
        expect(lastJournalFilter(invoke).text).toBe('old-user');
      },
      { timeout: 2000 },
    );
  });
});

describe('journal undo and restore', () => {
  it('routes restore and undo through the confirmation dialog', async () => {
    const user = userEvent.setup();
    const restoreEntry = journalEntry({
      id: 'restore',
      operation_id: 'operation-restore',
      kind: 'move',
      quarantine_ids: ['quarantine-1'],
      user: 'restore-user',
    });
    const undoEntry = journalEntry({
      id: 'undo',
      operation_id: 'operation-undo',
      kind: 'rename',
      user: 'undo-user',
    });
    const invoke = serveJournal([restoreEntry, undoEntry]);

    renderWithI18n(<Journal />);
    expect(await screen.findByText('restore-user')).toBeInTheDocument();

    await user.click(screen.getByText('restore-user'));
    await user.click(screen.getByRole('button', { name: t('journal.restore') }));

    let dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: t('journal.restore') }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('journal_restore', { entry_id: 'restore' });
    });
    expect(
      await screen.findByText(t('journal.restored', { restored: 1, failed: 0 })),
    ).toBeInTheDocument();

    await user.click(screen.getByText('undo-user'));
    await user.click(screen.getByRole('button', { name: t('journal.undo') }));

    dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: t('journal.undo') }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('journal_undo', { entry_id: 'undo' });
    });
    expect(await screen.findByText(t('operations.journal.undone'))).toBeInTheDocument();
  });
});

describe('shredder', () => {
  it('previews first and starts only after the explicit confirmation', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const original = transport.invoke.bind(transport);
    const invoke = vi.spyOn(transport, 'invoke').mockImplementation(((
      command: CommandName,
      args?: Record<string, unknown>,
    ) => original(command, args)));

    renderWithI18n(<Shred />);

    const prepare = screen.getByRole('button', { name: t('details.inspect') });
    expect(prepare).toBeDisabled();
    expect(screen.getByText(t('shred.irreversible'))).toBeInTheDocument();
    expect(screen.getByText(t('shred.ssd_warning'))).toBeInTheDocument();

    await user.type(
      screen.getByLabelText(t('shred.files')),
      'C:\\Temp\\a.bin\nD:\\Temp\\b.bin',
    );
    expect(prepare).toBeEnabled();
    await user.click(prepare);

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText(/Объектов: 2/)).toBeInTheDocument();

    await waitFor(() => {
      expect(
        invoke.mock.calls.some(([command]) => command === 'shred_preview'),
      ).toBe(true);
    });
    const previewArgs = invoke.mock.calls.find(([command]) => command === 'shred_preview')?.[1];
    expect(previewArgs).toMatchObject({
      targets: ['C:\\Temp\\a.bin', 'D:\\Temp\\b.bin'],
      passes: 3,
      pattern: 'zeros',
    });

    const destroy = within(dialog).getByRole('button', { name: t('shred.destroy') });
    expect(destroy).toBeDisabled();
    expect(destroy).toHaveAttribute('data-agent-confirm', 'true');

    // Nothing has started while the acknowledgement is unchecked.
    expect(invoke.mock.calls.some(([command]) => command === 'shred_start')).toBe(false);

    await user.click(
      within(dialog).getByRole('checkbox', { name: t('shred.confirm') }),
    );
    expect(destroy).toBeEnabled();
    await user.click(destroy);

    await waitFor(() => {
      expect(
        invoke.mock.calls.some(([command]) => command === 'shred_start'),
      ).toBe(true);
    });
    const startArgs = invoke.mock.calls.find(([command]) => command === 'shred_start')?.[1];
    expect(startArgs).toBeDefined();
    expect(typeof startArgs?.preview_id).toBe('string');
    expect(typeof startArgs?.confirmation_token).toBe('string');
    expect(startArgs?.extra_confirmation).toBe(true);
  });
});

describe('write monitor', () => {
  it('starts, renders owner, path and size, and stops with a visible state', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const original = transport.invoke.bind(transport);
    const invoke = vi.spyOn(transport, 'invoke').mockImplementation(((
      command: CommandName,
      args?: Record<string, unknown>,
    ) => original(command, args)));

    renderWithI18n(<Monitor />);

    expect(screen.getByText(t('monitor.no_events'))).toBeInTheDocument();
    expect(screen.getAllByText(t('status.ready')).length).toBeGreaterThan(0);

    await user.click(screen.getByRole('button', { name: t('monitor.start') }));

    expect(await screen.findByText('explorer.exe')).toBeInTheDocument();
    expect(
      screen.getByText('C:\\Users\\Public\\Documents\\report.tmp'),
    ).toBeInTheDocument();
    expect(screen.getByText(formatBytes(64 * 1024 * 1024))).toBeInTheDocument();
    expect(screen.getByText(/Источник событий: ETW/)).toBeInTheDocument();
    expect(screen.getAllByText(t('monitor.running')).length).toBeGreaterThan(0);

    const startArgs = invoke.mock.calls.find(([command]) => command === 'monitor_start')?.[1];
    expect(startArgs).toMatchObject({
      threshold: 1024 * 1024 * 1024,
      interval: 2,
      exclusions: [],
    });

    const polite = document.querySelector('[aria-live="polite"]');
    expect(polite).not.toBeNull();
    expect(polite).toHaveTextContent(
      t('operations.monitor.new_events', { count: 3 }),
    );

    const status = screen.getByTestId('monitor-status');
    const taskId = status.getAttribute('data-task-id') ?? '';
    expect(taskId).not.toBe('');

    await user.click(screen.getByRole('button', { name: t('monitor.stop') }));

    expect(await screen.findByText(t('monitor.summary'))).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith('monitor_stop', { task_id: taskId });
    expect(screen.getAllByText(t('status.finished')).length).toBeGreaterThan(0);
  });

  it('appends live tick events to the table', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Monitor />);
    await user.click(screen.getByRole('button', { name: t('monitor.start') }));
    expect(await screen.findByText('explorer.exe')).toBeInTheDocument();

    const taskId =
      screen.getByTestId('monitor-status').getAttribute('data-task-id') ?? '';
    const event: MonitorEvent = {
      sequence: 99,
      timestamp: Date.now(),
      process_id: 99,
      process: 'injected.exe',
      bytes: 1024,
      paths: ['C:\\Temp\\injected.bin'],
      threshold_exceeded: true,
    };

    act(() => {
      sharedTransport().emit(
        'monitor-tick',
        { task_id: taskId, sequence: 99, events: [event] },
        taskId,
      );
    });

    expect(await screen.findByText('injected.exe')).toBeInTheDocument();
    expect(screen.getByText('C:\\Temp\\injected.bin')).toBeInTheDocument();
  });

  it('rejects an out-of-range interval before starting', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Monitor />);

    const interval = screen.getByLabelText((content) =>
      content.startsWith(t('monitor.interval')),
    );
    await user.clear(interval);
    await user.type(interval, '0');
    await user.click(screen.getByRole('button', { name: t('monitor.start') }));

    expect(screen.getByRole('alert')).toHaveTextContent(t('error.validation'));
  });
});
