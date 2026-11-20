import { useEffect, useRef, useState } from 'react';
import {
  Activity,
  BarChart3,
  Command,
  Copy,
  FolderTree,
  HardDrive,
  PanelLeftClose,
  PanelLeftOpen,
  ScrollText,
  Settings,
  Wrench,
  type LucideIcon,
} from 'lucide-react';
import { Broom } from './icons';

import { api } from './api/runtime';
import type {
  EventEnvelope,
  ScanCompletedEvent,
  ScanProgressEvent,
  ScreenId,
  Task,
  UiSettings,
} from './api/dto';
import { AppShell, Header, Sidebar, SidebarDrive, StatusBar } from './app/AppShell';
import { CommandPalette, type PaletteCommand } from './app/CommandPalette';
import { ErrorBoundary } from './app/ErrorBoundary';
import { useHotkeys } from './app/hotkeys';
import {
  GROUPS,
  SCREENS,
  SCREEN_BY_ID,
  groupEntries,
  normalizeScreenId,
} from './app/routing';
import { startSystemThemeSync } from './app/theme';
import { Button, IconButton } from './components/Button';
import { ErrorState, Loading } from './components/Feedback';
import { setLanguage, t } from './i18n';
import { formatBytes, formatNumber } from './i18n/format';
import { ScreenRouter } from './screens/ScreenRouter';
import { useSettingsStore } from './stores/settingsStore';
import { useShellStore } from './stores/shellStore';
import { useSnapshotStore } from './stores/snapshotStore';
import { useTasksStore } from './stores/tasksStore';

/** One glyph per screen; the shared Lucide set keeps the navigation coherent. */
const SCREEN_ICON: Record<ScreenId, LucideIcon> = {
  overview: HardDrive,
  tree: FolderTree,
  analysis: BarChart3,
  duplicates: Copy,
  cleanup: Broom,
  journal: ScrollText,
  monitor: Activity,
  inspect: Wrench,
  settings: Settings,
};

/** Application root: bootstrap, shell, events and the command palette. */
export function App() {
  const ready = useShellStore((state) => state.ready);
  const error = useShellStore((state) => state.error);
  const bootstrap = useShellStore((state) => state.bootstrap);
  const screen = useShellStore((state) => state.screen);
  const sidebarCollapsed = useShellStore((state) => state.sidebarCollapsed);
  const load = useShellStore((state) => state.load);
  const setScreen = useShellStore((state) => state.setScreen);
  const toggleSidebar = useShellStore((state) => state.toggleSidebar);
  const tasks = useTasksStore((state) => state.tasks);
  const setTasks = useTasksStore((state) => state.setTasks);
  const applyProgress = useTasksStore((state) => state.applyProgress);
  const updateState = useTasksStore((state) => state.updateState);
  const setSettings = useSettingsStore((state) => state.setSettings);
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const volume = useSnapshotStore((state) => state.volume);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const history = useRef<{ entries: ScreenId[]; index: number }>({
    entries: [screen],
    index: 0,
  });
  const skipHistory = useRef(false);

  useEffect(() => {
    void load(api);
  }, [load]);

  useEffect(() => {
    if (bootstrap === null) return;
    setLanguage(bootstrap.settings.language);
    setSettings(bootstrap.settings);
  }, [bootstrap, setSettings]);

  // A language switch updates the store; subscribing to it re-renders the
  // shell so navigation, header and palette labels change immediately.
  useEffect(() => {
    setLanguage(language);
  }, [language]);

  // Follow OS theme and motion changes while the app is open.
  useEffect(() => startSystemThemeSync(), []);

  useEffect(() => {
    let disposed = false;
    const cleanups: (() => void)[] = [];

    const subscribe = async () => {
      const queue = await api.transport.listen<{ tasks: Task[] }>(
        'task-queue-changed',
        (envelope) => {
          setTasks(envelope.payload.tasks);
        },
      );
      const progress = await api.transport.listen<ScanProgressEvent>(
        'scan-progress',
        (envelope) => {
          const taskId = envelope.task_id;
          if (taskId === null) return;
          const event = envelope.payload;
          applyProgress(taskId, {
            processed: event.progress.processed,
            total: event.progress.total,
            current_path: event.current_path,
            bytes_per_second: event.bytes_per_second,
            items_per_second: null,
            phase_key: event.phase_key,
            updated_at: Date.now(),
          });
        },
      );
      const completed = await api.transport.listen<ScanCompletedEvent>(
        'scan-completed',
        (envelope) => {
          const event = envelope.payload;
          updateState(event.task_id, event.state, true);
          if (event.state === 'completed' || event.state === 'partial') {
            useSnapshotStore.getState().setGeneration(event.generation);
            void api
              .volumesCurrent()
              .then((volume) => {
                useSnapshotStore.getState().setVolume(volume);
              })
              .catch(() => undefined);
          }
        },
      );
      const settings = await api.transport.listen<{ settings: UiSettings }>(
        'settings-changed',
        (envelope) => {
          setSettings(envelope.payload.settings);
        },
      );
      const navigation = await api.transport.listen<{
        screen: string;
        node_id: number | null;
        path: string | null;
      }>('navigation-request', (envelope) => {
        const target = normalizeScreenId(envelope.payload.screen);
        if (target !== null) {
          setScreen(target);
        }
      });

      if (disposed) {
        for (const unsubscribe of [queue, progress, completed, settings, navigation]) {
          unsubscribe();
        }
        return;
      }

      cleanups.push(queue, progress, completed, settings, navigation);
    };

    void subscribe();

    return () => {
      disposed = true;
      for (const cleanup of cleanups) cleanup();
    };
  }, [applyProgress, setScreen, setSettings, setTasks, updateState]);

  useEffect(() => {
    if (!ready || screen === 'overview') return;
    void api.appSettingsSet({ last_screen: screen });
  }, [ready, screen]);

  // History for Alt+Left / Alt+Right; back/forward navigation must not append.
  useEffect(() => {
    if (skipHistory.current) {
      skipHistory.current = false;
      return;
    }
    const stack = history.current;
    const current = stack.entries[stack.index];
    if (current === screen) return;
    stack.entries = [...stack.entries.slice(0, stack.index + 1), screen];
    stack.index = stack.entries.length - 1;
  }, [screen]);

  const navigateHistory = (delta: number) => {
    const stack = history.current;
    const next = stack.index + delta;
    const target = stack.entries[next];
    if (target === undefined) return;
    stack.index = next;
    skipHistory.current = true;
    setScreen(target);
  };

  useHotkeys({
    'Ctrl+K': () => {
      setPaletteOpen(true);
    },
    'Ctrl+Shift+K': () => {
      setPaletteOpen(false);
    },
    'Ctrl+,': () => {
      setScreen('settings');
    },
    F1: () => {
      setScreen('settings');
    },
    F6: () => {
      document.querySelector<HTMLElement>('[data-status-bar]')?.focus();
    },
    'Ctrl+F': () => {
      document
        .querySelector<HTMLElement>(
          '#main-content input[type="search"], #main-content [role="searchbox"]',
        )
        ?.focus();
    },
    'Alt+ArrowLeft': () => {
      navigateHistory(-1);
    },
    'Alt+ArrowRight': () => {
      navigateHistory(1);
    },
    ...Object.fromEntries(
      GROUPS.map((group, index) => [
        `Ctrl+${index + 1}`,
        () => {
          const target = group.screens[0];
          if (target !== undefined) setScreen(target);
        },
      ]),
    ),
  });

  const commands: PaletteCommand[] = SCREENS.map((item) => ({
    id: item.id,
    label: t(item.titleKey),
    hint: t(`nav.group.${item.group}`),
    run: () => {
      setScreen(item.id);
    },
  }));

  const running = tasks.find(
    (task) => task.state === 'running' || task.state === 'cancelling',
  );
  const progress = running?.progress ?? null;
  const meta = SCREEN_BY_ID[screen];

  return (
    <ErrorBoundary>
      <AppShell
        sidebar={
          <Sidebar
            groups={groupEntries().map(({ group, screens }) => ({
              id: group.id,
              label: t(group.titleKey),
              items: screens.map((item) => {
                const Icon = SCREEN_ICON[item.id];

                return {
                  id: item.id,
                  label: t(item.titleKey),
                  icon: <Icon size={16} strokeWidth={1.7} />,
                };
              }),
            }))}
            active={screen}
            collapsed={sidebarCollapsed}
            onSelect={(id) => {
              const target = normalizeScreenId(id);
              if (target !== null) setScreen(target);
            }}
            footer={
              volume === null ? undefined : (
                <SidebarDrive
                  mount={volume.mount_point}
                  free={formatBytes(volume.free_bytes, units)}
                  total={formatBytes(volume.total_bytes, units)}
                />
              )
            }
          />
        }
        header={
          <Header
            title={t(meta.titleKey)}
            actions={
              <>
                <IconButton
                  label={t('a11y.toggle_sidebar')}
                  onClick={() => {
                    toggleSidebar();
                    void api.appSettingsSet({ sidebar_collapsed: !sidebarCollapsed });
                  }}
                >
                  {sidebarCollapsed ? (
                    <PanelLeftOpen size={16} strokeWidth={1.6} />
                  ) : (
                    <PanelLeftClose size={16} strokeWidth={1.6} />
                  )}
                </IconButton>
                <Button
                  variant="secondary"
                  icon={<Command size={14} strokeWidth={1.6} aria-hidden="true" />}
                  onClick={() => {
                    setPaletteOpen(true);
                  }}
                >
                  {t('palette.open')}
                </Button>
              </>
            }
          />
        }
        status={
          <StatusBar
            message={running === undefined ? t('status.ready') : undefined}
            task={
              progress !== null
                ? `${t(progress.phase_key)} · ${formatNumber(progress.processed)}`
                : running !== undefined
                  ? t('status.running')
                  : undefined
            }
            right={bootstrap === null ? undefined : `v${bootstrap.app_version}`}
          />
        }
      >
        {!ready ? (
          <Loading label={t('common.loading')} rows={5} />
        ) : error !== null ? (
          <ErrorState
            title={t(error.message_key)}
            code={error.code}
            details={error.details ?? undefined}
            actions={
              <Button
                variant="primary"
                onClick={() => {
                  void load(api);
                }}
              >
                {t('error.retry')}
              </Button>
            }
          />
        ) : (
          <ScreenRouter screen={screen} />
        )}
      </AppShell>
      <CommandPalette
        open={paletteOpen}
        onClose={() => {
          setPaletteOpen(false);
        }}
        commands={commands}
        placeholder={t('palette.placeholder')}
        emptyLabel={t('palette.empty')}
      />
    </ErrorBoundary>
  );
}

/** Re-exported event type used by tests and future stores. */
export type { EventEnvelope };
