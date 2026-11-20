import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { UiError } from '../../../../src/api/dto';
import type { CommandName } from '../../../../src/api/transport';
import { ApiClient } from '../../../../src/api/client';
import { MockTransport } from '../../../../src/api/mockTransport';
import { t } from '../../../../src/i18n';
import { LINUX_CAPABILITIES } from '../../../../src/lib/capabilities';
import { About } from '../../../../src/screens/About';
import { Elevation } from '../../../../src/screens/Elevation';
import { Monitor } from '../../../../src/screens/Monitor';
import { Settings } from '../../../../src/screens/Settings';
import { useSettingsStore } from '../../../../src/stores/settingsStore';
import { useShellStore } from '../../../../src/stores/shellStore';
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
  useShellStore.setState({ bootstrap: null });
  vi.restoreAllMocks();
});

describe('settings screen', () => {
  it('applies theme, language, density, units and reduced motion immediately', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Settings />);

    const groups = await screen.findAllByRole('radiogroup');
    expect(groups).toHaveLength(4);
    expect(screen.queryByLabelText(/Хранение карантина/)).toBeNull();

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

    const panel = await screen.findByRole('tabpanel', { name: 'Настройки' });
    expect(await within(panel).findByText('Внутренняя ошибка.')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Повторить' })).toBeInTheDocument();
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

    const interval = screen.getByLabelText('Интервал опроса, секунд (1 – 86400)');
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

describe('settings tabs', () => {
  it('keeps settings, elevation and about in one screen with three tabs', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Settings />);

    const tabs = screen.getAllByRole('tab');
    expect(tabs.map((tab) => tab.textContent)).toEqual([
      'Настройки',
      'Права доступа',
      'О программе',
    ]);
    expect(screen.getByRole('tab', { name: 'Настройки' })).toHaveAttribute(
      'aria-selected',
      'true',
    );

    await user.click(screen.getByRole('tab', { name: 'Права доступа' }));
    const elevation = await screen.findByRole('tabpanel', { name: 'Права доступа' });
    expect(within(elevation).getByText('Текущий режим')).toBeInTheDocument();
    expect(screen.queryByRole('tabpanel', { name: 'Настройки' })).not.toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: 'О программе' }));
    const about = await screen.findByRole('tabpanel', { name: 'О программе' });
    expect(await within(about).findByText('1.1.0')).toBeInTheDocument();
    expect(within(about).getByText('130.0.2849.68')).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: 'Настройки' }));
    expect(await screen.findByLabelText('Язык')).toBeInTheDocument();
  });

  it('hides the context menu setting and the elevation tab on a backend without them', async () => {
    const transport = new MockTransport();
    transport.platformCapabilities = {
      ...LINUX_CAPABILITIES,
      context_menu: false,
      elevation: false,
    };
    await useShellStore.getState().load(new ApiClient(transport));

    renderWithI18n(<Settings />);

    expect(await screen.findByLabelText('Язык')).toBeInTheDocument();
    expect(screen.queryByText(t('settings.context_menu'))).not.toBeInTheDocument();
    const tabs = screen.getAllByRole('tab').map((tab) => tab.textContent);
    expect(tabs).toEqual([t('nav.settings'), t('nav.about')]);
  });
});

describe('elevation tab', () => {
  it('restarts with administrator rights only after the consequences are confirmed', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Elevation />);

    expect(
      await screen.findByText('Обычный пользователь (ограниченный режим)'),
    ).toBeInTheDocument();
    expect(screen.getByText('Доступен')).toBeInTheDocument();

    await user.click(
      screen.getByRole('button', { name: 'Перезапустить от имени администратора' }),
    );

    const dialog = await screen.findByRole('dialog');
    expect(
      within(dialog).getByText(t('system.elevation.restart_title')),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(t('system.elevation.restart_consequences')),
    ).toBeInTheDocument();

    await user.click(
      within(dialog).getByRole('button', {
        name: t('system.elevation.restart_confirm'),
      }),
    );

    expect(await screen.findByText('Администратор')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Перезапустить от имени администратора' }),
    ).toBeDisabled();
  });

  it('stays in the limited mode when the restart is cancelled', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Elevation />);

    await user.click(
      await screen.findByRole('button', {
        name: 'Перезапустить от имени администратора',
      }),
    );

    const dialog = await screen.findByRole('dialog');
    await user.click(
      within(dialog).getByRole('button', {
        name: t('system.elevation.restart_cancel'),
      }),
    );

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
    expect(
      screen.getByText('Обычный пользователь (ограниченный режим)'),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Перезапустить от имени администратора' }),
    ).toBeEnabled();
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
      await screen.findByRole('button', {
        name: 'Перезапустить от имени администратора',
      }),
    );
    const dialog = await screen.findByRole('dialog');
    await user.click(
      within(dialog).getByRole('button', {
        name: t('system.elevation.restart_confirm'),
      }),
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
