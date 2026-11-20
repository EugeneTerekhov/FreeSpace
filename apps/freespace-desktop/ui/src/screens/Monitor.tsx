import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Folder, FolderOpen } from 'lucide-react';

import type { MonitorEvent, MonitorEventFilter, MonitorTickEvent } from '../api/dto';
import type { Unlisten } from '../api/transport';
import {
  monitorEventsPage,
  monitorRevealExecutable,
  monitorRevealFile,
  monitorSetExclusions,
  monitorStart,
  monitorStatus,
  monitorStop,
} from '../api/settingsOps';
import { api } from '../api/runtime';
import { normalizeError } from '../app/errors';
import { useLiveAnnouncer } from '../app/live';
import { Button, IconButton } from '../components/Button';
import { Badge, Checkbox } from '../components/Choice';
import { DataTable, type Column, type RowAnchor, type SortState } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, LiveRegions } from '../components/Feedback';
import { Field, SearchField } from '../components/Fields';
import {
  HoverCard,
  HoverCardHead,
  HoverCardNote,
  HoverCardPath,
  HoverCardRow,
  HoverCardRows,
} from '../components/HoverCard';
import { t, tPlural } from '../i18n';
import { formatBytes, formatDateTimeShort, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { MONITOR_MAX_INTERVAL, useMonitorStore } from '../stores/monitorStore';
import { Panel, Screen } from './ScreenFrame';
import styles from './operations.module.css';

const HOVER_DELAY = 450;

function splitList(value: string): string[] {
  return value
    .split(/\r?\n|;/)
    .map((item) => item.trim())
    .filter((item) => item !== '');
}

function matchesEvent(event: MonitorEvent, process: string, path: string): boolean {
  const processNeedle = process.trim().toLowerCase();
  const pathNeedle = path.trim().toLowerCase();
  if (processNeedle !== '' && !event.process.toLowerCase().includes(processNeedle)) {
    return false;
  }
  if (
    pathNeedle !== '' &&
    !event.paths.some((item) => item.toLowerCase().includes(pathNeedle))
  ) {
    return false;
  }

  return true;
}

/**
 * Live write monitor (T062).
 *
 * The session state lives in `useMonitorStore`, not in this component: the
 * router unmounts a screen when another one is opened, and the monitor must
 * keep its task id, counters, events and form values across that navigation.
 * Events arrive through `monitor-tick`, while the cursor page is polled as a
 * fallback so the table also works with a transport that only answers commands
 * (the browser mock and the contract fixtures). New events are announced into a
 * polite live region at the throttled `progress` rate, so a busy disk never
 * floods the narrator.
 */
export function Monitor() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');

  const threshold = useMonitorStore((state) => state.threshold);
  const interval = useMonitorStore((state) => state.interval);
  const exclusions = useMonitorStore((state) => state.exclusions);
  const processFilter = useMonitorStore((state) => state.processFilter);
  const pathFilter = useMonitorStore((state) => state.pathFilter);
  const thresholdOnly = useMonitorStore((state) => state.thresholdOnly);
  const taskId = useMonitorStore((state) => state.taskId);
  const running = useMonitorStore((state) => state.running);
  const status = useMonitorStore((state) => state.status);
  const events = useMonitorStore((state) => state.events);
  const loading = useMonitorStore((state) => state.loading);
  const validation = useMonitorStore((state) => state.validation);
  const error = useMonitorStore((state) => state.error);
  const notice = useMonitorStore((state) => state.notice);
  const { polite, assertive, announce } = useLiveAnnouncer();
  const [sort, setSort] = useState<SortState>({ key: 'date', descending: true });
  const [hoverRow, setHoverRow] = useState<{ event: MonitorEvent; anchor: RowAnchor } | null>(
    null,
  );
  const [folderError, setFolderError] = useState<string | null>(null);
  const hoverTimer = useRef<number | null>(null);

  const filter = useMemo<MonitorEventFilter>(
    () => ({
      process: processFilter.trim() === '' ? null : processFilter,
      path: pathFilter.trim() === '' ? null : pathFilter,
      ...(thresholdOnly ? { threshold_only: true } : {}),
    }),
    [pathFilter, processFilter, thresholdOnly],
  );

  const visibleEvents = useMemo(() => {
    const filtered = events
      .filter((event) => matchesEvent(event, processFilter, pathFilter))
      .filter((event) => !thresholdOnly || event.threshold_exceeded);

    return [...filtered].sort((left, right) =>
      sort.descending
        ? right.timestamp - left.timestamp
        : left.timestamp - right.timestamp,
    );
  }, [events, pathFilter, processFilter, sort, thresholdOnly]);

  // Numeric columns are sized to the widest value actually on screen, so no
  // dead space is left between them and the row actions. `ch` resolves against
  // the grid font, which DataTable keeps identical in the header and rows.
  const valueWidth = (values: string[], minChars = 3): string => {
    const chars = values.reduce((max, value) => Math.max(max, value.length), minChars);
    return `calc(${chars}ch + var(--fs-table-gap))`;
  };

  // Announce only sequences that have not been announced yet (the cursor is
  // kept in the store, so a screen remount cannot re-announce old events),
  // at the throttled `progress` rate owned by `LiveAnnouncer`.
  const announceEvents = (incoming: MonitorEvent[]) => {
    const fresh = useMonitorStore.getState().takeFresh(incoming);
    if (fresh.length === 0) return;
    announce(t('operations.monitor.new_events', { count: fresh.length }), {
      priority: 'progress',
    });
  };

  const closeHover = useCallback(() => {
    if (hoverTimer.current !== null) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = null;
    }
    setHoverRow(null);
  }, []);

  const openHover = useCallback((event: MonitorEvent, anchor: RowAnchor) => {
    if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => {
      hoverTimer.current = null;
      setHoverRow({ event, anchor });
    }, HOVER_DELAY);
  }, []);

  useEffect(
    () => () => {
      if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
    },
    [],
  );

  // A card that stays put while its row scrolls away would point at nothing.
  useEffect(() => {
    if (hoverRow === null) return undefined;
    const hide = () => closeHover();
    const onKeyDown = (keyEvent: KeyboardEvent) => {
      if (keyEvent.key === 'Escape') hide();
    };
    document.addEventListener('scroll', hide, true);
    document.addEventListener('keydown', onKeyDown);
    window.addEventListener('resize', hide);

    return () => {
      document.removeEventListener('scroll', hide, true);
      document.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('resize', hide);
    };
  }, [closeHover, hoverRow]);

  const revealFile = async (event: MonitorEvent) => {
    const path = event.paths[0];
    if (path === undefined) return;
    setFolderError(null);
    try {
      await monitorRevealFile(path);
    } catch (cause) {
      setFolderError(t(normalizeError(cause).message_key));
    }
  };

  const revealExecutable = async (event: MonitorEvent) => {
    setFolderError(null);
    try {
      await monitorRevealExecutable(event.process_id);
    } catch (cause) {
      setFolderError(t(normalizeError(cause).message_key));
    }
  };

  useEffect(() => {
    if (taskId === null || !running) return undefined;

    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const poll = async () => {
      try {
        const [nextStatus, page] = await Promise.all([
          monitorStatus(taskId),
          monitorEventsPage(taskId, filter),
        ]);
        if (disposed) return;
        const actions = useMonitorStore.getState();
        actions.setStatus(nextStatus);
        actions.setEvents(page.events);
        announceEvents(page.events);
        if (nextStatus.running) {
          const seconds = Math.max(1, nextStatus.interval_secs);
          timer = setTimeout(() => void poll(), seconds * 1000);
        } else {
          actions.setRunning(false);
        }
      } catch (cause) {
        if (disposed) return;
        const actions = useMonitorStore.getState();
        actions.setError(normalizeError(cause));
        actions.setRunning(false);
      }
    };

    void poll();

    return () => {
      disposed = true;
      if (timer !== null) clearTimeout(timer);
    };
    // `filter` changes restart the poll with the new server-side filter.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filter, running, taskId]);

  useEffect(() => {
    if (taskId === null) return undefined;

    let disposed = false;
    let unlisten: Unlisten | null = null;

    // `monitor-tick` batches are appended as they arrive; the polling effect
    // above remains the fallback for transports that only answer commands.
    void api.transport
      .listen<MonitorTickEvent>('monitor-tick', (envelope) => {
        const payload = envelope.payload;
        if (payload.task_id !== taskId) return;
        useMonitorStore.getState().mergeEvents(payload.events);
        announceEvents(payload.events);
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId]);

  const start = async () => {
    const thresholdBytes = Number(threshold);
    const intervalSeconds = Number(interval);

    if (
      !Number.isInteger(thresholdBytes) ||
      thresholdBytes < 0 ||
      !Number.isInteger(intervalSeconds) ||
      intervalSeconds < 1 ||
      intervalSeconds > MONITOR_MAX_INTERVAL
    ) {
      useMonitorStore.getState().setValidation(t('error.validation'));
      return;
    }

    const actions = useMonitorStore.getState();
    actions.setValidation(null);
    actions.setError(null);
    actions.setNotice(null);
    actions.setLoading(true);
    try {
      const started = await monitorStart({
        threshold: thresholdBytes,
        interval: intervalSeconds,
        exclusions: splitList(exclusions),
      });
      actions.beginSession(started.task_id);
    } catch (cause) {
      actions.setError(normalizeError(cause));
    } finally {
      actions.setLoading(false);
    }
  };

  const stop = async () => {
    const currentTaskId = useMonitorStore.getState().taskId;
    if (currentTaskId === null) return;
    const actions = useMonitorStore.getState();
    actions.setLoading(true);
    actions.setError(null);
    try {
      await monitorStop(currentTaskId);
      actions.setStatus(await monitorStatus(currentTaskId));
      actions.setRunning(false);
      actions.setNotice(t('monitor.summary'));
    } catch (cause) {
      actions.setError(normalizeError(cause));
    } finally {
      actions.setLoading(false);
    }
  };

  const applyExclusions = async () => {
    if (useMonitorStore.getState().taskId === null || !useMonitorStore.getState().running) {
      return;
    }
    try {
      await monitorSetExclusions(splitList(exclusions));
    } catch (cause) {
      useMonitorStore.getState().setError(normalizeError(cause));
    }
  };

  const sourceLabel =
    status === null
      ? null
      : status.source_key !== ''
        ? t(status.source_key)
        : t(status.source === 'counters' ? 'monitor.source.counters' : 'monitor.source.etw');

  const stateLabel = running
    ? t('monitor.running')
    : status !== null
      ? t('status.finished')
      : t('status.ready');

  // The counter fallback reports no file paths: hide the «Путь» column and
  // its file-folder action, but keep the executable-folder action.
  const pathAvailable = status?.source !== 'counters';
  // Free width is split 2:1 between the path and the owner. When the counter
  // fallback hides the path, the owner's 1fr track absorbs all of it.
  const pathColumn: Column<MonitorEvent> = {
    id: 'path',
    header: t('monitor.path'),
    width: 'minmax(220px, 2fr)',
    render: (row) => {
      const path = row.paths[0] ?? '';

      return (
        <span className={styles.pathCell} title={path}>
          {path === '' ? '—' : path}
        </span>
      );
    },
  };

  const openColumn: Column<MonitorEvent> = {
    id: 'open',
    header: '',
    // Two 30px buttons + 4px gap + the shared cell padding (16px) need 80px;
    // without the file-folder action one button needs 48px. A narrower track
    // clips the icons and paints an ellipsis.
    width: pathAvailable ? '80px' : '48px',
    render: (row) => (
      <span className={styles.iconGroup}>
        {pathAvailable ? (
          <IconButton
            label={t('monitor.open_file_folder')}
            disabled={row.paths.length === 0}
            onClick={() => void revealFile(row)}
          >
            <Folder size={15} strokeWidth={1.7} aria-hidden="true" />
          </IconButton>
        ) : null}
        <IconButton
          label={t('monitor.open_executable_folder')}
          disabled={row.process_id === 0}
          onClick={() => void revealExecutable(row)}
        >
          <FolderOpen size={15} strokeWidth={1.7} aria-hidden="true" />
        </IconButton>
      </span>
    ),
  };

  const columns: Column<MonitorEvent>[] = [
    {
      id: 'date',
      header: t('monitor.date'),
      width: '166px',
      sortable: true,
      render: (row) => (
        <span className={styles.dateCell}>{formatDateTimeShort(row.timestamp)}</span>
      ),
    },
    {
      id: 'owner',
      header: t('operations.monitor.owner'),
      width: 'minmax(140px, 1fr)',
      render: (row) => <span className={styles.owner}>{row.process}</span>,
    },
    ...(pathAvailable ? [pathColumn] : []),
    {
      id: 'bytes',
      header: t('monitor.bytes'),
      width: valueWidth(
        visibleEvents.map((row) => formatBytes(row.bytes, units)),
        6,
      ),
      align: 'end',
      render: (row) => formatBytes(row.bytes, units),
    },
    {
      id: 'pid',
      header: t('monitor.pid'),
      width: valueWidth(
        visibleEvents.map((row) =>
          row.process_id === 0 ? '—' : formatNumber(row.process_id, language),
        ),
      ),
      render: (row) => (
        <span className={styles.pidCell}>
          {row.process_id === 0 ? '—' : formatNumber(row.process_id, language)}
        </span>
      ),
    },
    openColumn,
  ];

  return (
    <Screen
      context={t('operations.monitor.context', {
        count: status?.events_count ?? events.length,
        size: formatBytes(status?.bytes_total ?? 0, units),
      })}
    >
      <LiveRegions polite={polite} assertive={assertive} />

      <Panel title={t('operations.monitor.form')} hint={t('monitor.intro')}>
        <div className={styles.formGrid}>
          <div className={styles.paramGrid}>
            <Field
              id="monitor-threshold"
              label={`${t('monitor.threshold')} (<1 GiB)`}
            >
              <input
                id="monitor-threshold"
                className={`${styles.control} ${styles.number}`}
                type="number"
                min={0}
                step={1024 * 1024}
                value={threshold}
                onChange={(event) => {
                  useMonitorStore.getState().setThreshold(event.target.value);
                }}
              />
            </Field>

            <Field
              id="monitor-interval"
              label={`${t('monitor.interval')} (1 – ${MONITOR_MAX_INTERVAL})`}
            >
              <input
                id="monitor-interval"
                className={`${styles.control} ${styles.number}`}
                type="number"
                min={1}
                max={MONITOR_MAX_INTERVAL}
                step={1}
                value={interval}
                onChange={(event) => {
                  useMonitorStore.getState().setInterval(event.target.value);
                }}
              />
            </Field>
          </div>

          <div className={styles.wide}>
            <Field id="monitor-exclusions" label={t('settings.exclusions')}>
              <textarea
                id="monitor-exclusions"
                className={styles.control}
                rows={2}
                value={exclusions}
                onChange={(event) => {
                  useMonitorStore.getState().setExclusions(event.target.value);
                }}
                onBlur={() => void applyExclusions()}
              />
            </Field>
          </div>

          <div className={styles.inline}>
            {running ? (
              <Button
                variant="secondary"
                loading={loading}
                className={styles.monitorAction}
                onClick={() => void stop()}
              >
                {t('monitor.stop')}
              </Button>
            ) : (
              <Button
                variant="primary"
                loading={loading}
                className={styles.monitorAction}
                onClick={() => void start()}
              >
                {t('monitor.start')}
              </Button>
            )}
          </div>
        </div>

        {validation !== null ? <Banner tone="error">{validation}</Banner> : null}
      </Panel>

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void start()} disabled={running}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {notice !== null ? <Banner tone="info">{notice}</Banner> : null}
      {folderError !== null ? <Banner tone="error">{folderError}</Banner> : null}

      <Panel>
        <div className={styles.stateRow} data-testid="monitor-status" data-task-id={taskId ?? ''}>
          <Badge tone={running ? 'success' : 'neutral'}>{stateLabel}</Badge>
          {sourceLabel !== null ? (
            <Badge tone={status?.source === 'counters' ? 'warning' : 'info'}>{sourceLabel}</Badge>
          ) : null}
          {status !== null ? (
            <span className={styles.hint}>
              {tPlural('scan.objects', visibleEvents.length)} ·{' '}
              {formatBytes(status.bytes_total, units)}
            </span>
          ) : null}
        </div>

        {status?.warnings.map((key) => (
          <Banner tone="warning" key={key}>
            {t(key)}
          </Banner>
        ))}

        {status?.error !== null && status?.error !== undefined ? (
          <Banner tone="error">{t(status.error.message_key)}</Banner>
        ) : null}

        <div className={styles.toolbar}>
          <SearchField
            className={styles.search}
            label={t('monitor.process')}
            placeholder={t('monitor.process')}
            value={processFilter}
            onValueChange={(value) => {
              useMonitorStore.getState().setProcessFilter(value);
            }}
          />
          <SearchField
            className={styles.search}
            label={t('monitor.paths')}
            placeholder={t('monitor.paths')}
            value={pathFilter}
            onValueChange={(value) => {
              useMonitorStore.getState().setPathFilter(value);
            }}
          />
          <Checkbox
            id="monitor-threshold-only"
            checked={thresholdOnly}
            label={t('monitor.threshold_only')}
            onChange={(event) => {
              useMonitorStore.getState().setThresholdOnly(event.target.checked);
            }}
          />
        </div>

        <DataTable
          label={t('nav.monitor')}
          rows={visibleEvents}
          columns={columns}
          rowKey={(row) => String(row.sequence)}
          sort={sort}
          onSortChange={setSort}
          onRowHoverStart={openHover}
          onRowHoverEnd={closeHover}
          loading={loading}
          height={360}
          empty={<EmptyState title={t('monitor.no_events')} />}
        />

        {hoverRow !== null ? (
          <HoverCard anchor={hoverRow.anchor} label={t('operations.monitor.events')}>
            <HoverCardHead
              title={hoverRow.event.process === '' ? t('monitor.process') : hoverRow.event.process}
              meta={`#${formatNumber(hoverRow.event.process_id, language)}`}
            />
            {pathAvailable ? (
              <HoverCardPath title={hoverRow.event.paths[0] ?? ''}>
                {hoverRow.event.paths[0] ?? '—'}
              </HoverCardPath>
            ) : (
              <HoverCardNote>{t('monitor.path_unavailable')}</HoverCardNote>
            )}
            <HoverCardRows>
              <HoverCardRow
                label={t('monitor.date')}
                value={formatDateTimeShort(hoverRow.event.timestamp)}
              />
              <HoverCardRow
                label={t('monitor.bytes')}
                value={formatBytes(hoverRow.event.bytes, units)}
              />
              <HoverCardRow
                label={t('monitor.pid')}
                value={formatNumber(hoverRow.event.process_id, language)}
              />
              <HoverCardRow
                label={t('monitor.sequence')}
                value={formatNumber(hoverRow.event.sequence, language)}
              />
              <HoverCardRow
                label={t('monitor.threshold_hit')}
                value={
                  hoverRow.event.threshold_exceeded ? t('monitor.threshold_hit') : '—'
                }
              />
            </HoverCardRows>
          </HoverCard>
        ) : null}
      </Panel>
    </Screen>
  );
}
