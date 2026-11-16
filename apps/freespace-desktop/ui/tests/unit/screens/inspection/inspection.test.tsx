import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UiError } from '../../../../src/api/dto';
import type { OwnerSummaryPage } from '../../../../src/api/dto-inspection';
import type { CommandName } from '../../../../src/api/transport';
import { Archives } from '../../../../src/screens/Archives';
import { Export } from '../../../../src/screens/Export';
import { Metadata } from '../../../../src/screens/Metadata';
import { Owners } from '../../../../src/screens/Owners';
import { Unlock } from '../../../../src/screens/Unlock';
import { useShellStore } from '../../../../src/stores/shellStore';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { renderWithI18n } from '../../../helpers/render';
import { sharedTransport } from '../../../helpers/mockTransport';

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

function failure(): Error {
  const error: UiError = {
    code: 'E_IO',
    category: 'io',
    message_key: 'error.io',
    details: 'injected failure',
    retryable: true,
    task_id: null,
    journal_entry_id: null,
  };

  return Object.assign(new Error(error.details ?? 'injected failure'), error);
}

beforeEach(() => {
  useSnapshotStore.getState().setSnapshot(2, volume);
});

afterEach(() => {
  useSnapshotStore.getState().reset();
  useShellStore.getState().setScreen('dashboard');
  vi.restoreAllMocks();
});

describe('metadata screen', () => {
  it('extracts records by path and filters by metadata kind and dimensions', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Metadata />);

    expect(
      screen.getByText('Нажмите «Прочитать метаданные», чтобы извлечь теги из найденных файлов.'),
    ).toBeInTheDocument();

    const path = screen.getByRole('textbox', { name: 'Файл' });
    await user.type(path, 'photo');
    await user.keyboard('{Enter}');

    expect(await screen.findByText('C:\\Users\\Pictures\\photo.jpg')).toBeInTheDocument();
    expect(screen.getByText('ОК')).toBeInTheDocument();
    expect(screen.getByText('6000')).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText('Семейство'), 'audio');
    expect(
      await screen.findByText('Нажмите «Прочитать метаданные», чтобы извлечь теги из найденных файлов.'),
    ).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText('Семейство'), 'all');
    const minWidth = screen.getByLabelText('Минимальная ширина');
    await user.type(minWidth, '7000');
    expect(screen.getByText('Нажмите «Прочитать метаданные», чтобы извлечь теги из найденных файлов.')).toBeInTheDocument();

    await user.clear(minWidth);
    await user.type(minWidth, '5000');
    expect(await screen.findByText('C:\\Users\\Pictures\\photo.jpg')).toBeInTheDocument();
  });

  it('shows the empty result and the validation error', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Metadata />);

    const path = screen.getByRole('textbox', { name: 'Файл' });
    await user.type(path, 'no-such-media');
    await user.click(screen.getByRole('button', { name: 'Прочитать метаданные' }));
    expect(
      await screen.findByText('Нажмите «Прочитать метаданные», чтобы извлечь теги из найденных файлов.'),
    ).toBeInTheDocument();

    await user.clear(path);
    await user.click(screen.getByRole('button', { name: 'Прочитать метаданные' }));
    expect(await screen.findByText('Проверьте введённые значения.')).toBeInTheDocument();
    expect(screen.getByText('Сначала укажите путь.')).toBeInTheDocument();
  });
});

describe('owners screen', () => {
  it('loads owner totals, pages them and drills into a paged file list', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Owners />);

    expect(
      screen.getByText('Нажмите «Построить отчёт», чтобы прочитать владельцев файлов.'),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Построить отчёт по владельцам' }));
    expect(await screen.findByText('CORP\\alice')).toBeInTheDocument();
    expect(screen.getByText('S-1-5-21-1001')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Загрузить ещё' }));
    expect(await screen.findByText('CORP\\dmitri')).toBeInTheDocument();

    const logins = screen.getAllByRole('button', { name: 'Объекты' });
    await user.click(logins[0]!);
    expect(await screen.findByText('C:\\Users\\alice\\Documents\\owned-1.dat')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Загрузить ещё' }));
    expect(await screen.findByText('C:\\Users\\alice\\Documents\\owned-6.dat')).toBeInTheDocument();
  });

  it('shows the loading progress and a transport error', async () => {
    const transport = sharedTransport();
    const realPage = await transport.invoke<OwnerSummaryPage>('owners_page', {
      generation: 2,
      filter: null,
      cursor: null,
      page_size: 5,
    });
    let release!: (page: OwnerSummaryPage) => void;
    const gate = new Promise<OwnerSummaryPage>((resolve) => {
      release = resolve;
    });
    interceptInvoke(transport, (command) =>
      command === 'owners_page' ? gate : null,
    );

    const user = userEvent.setup();
    renderWithI18n(<Owners />);
    await user.click(screen.getByRole('button', { name: 'Построить отчёт по владельцам' }));
    expect(screen.getByRole('progressbar', { name: 'Чтение владельцев…' })).toBeInTheDocument();

    release(realPage);
    expect(await screen.findByText('CORP\\alice')).toBeInTheDocument();
  });

  it('renders the retry action when the owner report fails', async () => {
    const transport = sharedTransport();
    interceptInvoke(transport, (command) =>
      command === 'owners_page' ? Promise.reject(failure()) : null,
    );

    const user = userEvent.setup();
    renderWithI18n(<Owners />);
    await user.click(screen.getByRole('button', { name: 'Построить отчёт по владельцам' }));

    expect(await screen.findByText('Ошибка файловой системы.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Повторить' })).toBeInTheDocument();
  });

  it('opens the owners export dialog from the summary', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Owners />);

    await user.click(screen.getByRole('button', { name: 'Построить отчёт по владельцам' }));
    await screen.findByText('CORP\\alice');
    await user.click(screen.getByRole('button', { name: 'Экспортировать' }));

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByRole('checkbox', { name: 'Владельцы' })).toBeChecked();
    expect(within(dialog).getByRole('checkbox', { name: 'Дерево' })).not.toBeChecked();
    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
  });
});

describe('archives screen', () => {
  it('reads entries, pages them and exports the list', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Archives />);

    expect(screen.getByText('Выберите архив или папку с архивами.')).toBeInTheDocument();

    const path = screen.getByRole('textbox', { name: 'Архив или папка' });
    await user.type(path, 'C:\\Downloads\\archive.zip');
    await user.keyboard('{Enter}');
    const grid = await screen.findByRole('grid', { name: 'Архив' });
    expect(within(grid).getByText('data/backup.bin')).toBeInTheDocument();
    expect(screen.getByText('Крупнейшие элементы')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Загрузить ещё' }));
    await waitFor(() => {
      expect(within(grid).getByText('readme.txt')).toBeInTheDocument();
    });

    await user.type(
      screen.getByLabelText('Файл экспорта (.csv, .json, .xml, .html)'),
      'C:\\Temp\\archive-list.csv',
    );
    await user.click(screen.getByRole('button', { name: 'Экспортировать список' }));
    expect(
      await screen.findByText('Список экспортирован: C:\\Temp\\archive-list.csv'),
    ).toBeInTheDocument();
  });

  it('reports unsupported and password-protected archives without unpacking', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Archives />);

    const path = screen.getByRole('textbox', { name: 'Архив или папка' });
    await user.type(path, 'C:\\Downloads\\secret-password.7z');
    await user.click(screen.getByRole('button', { name: 'Открыть' }));

    expect(await screen.findByText('Ошибка')).toBeInTheDocument();
    expect(screen.getByText('Операция не поддерживается.')).toBeInTheDocument();
    expect(
      screen.getByText('archive.unsupported.password: the archive is password-protected'),
    ).toBeInTheDocument();
  });
});

describe('export screen', () => {
  it('previews a composition and completes an offline export', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Export />);

    expect(screen.getByRole('checkbox', { name: 'Дерево' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Владельцы' })).toBeChecked();

    const output = screen.getByLabelText('Файл отчёта');
    await user.clear(output);
    await user.type(output, 'C:\\Temp\\report.csv');
    await user.click(screen.getByRole('checkbox', { name: 'Типы' }));
    expect(screen.getByRole('checkbox', { name: 'Типы' })).not.toBeChecked();

    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));
    expect(await screen.findByText('C:\\Temp\\report.csv')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Экспортировать' })).toBeEnabled();

    await user.click(screen.getByRole('button', { name: 'Экспортировать' }));
    expect(
      await screen.findByText(
        'Отчёт записан: C:\\Temp\\report.csv',
        {},
        { timeout: 5000 },
      ),
    ).toBeInTheDocument();
  });

  it('requires an output path and a scanned generation', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Export />);

    const output = screen.getByLabelText('Файл отчёта');
    await user.clear(output);
    await user.click(screen.getByRole('button', { name: 'Предпросмотр' }));
    expect(await screen.findByText('Проверьте введённые значения.')).toBeInTheDocument();
    expect(screen.getAllByText('Файл отчёта').length).toBeGreaterThan(0);

    useSnapshotStore.getState().reset();
    renderWithI18n(<Export />);
    expect(
      screen.getAllByText('Сначала просканируйте том или каталог.').length,
    ).toBeGreaterThan(0);
  });
});

describe('unlock screen', () => {
  it('lists locks and releases them with the soft confirmation', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Unlock />);

    const path = screen.getByRole('textbox', { name: 'Файл' });
    await user.type(path, 'C:\\Temp\\soft-unlock.bin');
    await user.keyboard('{Enter}');

    expect(await screen.findByText('explorer.exe')).toBeInTheDocument();
    expect(screen.getByText('критический')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Попросить приложения закрыть файл' }));
    const dialog = await screen.findByRole('dialog');
    await user.click(
      within(dialog).getByRole('button', { name: 'Попросить приложения закрыть файл' }),
    );
    expect(await screen.findByText('снято')).toBeInTheDocument();
  });

  it('requires a process selection and an acknowledgement for force release', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Unlock />);

    await user.type(screen.getByRole('textbox', { name: 'Файл' }), 'C:\\Temp\\force-unlock.bin');
    await user.keyboard('{Enter}');
    await screen.findByText('explorer.exe');

    const force = screen.getByRole('button', { name: 'Завершить удерживающие процессы' });
    expect(force).toBeDisabled();

    await user.click(screen.getByRole('checkbox', { name: 'Процесс: explorer.exe' }));
    await user.click(force);
    const dialog = await screen.findByRole('dialog');
    const confirm = within(dialog).getByRole('button', {
      name: 'Завершить удерживающие процессы',
    });
    expect(confirm).toBeDisabled();

    await user.click(
      within(dialog).getByRole('checkbox', {
        name: 'Я понимаю, что работающие процессы будут завершены',
      }),
    );
    expect(confirm).toBeEnabled();
    await user.click(confirm);
    expect(await screen.findByText('снято')).toBeInTheDocument();
  });

  it('schedules deletion on reboot and shows the reboot state', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Unlock />);

    await user.type(screen.getByRole('textbox', { name: 'Файл' }), 'C:\\Temp\\reboot-unlock.bin');
    await user.keyboard('{Enter}');
    await screen.findByText('explorer.exe');

    await user.click(screen.getByRole('button', { name: 'Удалить при перезагрузке' }));
    const dialog = await screen.findByRole('dialog');
    const confirm = within(dialog).getByRole('button', { name: 'Удалить при перезагрузке' });
    expect(confirm).toBeDisabled();

    await user.click(within(dialog).getByRole('checkbox', { name: 'Удалить до следующего входа' }));
    await user.click(confirm);

    expect(
      await screen.findByText('Удаление запланировано до следующего входа.'),
    ).toBeInTheDocument();
    expect(
      screen.getByText('Для снятия блокировки требуется перезагрузка.'),
    ).toBeInTheDocument();
  });

  it('requires elevation and handles empty paths and unlocked files', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Unlock />);

    await user.click(screen.getByRole('button', { name: 'Найти блокировки' }));
    expect(await screen.findByText('Проверьте введённые значения.')).toBeInTheDocument();

    const path = screen.getByRole('textbox', { name: 'Файл' });
    await user.type(path, 'C:\\Temp\\protected-unlock.bin');
    await user.keyboard('{Enter}');
    await screen.findByText('explorer.exe');

    await user.click(screen.getByRole('checkbox', { name: 'Процесс: explorer.exe' }));
    await user.click(screen.getByRole('button', { name: 'Завершить удерживающие процессы' }));
    const dialog = await screen.findByRole('dialog');
    await user.click(
      within(dialog).getByRole('checkbox', {
        name: 'Я понимаю, что работающие процессы будут завершены',
      }),
    );
    await user.click(
      within(dialog).getByRole('button', { name: 'Завершить удерживающие процессы' }),
    );

    expect(
      await screen.findByText('Для этой операции нужны права администратора.'),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Запросить повышение прав' })).toBeInTheDocument();

    await user.clear(path);
    await user.type(path, 'C:\\Temp\\free-file.bin');
    await user.keyboard('{Enter}');
    expect(await screen.findByText('Блокирующий процесс не найден.')).toBeInTheDocument();
    expect(screen.getByText('Файл доступен.')).toBeInTheDocument();
  });
});
