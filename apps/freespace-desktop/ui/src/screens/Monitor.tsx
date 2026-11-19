import { useEffect, useMemo, useRef, useState } from 'react';

import type {
  MonitorEvent,
  MonitorEventFilter,
  MonitorStatus,
  MonitorTickEvent,
  UiError,
} from '../api/dto';
import type { Unlisten } from '../api/transport';
import {
  monitorEventsPage,
  monitorSetExclusions,
  monitorStart,
  monitorStatus,
  monitorStop,
} from '../api/settingsOps';
import { api } from '../api/runtime';
import { normalizeError } from '../app/errors';
import { useLiveAnnouncer } from '../app/live';
import { Button } from '../components/Button';
import { Badge, Checkbox } from '../components/Choice';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, LiveRegions } from '../components/Feedback';
import { Field, SearchField } from '../components/Fields';
import { t, tPlural } from '../i18n';
import { formatBytes, formatDate, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { Panel, Screen } from './ScreenFrame';
import styles from './operations.module.css';

const MAX_INTERVAL = 86_400;

function splitList(value: string): string[] {
  return value
    .split(/\r?\n|;/)
    .map((item) => item.trim())
    .filter((item) => item !== '');
}

function mergeEvents(current: MonitorEvent[], incoming: MonitorEvent[]): MonitorEvent[] {
  const bySequence = new Map(current.map((event) => [event.sequence, event]));
  for (const event of incoming) {
    bySequence.set(event.sequence, event);
  }

  return [...bySequence.values()].sort((left, right) => right.sequence - left.sequence);
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
 * Events arrive through `monitor-tick`, while the cursor page is polled as a
 * fallback so the table also works with a transport that only answers commands
 * (the browser mock and the contract fixtures). New events are announced into a
 * polite live region at the throttled `progress` rate, so a busy disk never
 * floods the narrator.
 */
export function Monitor() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [threshold, setThreshold] = useState(String(1024 * 1024 * 1024));
  const [interval, setIntervalValue] = useState('2');
  const [exclusions, setExclusions] = useState('');
  const [taskId, setTaskId] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState<MonitorStatus | null>(null);
  const [events, setEvents] = useState<MonitorEvent[]>([]);
  const [processFilter, setProcessFilter] = useState('');
  const [pathFilter, setPathFilter] = useState('');
  const [thresholdOnly, setThresholdOnly] = useState(false);
  const [loading, setLoading] = useState(false);
  const [validation, setValidation] = useState<string | null>(null);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const { polite, assertive, announce } = useLiveAnnouncer();
  const seen = useRef(new Set<number>());

  const filter = useMemo<MonitorEventFilter>(
    () => ({
      process: processFilter.trim() === '' ? null : processFilter,
      path: pathFilter.trim() === '' ? null : pathFilter,
      ...(thresholdOnly ? { threshold_only: true } : {}),
    }),
    [pathFilter, processFilter, thresholdOnly],
  );

  const visibleEvents = useMemo(
    () =>
      events
        .filter((event) => matchesEvent(event, processFilter, pathFilter))
        .filter((event) => !thresholdOnly || event.threshold_exceeded),
    [events, pathFilter, processFilter, thresholdOnly],
  );

  // Announce only sequences that have not been seen yet, at the throttled
  // `progress` rate owned by `LiveAnnouncer`.
  const announceEvents = (incoming: MonitorEvent[]) => {
    const fresh = incoming.filter((event) => !seen.current.has(event.sequence));
    if (fresh.length === 0) return;
    for (const event of fresh) seen.current.add(event.sequence);
    announce(t('operations.monitor.new_events', { count: fresh.length }), {
      priority: 'progress',
    });
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
        setStatus(nextStatus);
        setEvents(page.events);
        announceEvents(page.events);
        if (nextStatus.running) {
          const seconds = Math.max(1, nextStatus.interval_secs);
          timer = setTimeout(() => void poll(), seconds * 1000);
        } else {
          setRunning(false);
        }
      } catch (cause) {
        if (disposed) return;
        setError(normalizeError(cause));
        setRunning(false);
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
        setEvents((current) => mergeEvents(current, payload.events));
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
      intervalSeconds > MAX_INTERVAL
    ) {
      setValidation(t('error.validation'));
      return;
    }

    setValidation(null);
    setError(null);
    setNotice(null);
    setLoading(true);
    try {
      const started = await monitorStart({
        threshold: thresholdBytes,
        interval: intervalSeconds,
        exclusions: splitList(exclusions),
      });
      seen.current.clear();
      setTaskId(started.task_id);
      setStatus(null);
      setEvents([]);
      setRunning(true);
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  };

  const stop = async () => {
    if (taskId === null) return;
    setLoading(true);
    setError(null);
    try {
      await monitorStop(taskId);
      setStatus(await monitorStatus(taskId));
      setRunning(false);
      setNotice(t('monitor.summary'));
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  };

  const applyExclusions = async () => {
    if (taskId === null || !running) return;
    try {
      await monitorSetExclusions(splitList(exclusions));
    } catch (cause) {
      setError(normalizeError(cause));
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

  const columns: Column<MonitorEvent>[] = [
    {
      id: 'window',
      header: t('monitor.window'),
      width: '170px',
      render: (row) => formatDate(row.timestamp, language),
    },
    {
      id: 'owner',
      header: t('operations.monitor.owner'),
      width: 'minmax(140px, 1fr)',
      render: (row) => (
        <span className={styles.owner}>
          <span>{row.process}</span>
          <span className={styles.hint}>#{formatNumber(row.process_id, language)}</span>
        </span>
      ),
    },
    {
      id: 'paths',
      header: t('monitor.paths'),
      width: 'minmax(220px, 2fr)',
      render: (row) => (
        <span className={styles.pathCell} title={row.paths.join('; ')}>
          {row.paths.join('; ')}
        </span>
      ),
    },
    {
      id: 'bytes',
      header: t('monitor.bytes'),
      width: '120px',
      align: 'end',
      render: (row) => formatBytes(row.bytes, units),
    },
    {
      id: 'threshold',
      header: t('monitor.threshold_hit'),
      width: '120px',
      render: (row) =>
        row.threshold_exceeded ? (
          <Badge tone="warning">{t('monitor.threshold_hit')}</Badge>
        ) : (
          <span aria-hidden="true">—</span>
        ),
    },
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
                  setThreshold(event.target.value);
                }}
              />
            </Field>

            <Field
              id="monitor-interval"
              label={`${t('monitor.interval')} (1 – ${MAX_INTERVAL})`}
            >
              <input
                id="monitor-interval"
                className={`${styles.control} ${styles.number}`}
                type="number"
                min={1}
                max={MAX_INTERVAL}
                step={1}
                value={interval}
                onChange={(event) => {
                  setIntervalValue(event.target.value);
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
                  setExclusions(event.target.value);
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
            onValueChange={setProcessFilter}
          />
          <SearchField
            className={styles.search}
            label={t('monitor.paths')}
            placeholder={t('monitor.paths')}
            value={pathFilter}
            onValueChange={setPathFilter}
          />
          <Checkbox
            id="monitor-threshold-only"
            checked={thresholdOnly}
            label={t('monitor.threshold_only')}
            onChange={(event) => {
              setThresholdOnly(event.target.checked);
            }}
          />
        </div>

        <DataTable
          label={t('nav.monitor')}
          rows={visibleEvents}
          columns={columns}
          rowKey={(row) => String(row.sequence)}
          loading={loading}
          height={360}
          empty={<EmptyState title={t('monitor.no_events')} />}
        />
      </Panel>
    </Screen>
  );
}
