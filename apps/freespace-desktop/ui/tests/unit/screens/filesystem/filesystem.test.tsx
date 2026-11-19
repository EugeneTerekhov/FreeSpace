import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type {
  JournalEntryView,
  JournalFilter,
  JournalPage,
  NodeRow,
  TreePage,
  UiError,
  VolumeSummary,
} from '../../../../src/api/dto';
import type { CommandName } from '../../../../src/api/transport';
import { FileOps } from '../../../../src/screens/FileOps';
import { Journal } from '../../../../src/screens/Journal';
import { LongPaths } from '../../../../src/screens/LongPaths';
import { Permissions } from '../../../../src/screens/Permissions';
import { Rename } from '../../../../src/screens/Rename';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { renderWithI18n } from '../../../helpers/render';
import { sharedTransport } from '../../../helpers/mockTransport';

const volume: VolumeSummary = {
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

const LONG_ROW: NodeRow = {
  id: 9101,
  parent_id: null,
  name: 'long-path-01-audit-report.pdf',
  path: `C:\\Users\\Public\\Documents\\FreeSpace\\Archive\\2026\\${'deeply-nested-'.repeat(6)}long-path-01-audit-report.pdf`,
  kind: 'file',
  logical_size: 48 * 1024 * 1024,
  on_disk_size: 48 * 1024 * 1024,
  physical_size: 48 * 1024 * 1024,
  ads_size: 0,
  child_count: 0,
  file_count: 1,
  modified_at: Date.UTC(2025, 11, 1),
  accessed_at: Date.UTC(2025, 11, 2),
  attributes: {
    read_only: false,
    hidden: false,
    system: false,
    compressed: false,
    sparse: false,
    reparse: false,
  },
  accessible: true,
  link_target: null,
};

const IO_ERROR: UiError = {
  code: 'E_IO',
  category: 'io',
  message_key: 'error.io',
  details: 'C:\\locked',
  retryable: true,
  task_id: null,
  journal_entry_id: null,
};

const PREVIEW_EXPIRED: UiError = {
  code: 'E_PREVIEW_EXPIRED',
  category: 'validation',
  message_key: 'error.preview_expired',
  details: 'token',
  retryable: true,
  task_id: null,
  journal_entry_id: null,
};

type Transport = ReturnType<typeof sharedTransport>;

interface RecordedCall {
  command: CommandName;
  args: Record<string, unknown> | undefined;
}

/** Records every command a screen sends, without changing the response. */
function recordInvoke(transport: Transport): RecordedCall[] {
  const calls: RecordedCall[] = [];
  const original = transport.invoke.bind(transport);
  vi.spyOn(transport, 'invoke').mockImplementation(
    <T,>(command: CommandName, args?: Record<string, unknown>): Promise<T> => {
      calls.push({ command, args });

      return original(command, args);
    },
  );

  return calls;
}

beforeEach(() => {
  useSnapshotStore.getState().setSnapshot(2, volume);
});

afterEach(() => {
  useSnapshotStore.getState().reset();
  vi.restoreAllMocks();
});

describe('LongPaths screen', () => {
  it('selects a row with the keyboard, validates the path and opens the delete preview', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const page: TreePage = {
      generation: 2,
      parent_id: null,
      cursor: null,
      next_cursor: null,
      rows: [LONG_ROW],
      total_estimate: 1,
      metric: 'logical',
      sort: { key: 'size', descending: true },
      stale_cursor: false,
    };
    transport.setHandler('long_paths_page', () => page);
    renderWithI18n(<LongPaths />);

    const cell = await screen.findByTitle(/long-path-01-/);
    const path = cell.getAttribute('title') ?? '';
    const row = screen.getByRole('row', { name: /long-path-01-/ });
    row.focus();
    expect(row).toHaveFocus();
    await user.keyboard(' ');

    expect(screen.getByLabelText('Объект')).toHaveValue(path);
    await user.click(screen.getByRole('button', { name: 'Проверить' }));
    expect(await screen.findByText(/Готово/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Удалить' }));
    const dialog = await screen.findByRole('dialog');
    const confirm = within(dialog).getByRole('button', { name: 'Удалить' });
    expect(confirm).toBeDisabled();
    await user.click(
      within(dialog).getByRole('checkbox', {
        name: 'Я просмотрел объекты и подтверждаю операцию',
      }),
    );
    expect(confirm).toBeEnabled();
    await user.click(confirm);

    expect(await screen.findByText('Ход операции')).toBeInTheDocument();
    expect(screen.getByText('завершено')).toBeInTheDocument();
  });

  it('renders the empty result inside the table', async () => {
    const transport = sharedTransport();
    const emptyPage: TreePage = {
      generation: 2,
      parent_id: null,
      cursor: null,
      next_cursor: null,
      rows: [],
      total_estimate: 0,
      metric: 'logical',
      sort: { key: 'size', descending: true },
      stale_cursor: false,
    };
    transport.setHandler('long_paths_page', () => emptyPage);
    renderWithI18n(<LongPaths />);

    expect(await screen.findByText('По фильтру ничего не найдено.')).toBeInTheDocument();
  });

  it('surfaces a backend error', async () => {
    const transport = sharedTransport();
    transport.failCommand('long_paths_page', IO_ERROR);
    renderWithI18n(<LongPaths />);

    expect(await screen.findByText('Ошибка файловой системы.')).toBeInTheDocument();
  });

  it('shows the verbatim path and previews an operation before writing', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const verbatimRow: NodeRow = {
      ...LONG_ROW,
      id: 9201,
      name: 'verbatim-check.txt',
      path: `C:\\Users\\Public\\Documents\\FreeSpace\\Archive\\2026\\${'very-long-folder-name\\'.repeat(16)}verbatim-check.txt`,
    };
    const page: TreePage = {
      generation: 2,
      parent_id: null,
      cursor: null,
      next_cursor: null,
      rows: [verbatimRow],
      total_estimate: 1,
      metric: 'logical',
      sort: { key: 'size', descending: true },
      stale_cursor: false,
    };
    transport.setHandler('long_paths_page', () => page);
    const calls = recordInvoke(transport);
    renderWithI18n(<LongPaths />);

    const row = await screen.findByRole('row', { name: /verbatim-check/ });
    row.focus();
    await user.keyboard(' ');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));

    // The validated path keeps the verbatim prefix the operations layer uses.
    expect(await screen.findByText(/^\\\\\?\\/)).toBeInTheDocument();
    expect(screen.getByText(/Готово/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Удалить' }));
    const dialog = await screen.findByRole('dialog');
    expect(calls.some((call) => call.command === 'operation_preview')).toBe(true);
    expect(calls.some((call) => call.command === 'operation_confirm')).toBe(false);

    await user.click(
      within(dialog).getByRole('checkbox', {
        name: 'Я просмотрел объекты и подтверждаю операцию',
      }),
    );
    await user.click(within(dialog).getByRole('button', { name: 'Удалить' }));

    expect(await screen.findByText('Ход операции')).toBeInTheDocument();
    expect(calls.some((call) => call.command === 'operation_confirm')).toBe(true);
  });
});

describe('Rename screen', () => {
  it('previews old and new names and applies exactly once', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Rename />);

    await user.type(
      screen.getByLabelText('Объекты (по одному в строке)'),
      'C:\\Users\\Downloads\\archive.zip\nC:\\Users\\Downloads\\backup-1.bin',
    );
    await user.type(screen.getByLabelText('Шаблон'), 'archive|backup');
    await user.type(screen.getByLabelText('Замена'), 'media');
    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));

    expect(await screen.findByText('C:\\Users\\Downloads\\media.zip')).toBeInTheDocument();
    expect(screen.getByText('C:\\Users\\Downloads\\media-1.bin')).toBeInTheDocument();
    expect(screen.getByText(/Планируется переименований: 2, конфликтов: 0/)).toBeInTheDocument();

    // The plan shows the old and the new name of every object side by side.
    expect(screen.getByText('archive.zip')).toBeInTheDocument();
    expect(screen.getByText('media.zip')).toBeInTheDocument();
    expect(screen.getByText('backup-1.bin')).toBeInTheDocument();
    expect(screen.getByText('media-1.bin')).toBeInTheDocument();

    const apply = screen.getByRole('button', { name: 'Переименовать' });
    expect(apply).toBeDisabled();
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я просмотрел предпросмотр и подтверждаю переименование',
      }),
    );
    expect(apply).toBeEnabled();
    await user.click(apply);

    expect(await screen.findByText('Переименовано объектов: 2')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Переименовать' })).not.toBeInTheDocument();
  });

  it('blocks apply while conflicts remain', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Rename />);

    await user.type(
      screen.getByLabelText('Объекты (по одному в строке)'),
      'C:\\a.txt\nC:\\b.txt',
    );
    await user.type(screen.getByLabelText('Шаблон'), '.*');
    await user.type(screen.getByLabelText('Замена'), 'same');
    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));

    expect(await screen.findByText(/Планируется переименований: 0, конфликтов: 2/)).toBeInTheDocument();
    const table = screen.getByRole('table', { name: 'Предпросмотр' });
    const conflictRows = within(table)
      .getAllByRole('row')
      .filter((row) => row.getAttribute('aria-invalid') === 'true');
    expect(conflictRows).toHaveLength(2);
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я просмотрел предпросмотр и подтверждаю переименование',
      }),
    );
    expect(screen.getByRole('button', { name: 'Переименовать' })).toBeDisabled();
    expect(screen.getAllByText('Проверьте введённые значения.').length).toBeGreaterThan(0);
  });

  it('recovers from an expired preview token', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    renderWithI18n(<Rename />);

    await user.type(
      screen.getByLabelText('Объекты (по одному в строке)'),
      'C:\\a.txt',
    );
    await user.type(screen.getByLabelText('Шаблон'), 'a');
    await user.type(screen.getByLabelText('Замена'), 'b');
    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));
    await screen.findByText(/Планируется переименований: 1/);

    transport.failCommand('rename_apply', PREVIEW_EXPIRED);
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я просмотрел предпросмотр и подтверждаю переименование',
      }),
    );
    await user.click(screen.getByRole('button', { name: 'Переименовать' }));

    expect(
      await screen.findByText('Предпросмотр истёк; повторите подтверждение.'),
    ).toBeInTheDocument();
    await user.click(screen.getAllByRole('button', { name: 'Предпросмотр' })[0]!);

    expect(await screen.findByText(/Планируется переименований: 1/)).toBeInTheDocument();
    expect(
      screen.queryByText('Предпросмотр истёк; повторите подтверждение.'),
    ).not.toBeInTheDocument();
  });
});

describe('Permissions screen', () => {
  it('shows the ACL, previews an attribute change and applies it after confirmation', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Permissions />);

    expect(screen.getByText('Ничего не выбрано')).toBeInTheDocument();
    await user.type(screen.getByLabelText('Объект'), 'C:\\Users\\Documents');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));

    expect(await screen.findByText('DOMAIN\\alex')).toBeInTheDocument();
    expect(
      screen.getByText('S-1-5-21-1004336348-1177238915-682003330-1001'),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('switch', { name: 'Скрытый' }));
    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));
    expect(await screen.findByText(/Снять.*Установить/)).toBeInTheDocument();

    const save = screen.getByRole('button', { name: 'Сохранить' });
    expect(save).toBeDisabled();
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я понимаю, что изменения затрагивают доступ и могут требовать прав администратора',
      }),
    );
    expect(save).toBeEnabled();
    await user.click(save);

    expect(await screen.findByText('Обновлено: 1, ошибок: 0')).toBeInTheDocument();
  });

  it('explains elevation when the protected path cannot be changed', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Permissions />);

    await user.type(screen.getByLabelText('Объект'), 'C:\\Windows\\System32');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));
    expect(
      await screen.findByText('Для этой операции нужны права администратора.'),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('switch', { name: 'Скрытый' }));
    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));
    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я понимаю, что изменения затрагивают доступ и могут требовать прав администратора',
      }),
    );
    await user.click(screen.getByRole('button', { name: 'Сохранить' }));

    expect(await screen.findByText(/Нужны права администратора/)).toBeInTheDocument();
  });

  it('reads the ACL without writing and applies only after the preview is confirmed', async () => {
    const user = userEvent.setup();
    const calls = recordInvoke(sharedTransport());
    renderWithI18n(<Permissions />);

    await user.type(screen.getByLabelText('Объект'), 'C:\\Users\\Documents');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));
    await screen.findByText('DOMAIN\\alex');

    // Reading the security descriptor is the only command so far.
    expect([...new Set(calls.map((call) => call.command))]).toEqual(['permissions_get']);

    await user.click(screen.getByRole('switch', { name: 'Скрытый' }));
    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));
    await screen.findByText(/Снять.*Установить/);

    expect(calls.some((call) => call.command === 'permissions_preview')).toBe(true);
    expect(calls.some((call) => call.command === 'permissions_apply')).toBe(false);

    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я понимаю, что изменения затрагивают доступ и могут требовать прав администратора',
      }),
    );
    await user.click(screen.getByRole('button', { name: 'Сохранить' }));

    expect(await screen.findByText('Обновлено: 1, ошибок: 0')).toBeInTheDocument();
    expect(calls.some((call) => call.command === 'permissions_apply')).toBe(true);
  });
});

describe('File operations screen', () => {
  it('keeps rename and permissions as two stateful tabs', async () => {
    const user = userEvent.setup();
    renderWithI18n(<FileOps />);

    expect(screen.getAllByRole('tab')).toHaveLength(2);

    await user.type(
      screen.getByLabelText('Объекты (по одному в строке)'),
      'C:\\a.txt',
    );
    await user.click(screen.getByRole('tab', { name: 'Права доступа' }));
    expect(screen.getByLabelText('Объект')).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: 'Переименование' }));

    // Each tab owns its state; switching back keeps the rename draft.
    expect(screen.getByLabelText('Объекты (по одному в строке)')).toHaveValue('C:\\a.txt');
    expect(screen.getByRole('tab', { name: 'Переименование' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
  });
});

describe('Journal screen', () => {
  const entries: JournalEntryView[] = [
    {
      id: '1',
      operation_id: 'op-1',
      occurred_at: Date.UTC(2026, 0, 1, 10, 0),
      user: 'mock-user',
      kind: 'delete',
      targets_count: 12,
      total_size: 2048,
      result: 'success',
      reversible: true,
      quarantine_ids: ['quarantine-1'],
      error: null,
    },
    {
      id: '2',
      operation_id: 'op-2',
      occurred_at: Date.UTC(2026, 0, 2, 10, 0),
      user: 'auditor',
      kind: 'rename',
      targets_count: 3,
      total_size: 1024,
      result: 'failed',
      reversible: false,
      quarantine_ids: [],
      error: null,
    },
  ];

  it('passes the JournalFilter to the backend, resets the cursor and can restore', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const calls: JournalFilter[] = [];
    const cursors: (string | null)[] = [];
    let restoreEntry: string | null = null;

    transport.setHandler('journal_page', (args) => {
      const filter = args.filter as JournalFilter;
      calls.push(filter);
      const cursor = typeof args.cursor === 'string' ? args.cursor : null;
      cursors.push(cursor);
      const filtered = entries.filter((entry) => {
        if (filter.kinds.length > 0 && !filter.kinds.includes(entry.kind)) return false;
        if (filter.results.length > 0 && !filter.results.includes(entry.result)) return false;
        if (
          filter.text !== '' &&
          !entry.user.toLowerCase().includes(filter.text.toLowerCase()) &&
          !entry.operation_id.toLowerCase().includes(filter.text.toLowerCase())
        ) {
          return false;
        }

        return true;
      });
      const page: JournalPage = {
        cursor,
        next_cursor: cursor === null && filtered.length > 1 ? '1' : null,
        rows: cursor === null ? filtered.slice(0, 1) : filtered.slice(1),
        total_estimate: filtered.length,
      };

      return page;
    });
    transport.setHandler('journal_restore', (args) => {
      restoreEntry = String(args.entry_id);
      return { operation_id: 'restore-1' };
    });

    renderWithI18n(<Journal />);
    expect(await screen.findByText('mock-user')).toBeInTheDocument();
    expect(calls[0]).toEqual({ text: '', kinds: [], results: [] });

    await user.selectOptions(screen.getByRole('combobox', { name: 'Операция' }), 'delete');
    await waitFor(() => {
      expect(calls.some((filter) => filter.kinds.includes('delete'))).toBe(true);
    });
    expect(calls.at(-1)?.kinds).toEqual(['delete']);
    expect(calls.at(-1)?.text).toBe('');
    expect(await screen.findByText('mock-user')).toBeInTheDocument();
    expect(screen.queryByText('auditor')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Сбросить' }));
    await waitFor(() => {
      expect(calls.at(-1)).toEqual({ text: '', kinds: [], results: [] });
    });

    const loadMore = await screen.findByRole('button', { name: 'Загрузить ещё' });
    await user.click(loadMore);
    await waitFor(() => {
      expect(cursors.includes('1')).toBe(true);
    });
    await user.selectOptions(screen.getByRole('combobox', { name: 'Операция' }), 'rename');
    await waitFor(() => {
      expect(calls.at(-1)?.kinds).toEqual(['rename']);
    });
    expect(cursors.at(-1)).toBeNull();

    await user.click(screen.getByRole('button', { name: 'Сбросить' }));
    await waitFor(() => {
      expect(calls.at(-1)?.kinds).toEqual([]);
    });
    expect(await screen.findByText('mock-user')).toBeInTheDocument();
    await user.click(screen.getByText('mock-user'));
    await user.click(screen.getByRole('button', { name: 'Восстановить' }));
    const confirmRestore = await screen.findByRole('dialog');
    await user.click(within(confirmRestore).getByRole('button', { name: 'Восстановить' }));
    expect(await screen.findByText('Восстановлено: 1, ошибок: 0')).toBeInTheDocument();
    expect(restoreEntry).toBe('1');
  });

  it('filters by text on the backend after the debounce', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const calls: JournalFilter[] = [];
    transport.setHandler('journal_page', (args) => {
      const filter = args.filter as JournalFilter;
      calls.push(filter);
      const rows = entries.filter((entry) =>
        filter.text === '' || entry.user.toLowerCase().includes(filter.text.toLowerCase()),
      );

      return {
        cursor: null,
        next_cursor: null,
        rows,
        total_estimate: rows.length,
      } satisfies JournalPage;
    });

    renderWithI18n(<Journal />);
    expect(await screen.findByText('mock-user')).toBeInTheDocument();
    await user.type(screen.getByRole('searchbox', { name: 'Поиск по журналу' }), 'auditor');

    await waitFor(() => {
      expect(calls.at(-1)?.text).toBe('auditor');
    });
    expect(await screen.findByText('auditor')).toBeInTheDocument();
    expect(screen.queryByText('mock-user')).not.toBeInTheDocument();
  });

  it('opens a details card on hover and writes the error text', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const failed: JournalEntryView = {
      id: '2',
      operation_id: 'op-2',
      occurred_at: Date.UTC(2026, 0, 2, 10, 0),
      user: 'auditor',
      kind: 'rename',
      targets_count: 3,
      total_size: 1024,
      result: 'failed',
      reversible: false,
      quarantine_ids: [],
      error: IO_ERROR,
    };
    transport.setHandler('journal_page', () => ({
      cursor: null,
      next_cursor: null,
      rows: [failed],
      total_estimate: 1,
    }));

    renderWithI18n(<Journal />);
    const row = await screen.findByRole('row', { name: /Переименование/ });
    await user.hover(row);

    const card = await screen.findByRole('tooltip', { name: 'Детали записи' });
    expect(within(card).getByText('op-2')).toBeInTheDocument();
    expect(within(card).getByText('Ошибка файловой системы.')).toBeInTheDocument();
    expect(within(card).getByText('C:\\locked')).toBeInTheDocument();
    expect(within(card).getByText('E_IO')).toBeInTheDocument();
  });

  it('shows a loading placeholder while the first page arrives', async () => {
    const transport = sharedTransport();
    transport.setHandler('journal_page', () => new Promise(() => undefined));
    renderWithI18n(<Journal />);

    expect(await screen.findByLabelText('Загрузка…')).toBeInTheDocument();
  });
});
