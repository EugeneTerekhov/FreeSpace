import { useCallback, useEffect, useState } from 'react';

import { api } from '../api/runtime';
import type {
  ScanMethodChoice,
  ScanStarted,
  Task,
  UiError,
  VolumeSummary,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { Badge } from '../components/Choice';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Banner, EmptyState, ErrorState, Loading, Meter, Progress } from '../components/Feedback';
import { SearchField, Select } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes, formatNumber, formatPercent } from '../i18n/format';
import { scanMethods } from '../lib/capabilities';
import { useSettingsStore } from '../stores/settingsStore';
import { usePlatformCapabilities, useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { useTasksStore } from '../stores/tasksStore';
import { Panel, Screen, Stat, StatRow } from './ScreenFrame';
import styles from './overview.module.css';

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);
/** A scan that still owns the volume; finished scans must not block a new run. */
const ACTIVE = new Set(['queued', 'running', 'cancelling']);

/**
 * Overview: volumes, headline numbers and the embedded scan progress
 * (plan.md, «Решение 2»).
 *
 * The scan lives here instead of a dedicated screen: the progress, current path,
 * speed, access errors and cancellation are visible while the user browses the
 * volumes, and the status bar repeats the summary everywhere else.
 */
export function Overview() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const tasks = useTasksStore((state) => state.tasks);
  const upsert = useTasksStore((state) => state.upsert);
  const [volumes, setVolumes] = useState<VolumeSummary[] | null>(null);
  const [error, setError] = useState<UiError | null>(null);
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState('');
  const [started, setStarted] = useState<ScanStarted | null>(null);
  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  // The scan type applies to the analysis started from the selection bar; the
  // volume blocks above provide the paths.
  const [method, setMethod] = useState<ScanMethodChoice>('auto');
  const capabilities = usePlatformCapabilities();
  const methods = scanMethods(capabilities);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const loaded = await api.volumesRefresh();
      setVolumes(loaded);

      if (useSnapshotStore.getState().volume === null) {
        const first = loaded.find((volume) => volume.status === 'available') ?? loaded[0];
        if (first !== undefined) useSnapshotStore.getState().setVolume(first);
      }
    } catch (cause) {
      setError(normalizeError(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const scan: Task | undefined =
    started === null
      ? tasks.filter((item) => item.kind === 'scan').at(-1)
      : tasks.find((item) => item.task_id === started.task_id);
  const scanId = scan?.task_id ?? started?.task_id;

  useEffect(() => {
    if (scanId === undefined) return undefined;

    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      try {
        const current = await api.scanStatus(scanId);
        if (!active) return;
        upsert(current);
        if (!TERMINAL.has(current.state)) {
          timer = setTimeout(() => void tick(), 500);
        }
      } catch (cause) {
        if (active) setError(normalizeError(cause));
      }
    };

    void tick();

    return () => {
      active = false;
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [scanId, upsert]);

  const analyzeVolumes = async (targets: VolumeSummary[]) => {
    if (targets.length === 0) return;
    const running = tasks.find(
      (task) =>
        task.kind === 'scan' &&
        (task.state === 'running' || task.state === 'queued' || task.state === 'cancelling'),
    );
    // A second click must not surface `E_BUSY`; the embedded progress already
    // shows the running analysis.
    if (running !== undefined) return;

    const first = targets[0];
    if (first === undefined) return;

    // Only the selected volumes are scanned; the backend merges their roots
    // into the current snapshot so volumes scanned earlier stay visible.
    setBusy(true);
    setError(null);
    try {
      const response = await api.scanStart({
        paths: targets.map((volume) => volume.mount_point),
        method,
        incremental: false,
        exclusions: [],
        include_ads: false,
        metric: 'on_disk',
        merge: true,
      });
      setStarted(response);
      useTasksStore.getState().upsert({
        task_id: response.task_id,
        kind: 'scan',
        state: 'running',
        volume_id: first.volume_id,
        volume_ids: targets.map((volume) => volume.volume_id),
        progress: null,
        started_at: Date.now(),
        finished_at: null,
        cancelable: true,
        error: null,
      });
    } catch (cause) {
      const normalized = normalizeError(cause);
      if (normalized.code !== 'E_BUSY') setError(normalized);
    } finally {
      setBusy(false);
    }
  };

  const toggleSelected = (volumeId: string) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(volumeId)) {
        next.delete(volumeId);
      } else {
        next.add(volumeId);
      }

      return next;
    });
  };

  if (error !== null && volumes === null) {
    return (
      <ErrorState
        title={t(error.message_key)}
        code={error.code}
        details={error.details ?? undefined}
        actions={
          <Button variant="primary" onClick={() => void refresh()}>
            {t('error.retry')}
          </Button>
        }
      />
    );
  }

  if (volumes === null) {
    return <Loading label={t('common.loading')} rows={4} />;
  }

  const needle = query.trim().toLowerCase();
  const visible = volumes.filter(
    (volume) =>
      needle === '' ||
      volume.mount_point.toLowerCase().includes(needle) ||
      volume.label.toLowerCase().includes(needle),
  );

  const total = volumes.reduce((sum, volume) => sum + volume.total_bytes, 0);
  const used = volumes.reduce((sum, volume) => sum + volume.used_bytes, 0);
  const free = volumes.reduce((sum, volume) => sum + volume.free_bytes, 0);
  const percent = scan === undefined ? undefined : scanPercent(scan);
  const selectedVolumes = visible.filter((volume) => selected.has(volume.volume_id));
  const scanningVolumeIds = new Set(
    scan === undefined
      ? []
      : (scan.volume_ids ?? (scan.volume_id === null ? [] : [scan.volume_id])),
  );

  return (
    <Screen
      actions={
        <>
          <SearchField
            className={styles.search}
            label={t('overview.search')}
            placeholder={t('overview.search_placeholder')}
            value={query}
            onValueChange={setQuery}
          />
          <Button variant="secondary" onClick={() => void refresh()}>
            {t('dashboard.refresh')}
          </Button>
        </>
      }
    >
      {error !== null ? (
        <Banner
          tone="error"
          action={
            <Button variant="ghost" onClick={() => void refresh()}>
              {t('error.retry')}
            </Button>
          }
        >
          {t(error.message_key)}
        </Banner>
      ) : null}

      <StatRow>
        <Stat label={t('dashboard.total')} value={formatBytes(total, units)} />
        <Stat label={t('dashboard.used')} value={formatBytes(used, units)} />
        <Stat label={t('dashboard.free')} value={formatBytes(free, units)} />
        <Stat
          label={t('dashboard.volumes')}
          value={`${volumes.length} · ${formatPercent(total === 0 ? 0 : used / total, language)}`}
        />
      </StatRow>

      {selectedVolumes.length === 0 ? null : (
        <Panel
          title={t('overview.selected.title', {
            list: selectedVolumes.map((volume) => volumeLetter(volume.mount_point)).join(', '),
          })}
          actions={
            <>
              <Select
                className={styles.methodSelect}
                aria-label={t('scan.method')}
                value={method}
                onChange={(event) => {
                  setMethod(event.target.value as ScanMethodChoice);
                }}
                options={methods.map((value) => ({
                  value,
                  label: t(
                    value === 'auto'
                      ? 'scan.method_auto'
                      : value === 'mft'
                        ? 'scan.method_mft'
                        : 'scan.method_walk',
                  ),
                }))}
              />
              <Button variant="ghost" onClick={() => setSelected(new Set())}>
                {t('overview.selected.clear')}
              </Button>
              <Button
                variant="primary"
                loading={busy}
                onClick={() => void analyzeVolumes(selectedVolumes)}
              >
                {t('overview.selected.start')}
              </Button>
            </>
          }
        >
          {null}
        </Panel>
      )}

      {visible.length === 0 ? (
        <EmptyState
          title={t('dashboard.no_volumes')}
          description={query === '' ? t('dashboard.no_volumes_hint') : t('overview.no_matches')}
          actions={
            <Button
              variant="primary"
              onClick={() => {
                setQuery('');
                void refresh();
              }}
            >
              {t('dashboard.refresh')}
            </Button>
          }
        />
      ) : (
        <section className={styles.volumeGrid} aria-label={t('dashboard.volumes')}>
          {visible.map((volume) => {
            const ratio =
              volume.total_bytes === 0 ? 0 : volume.used_bytes / volume.total_bytes;
            const isSelected = selected.has(volume.volume_id);
            const scanning =
              scan !== undefined &&
              ACTIVE.has(scan.state) &&
              scanningVolumeIds.has(volume.volume_id);
            const toggle = () => toggleSelected(volume.volume_id);

            return (
              <Card
                key={volume.volume_id}
                className={styles.volumeCard}
                title={
                  <span className={styles.volumeHead}>
                    <span className={styles.volumeLetter}>
                      {volumeLetter(volume.mount_point)}
                    </span>
                    <span className={styles.volumeLabel}>{volume.label}</span>
                  </span>
                }
                actions={
                  <span className={styles.volumeActions}>
                    {scanning ? (
                      <Badge tone="info">{t('overview.scan.running')}</Badge>
                    ) : null}
                    <Badge tone={volume.status === 'available' ? 'success' : 'warning'}>
                      {t(`volume.status.${volume.status}`)}
                    </Badge>
                  </span>
                }
                interactive
                selected={isSelected}
                role="button"
                tabIndex={0}
                aria-pressed={isSelected}
                aria-busy={scanning || undefined}
                onClick={toggle}
                onKeyDown={(event) => {
                  if (event.key !== 'Enter' && event.key !== ' ') return;
                  event.preventDefault();
                  toggle();
                }}
              >
                <div className={styles.volumeBody}>
                  <Meter
                    label={t('dashboard.usage')}
                    value={Math.round(ratio * 100)}
                    caption={formatPercent(ratio, language)}
                  />
                  <div className={styles.metaRow}>
                    <span>
                      {t('dashboard.free')}: <strong>{formatBytes(volume.free_bytes, units)}</strong>
                    </span>
                    <span>
                      {t('dashboard.total')}: <strong>{formatBytes(volume.total_bytes, units)}</strong>
                    </span>
                    <span className={styles.fs}>{volume.file_system}</span>
                  </div>
                </div>
              </Card>
            );
          })}
        </section>
      )}

      {scan !== undefined && scan.state !== 'cancelled' ? (
        <Panel
          title={t('overview.scan.title')}
          actions={
            scan.state === 'running' || scan.state === 'cancelling' ? (
              <Button
                variant="danger"
                disabled={!scan.cancelable}
                onClick={() => void api.scanCancel(scan.task_id)}
              >
                {t('scan.cancel')}
              </Button>
            ) : (
              <Button
                variant="secondary"
                onClick={() => useShellStore.getState().setScreen('tree')}
              >
                {t('scan.view_tree')}
              </Button>
            )
          }
          hint={
            scan.state === 'partial'
              ? t('overview.scan.partial')
              : scan.state === 'completed'
                ? t('scan.completed')
                : undefined
          }
        >
          <Progress
            label={t(scan.progress?.phase_key ?? 'scan.phase.preparing')}
            state={progressState(scan, percent)}
            {...(percent === undefined ? {} : { value: percent })}
          />
          <div className={styles.scanMeta}>
            <span>
              {t(scan.progress?.phase_key ?? 'scan.phase.preparing')} ·{' '}
              {formatNumber(scan.progress?.processed ?? 0, language)}
              {scan.progress?.total !== null && scan.progress?.total !== undefined
                ? ` / ${formatNumber(scan.progress.total, language)}`
                : ''}{' '}
              {t('scan.nodes')}
            </span>
            {scan.progress?.bytes_per_second === null ||
            scan.progress?.bytes_per_second === undefined ? null : (
              <span>{`${formatBytes(scan.progress.bytes_per_second, units)}/s`}</span>
            )}
          </div>
          {scan.progress?.current_path !== null &&
          scan.progress?.current_path !== undefined ? (
            <p className={styles.scanPath}>{scan.progress.current_path}</p>
          ) : null}
          {scan.state === 'failed' ? (
            <Banner tone="error">
              {t('scan.failed')}
              {scan.error === null ? '' : ` · ${scan.error.code}`}
            </Banner>
          ) : null}
        </Panel>
      ) : null}
    </Screen>
  );
}

/** Drive letter without the colon for a volume root; other mounts keep their path. */
function volumeLetter(mountPoint: string): string {
  const drive = /^([A-Za-z]):[\\/]*$/.exec(mountPoint);
  if (drive?.[1] !== undefined) return drive[1].toUpperCase();
  return mountPoint.replace(/[\\/]+$/, '');
}

function scanPercent(task: Task): number | undefined {
  if (task.state === 'completed' || task.state === 'partial') return 100;
  const progress = task.progress;
  if (progress === null) return undefined;
  const total = progress.total;
  if (total === null || total === undefined || total === 0) return undefined;
  return (progress.processed / total) * 100;
}

function progressState(task: Task, percent: number | undefined): 'normal' | 'error' | 'cancelled' | 'indeterminate' {
  if (task.state === 'failed') return 'error';
  if (task.state === 'cancelled') return 'cancelled';
  return percent === undefined ? 'indeterminate' : 'normal';
}
