import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UiError } from '../../../../src/api/dto';
import type { CommandName } from '../../../../src/api/transport';
import { About } from '../../../../src/screens/About';
import { Elevation } from '../../../../src/screens/Elevation';
import { Monitor } from '../../../../src/screens/Monitor';
import { Settings } from '../../../../src/screens/Settings';
import { Shred } from '../../../../src/screens/Shred';
import { useSettingsStore } from '../../../../src/stores/settingsStore';
import { sharedTransport } from '../../../helpers/mockTransport';
import { renderWithI18n } from '../../../helpers/render';

function uiError(code: UiError['code'], messageKey: string): UiError {
  return {
    code,
    category: code === 'E_ELEVATION_DENIED' ? 'access' : 'internal',
    message_key: messageKey,
    details: null,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };
}

beforeEach(async () => {
  useSettingsStore.setState({ settings: null, busy: false, error: null });

  const transport = sharedTransport();
  await transport.invoke('settings_reset', { section: 'all' });

  let elevated = false;
  transport.setHandler('app_elevation_status', () => ({
    elevated,
    can_elevate: !elevated,
    reason_key: 'elevation.protected_hint',
  }));
  transport.setHandler('app_elevation_request', () => {
    elevated = true;

    return { elevated };
  });
});

afterEach(() => {
  Reflect.deleteProperty(navigator, 'clipboard');
  vi.restoreAllMocks();
});

describe('settings screen', () => {
  it('applies theme, language, density, units and reduced motion immediately', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Settings />);

    const groups = await screen.findAllByRole('radiogroup');
    expect(groups).toHaveLength(4);

    const themeGroup = groups[0];
    expect(themeGroup).toBeDefined();
    await user.click(within(themeGroup!).getAllByRole('radio')[2]!);
    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe('light');
    });

    await user.selectOptions(screen.getByLabelText('Язык'), 'en');
    await waitFor(() => {
      expect(document.documentElement.lang).toBe('en');
    });

    const densityGroup = groups[1];
    await user.click(within(densityGroup!).getAllByRole('radio')[1]!);
    await waitFor(() => {
      expect(document.documentElement.dataset.density).toBe('compact');
    });

    const unitsGroup = groups[2];
    await user.click(within(unitsGroup!).getByRole('radio', { name: '1 KB' }));
    expect(within(unitsGroup!).getByRole('radio', { name: '1 KB' })).toHaveAttribute(
      'aria-checked',
      'true',
    );

    const motionGroup = groups[3];
    await user.click(within(motionGroup!).getAllByRole('radio')[1]!);
    await waitFor(() => {
      expect(document.documentElement.dataset.reducedMotion).toBe('true');
    });
  });

  it('validates exclusions and gates the save button', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Settings />);

    const exclusions = await screen.findByLabelText('Исключаемые пути (через ;)');
    await user.clear(exclusions);
    await user.type(exclusions, 'relative\\folder');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Проверьте введённые значения.',
    );

    await user.clear(exclusions);
    await user.type(exclusions, 'C:\\Temp; **\\node_modules');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));
    expect(await screen.findByText('Готово')).toBeInTheDocument();

    const days = screen.getByLabelText('Считать устаревшим через, дней');
    await user.clear(days);
    expect(screen.getByRole('button', { name: 'Сохранить' })).toBeDisabled();

    await user.type(days, '400');
    expect(screen.getByRole('button', { name: 'Сохранить' })).toBeEnabled();
  });

  it('saves the legacy settings and resets interface preferences', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Settings />);

    const theme = (await screen.findAllByRole('radiogroup'))[0]!;
    await user.click(within(theme).getAllByRole('radio')[1]!);
    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe('dark');
    });

    const days = await screen.findByLabelText('Считать устаревшим через, дней');
    await user.clear(days);
    await user.type(days, '400');

    const rule = screen.getByLabelText('Правило выбора копии при удалении дубликатов');
    await user.selectOptions(rule, 'newest');

    await user.click(screen.getByRole('button', { name: 'Сохранить' }));
    expect(await screen.findByText('Настройки сохранены')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Сбросить' }));
    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe('light');
    });
    expect(days).toHaveValue(400);
    expect(rule).toHaveValue('newest');
  });

  it('shows an error state when settings cannot be read', async () => {
    vi.spyOn(sharedTransport(), 'invoke').mockRejectedValue(
      uiError('E_INTERNAL', 'error.internal'),
    );

    renderWithI18n(<Settings />);

    expect(await screen.findByText('Внутренняя ошибка.')).toBeInTheDocument();
  });
});

describe('shred screen', () => {
  it('needs a preview and an explicit acknowledgement before starting', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Shred />);

    const preview = screen.getByRole('button', { name: 'Проверить' });
    expect(preview).toBeDisabled();
    expect(screen.getByText(/SSD/)).toBeInTheDocument();

    await user.type(
      screen.getByLabelText('Файлы (по одному в строке)'),
      'C:\\Temp\\secret.bin\nD:\\Temp\\old.bin',
    );
    expect(preview).toBeEnabled();
    await user.click(preview);

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText(/Объектов: 2/)).toBeInTheDocument();

    const destroy = within(dialog).getByRole('button', { name: 'Уничтожить' });
    expect(destroy).toBeDisabled();

    await user.click(
      within(dialog).getByRole('checkbox', {
        name: 'Я понимаю, что эти файлы будут уничтожены безвозвратно',
      }),
    );
    expect(destroy).toBeEnabled();
    await user.click(destroy);

    expect(await screen.findByText('Ход операции')).toBeInTheDocument();
    expect(await screen.findByText('Уничтожено файлов: 2')).toBeInTheDocument();
  });

  it('clamps passes to 1..35 and closes the preview with Escape', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Shred />);

    const passes = screen.getByLabelText('Проходов');
    await user.clear(passes);
    await user.type(passes, '99');
    expect(passes).toHaveValue(35);

    await user.type(screen.getByLabelText('Файлы (по одному в строке)'), 'C:\\Temp\\a.bin');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();

    await user.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
  });

  it('reports a rejected preview without starting anything', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Shred />);
    await user.type(screen.getByLabelText('Файлы (по одному в строке)'), 'C:\\Temp\\a.bin');

    vi.spyOn(sharedTransport(), 'invoke').mockRejectedValue(
      uiError('E_VALIDATION', 'error.validation'),
    );
    await user.click(screen.getByRole('button', { name: 'Проверить' }));

    expect(await screen.findByText('Проверьте введённые значения.')).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('shows progress and cancels a running operation', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Shred />);
    await user.type(screen.getByLabelText('Файлы (по одному в строке)'), 'C:\\Temp\\a.bin');
    await user.click(screen.getByRole('button', { name: 'Проверить' }));

    const dialog = await screen.findByRole('dialog');
    await user.click(
      within(dialog).getByRole('checkbox', {
        name: 'Я понимаю, что эти файлы будут уничтожены безвозвратно',
      }),
    );

    const transport = sharedTransport();
    const original = transport.invoke.bind(transport);
    let cancelled = false;
    const progress = {
      operation_id: 'shred-op-test',
      state: 'running' as const,
      processed: 1,
      total: 2,
      current_target: 'C:\\Temp\\a.bin',
      bytes_per_second: null,
      errors: [],
      undo_available: false,
      journal_entry_id: null,
    };
    vi.spyOn(transport, 'invoke').mockImplementation(((
      command: CommandName,
      args?: Record<string, unknown>,
    ) => {
      if (command === 'shred_start') {
        return Promise.resolve({ operation_id: 'shred-op-test' });
      }
      if (command === 'shred_status') {
        return Promise.resolve(
          cancelled ? { ...progress, state: 'cancelled' as const } : progress,
        );
      }
      if (command === 'shred_cancel') {
        cancelled = true;

        return Promise.resolve({});
      }

      return original(command, args);
    }));

    await user.click(within(dialog).getByRole('button', { name: 'Уничтожить' }));
    expect(await screen.findByText('Ход операции')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Отменить операцию' }));
    await waitFor(() => {
      expect(screen.getByText('отменено')).toBeInTheDocument();
    });
  });
});

describe('monitor screen', () => {
  it('starts, filters and stops the live event table', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Monitor />);

    expect(
      screen.getByText('За выбранный период записи не обнаружены.'),
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Запустить мониторинг' }));

    expect(await screen.findByText('explorer.exe')).toBeInTheDocument();
    expect(screen.getByText(/Источник событий: ETW/)).toBeInTheDocument();

    await user.type(screen.getByRole('searchbox', { name: 'Процесс' }), 'backup');
    await waitFor(() => {
      expect(screen.queryByText('explorer.exe')).not.toBeInTheDocument();
    });
    expect(screen.getByText('backup-agent.exe')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Остановить мониторинг' }));
    expect(await screen.findByText('Мониторинг завершён')).toBeInTheDocument();
  });

  it('appends monitor-tick events to the table', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Monitor />);
    await user.click(screen.getByRole('button', { name: 'Запустить мониторинг' }));
    expect(await screen.findByText('explorer.exe')).toBeInTheDocument();

    const taskId = screen.getByTestId('monitor-status').getAttribute('data-task-id') ?? '';

    act(() => {
      sharedTransport().emit(
        'monitor-tick',
        {
          task_id: taskId,
          sequence: 99,
          events: [
            {
              sequence: 99,
              timestamp: Date.now(),
              process_id: 99,
              process: 'injected.exe',
              bytes: 1024,
              paths: ['C:\\Temp\\injected.bin'],
              threshold_exceeded: false,
            },
          ],
        },
        taskId,
      );
    });

    expect(await screen.findByText('injected.exe')).toBeInTheDocument();
  });

  it('validates the interval before starting', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Monitor />);

    const interval = screen.getByLabelText('Интервал опроса, секунд');
    await user.clear(interval);
    await user.type(interval, '0');
    await user.click(screen.getByRole('button', { name: 'Запустить мониторинг' }));

    expect(screen.getByRole('alert')).toHaveTextContent('Проверьте введённые значения.');
  });

  it('marks counters-only sessions with the fallback banner', async () => {
    const user = userEvent.setup();
    const transport = sharedTransport();
    const original = transport.invoke.bind(transport);
    vi.spyOn(transport, 'invoke').mockImplementation(((
      command: CommandName,
      args?: Record<string, unknown>,
    ) => {
      if (command === 'monitor_start') {
        return Promise.resolve({ task_id: 'monitor-counters' });
      }
      if (command === 'monitor_status') {
        return Promise.resolve({
          task_id: 'monitor-counters',
          running: true,
          state: 'running',
          source: 'counters',
          source_key: 'monitor.source.counters',
          threshold_bytes: 0,
          interval_secs: 2,
          exclusions: [],
          events_count: 1,
          bytes_total: 2048,
          started_at: 0,
          finished_at: null,
          last_event_at: 0,
          warnings: ['monitor.fallback'],
          error: null,
        });
      }
      if (command === 'monitor_events_page') {
        return Promise.resolve({
          task_id: 'monitor-counters',
          cursor: null,
          next_cursor: null,
          total_estimate: 1,
          events: [
            {
              sequence: 1,
              timestamp: Date.now(),
              process_id: 7,
              process: 'counter.exe',
              bytes: 2048,
              paths: ['C:\\Temp\\counter.bin'],
              threshold_exceeded: true,
            },
          ],
        });
      }

      return original(command, args);
    }));

    renderWithI18n(<Monitor />);
    await user.click(screen.getByRole('button', { name: 'Запустить мониторинг' }));

    expect(
      await screen.findByText(
        'Источник событий: счётчики процессов и USN (ограниченный режим)',
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(/Сеанс ETW недоступен/)).toBeInTheDocument();
  });
});

describe('elevation screen', () => {
  it('requests elevation and switches to the administrator state', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Elevation />);

    expect(
      await screen.findByText('Обычный пользователь (ограниченный режим)'),
    ).toBeInTheDocument();
    expect(screen.getByText('Доступен')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Запросить повышение прав' }));

    expect(await screen.findByText('Администратор')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Запросить повышение прав' }),
    ).toBeDisabled();
  });

  it('shows the limited mode after a refusal', async () => {
    const user = userEvent.setup();
    sharedTransport().setHandler('app_elevation_request', () => {
      throw Object.assign(
        new Error('elevation denied'),
        uiError('E_ELEVATION_DENIED', 'error.elevation_denied'),
      );
    });

    renderWithI18n(<Elevation />);
    await user.click(
      await screen.findByRole('button', { name: 'Запросить повышение прав' }),
    );

    expect(
      await screen.findByText(
        'Повышение прав отклонено; анализ продолжится в ограниченном режиме.',
      ),
    ).toBeInTheDocument();
  });

  it('surfaces a status error with a retry action', async () => {
    vi.spyOn(sharedTransport(), 'invoke').mockRejectedValue(
      uiError('E_INTERNAL', 'error.internal'),
    );

    renderWithI18n(<Elevation />);

    expect(await screen.findByText('Внутренняя ошибка.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Повторить' })).toBeInTheDocument();
  });
});

describe('about screen', () => {
  it('shows a loading placeholder while the payload is pending', () => {
    vi.spyOn(sharedTransport(), 'invoke').mockImplementation(
      () => new Promise(() => undefined),
    );

    renderWithI18n(<About />);

    expect(screen.getByText('Загрузка…')).toBeInTheDocument();
  });

  it('shows version, build, WebView2, data directory and licences', async () => {
    renderWithI18n(<About />);

    expect(await screen.findByText('1.1.0')).toBeInTheDocument();
    expect(screen.getByText('mock-002')).toBeInTheDocument();
    expect(screen.getByText('130.0.2849.68')).toBeInTheDocument();
    expect(screen.getByText('C:\\FreeSpace\\data')).toBeInTheDocument();
    expect(screen.getByRole('grid', { name: 'Лицензии' })).toBeInTheDocument();
    expect(screen.getAllByText('MIT').length).toBeGreaterThan(0);
    expect(screen.queryAllByRole('link')).toHaveLength(0);
  });

  it('copies the data directory without opening a link', async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    renderWithI18n(<About />);
    await user.click(await screen.findByRole('button', { name: 'Копировать путь' }));

    expect(writeText).toHaveBeenCalledWith('C:\\FreeSpace\\data');
    expect(await screen.findByText('Путь скопирован')).toBeInTheDocument();
    expect(screen.queryAllByRole('link')).toHaveLength(0);
  });

  it('shows an error state when about info fails', async () => {
    vi.spyOn(sharedTransport(), 'invoke').mockRejectedValue(
      uiError('E_INTERNAL', 'error.internal'),
    );

    renderWithI18n(<About />);

    expect(await screen.findByText('Внутренняя ошибка.')).toBeInTheDocument();
  });
});
