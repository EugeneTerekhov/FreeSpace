import { useEffect, useState } from 'react';

import { api } from '../api/runtime';
import type { ScanMethodChoice, ScanStarted, Task, UiError } from '../api/dto';
import { Banner, ErrorState, Loading, Progress } from '../components/Feedback';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Field, Select } from '../components/Fields';
import { normalizeError, validationError } from '../app/errors';
import { t } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { useTasksStore } from '../stores/tasksStore';
import styles from './screens.module.css';

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);

/** Scan form, live progress and cancellation. */
export function Scan() {
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const upsert = useTasksStore((state) => state.upsert);
  const tasks = useTasksStore((state) => state.tasks);
  const [path, setPath] = useState('C:\\');
  const [method, setMethod] = useState<ScanMethodChoice>('auto');
  const [exclusions, setExclusions] = useState('');
  const [started, setStarted] = useState<ScanStarted | null>(null);
  const [error, setError] = useState<UiError | null>(null);
  const [busy, setBusy] = useState(false);

  const task: Task | undefined =
    started === null
      ? tasks.filter((item) => item.kind === 'scan').at(-1)
      : tasks.find((item) => item.task_id === started.task_id);

  useEffect(() => {
    if (started === null) return undefined;

    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      try {
        const current = await api.scanStatus(started.task_id);
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
  }, [started, upsert]);

  useEffect(() => {
    if (task?.state !== 'completed' || started === null) return;

    useSnapshotStore.getState().setGeneration(started.generation);
    void api
      .volumesCurrent()
      .then((volume) => {
        useSnapshotStore.getState().setVolume(volume);
      })
      .catch(() => undefined);
  }, [task?.state, started]);

  const start = async () => {
    if (path.trim() === '') {
      setError(validationError(t('scan.empty_path')));
      return;
    }

    setBusy(true);
    setError(null);
    try {
      const response = await api.scanStart({
        path,
        method,
        incremental: false,
        exclusions: exclusions
          .split(/[;|]/)
          .map((item) => item.trim())
          .filter((item) => item !== ''),
        include_ads: false,
        metric: 'on_disk',
      });
      setStarted(response);
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setBusy(false);
    }
  };

  const percent =
    task?.progress?.total === null || task?.progress?.total === undefined
      ? 0
      : (task.progress.processed / task.progress.total) * 100;

  return (
    <div className={styles.stack}>
      <Card title={t('nav.scan')}>
        <form
          className={styles.form}
          onSubmit={(event) => {
            event.preventDefault();
            void start();
          }}
        >
          <Field id="scan-path" label={t('scan.path')} required>
            <input
              id="scan-path"
              className={styles.search}
              value={path}
              onChange={(event) => {
                setPath(event.target.value);
              }}
            />
          </Field>
          <Field id="scan-method" label={t('scan.method')}>
            <Select
              id="scan-method"
              value={method}
              onChange={(event) => {
                setMethod(event.target.value as ScanMethodChoice);
              }}
              options={[
                { value: 'auto', label: t('scan.method_auto') },
                { value: 'mft', label: t('scan.method_mft') },
                { value: 'walk', label: t('scan.method_walk') },
              ]}
            />
          </Field>
          <div className={styles.formWide}>
            <Field id="scan-exclusions" label={t('scan.exclusions')}>
              <input
                id="scan-exclusions"
                value={exclusions}
                onChange={(event) => {
                  setExclusions(event.target.value);
                }}
              />
            </Field>
          </div>
          <div className={styles.analyzeRow}>
            <Button type="submit" variant="primary" loading={busy}>
              {t('scan.start')}
            </Button>
          </div>
        </form>
      </Card>

      {error !== null ? (
        <Banner tone="error">{t(error.message_key)}</Banner>
      ) : null}

      {task === undefined ? (
        started === null ? (
          <p className={styles.currentPath}>{t('scan.required')}</p>
        ) : (
          <Loading label={t('common.loading')} rows={2} />
        )
      ) : (
        <Card
          title={t('nav.scan')}
          actions={
            task.state === 'running' || task.state === 'cancelling' ? (
              <Button
                variant="danger"
                disabled={!task.cancelable}
                onClick={() => {
                  void api.scanCancel(task.task_id);
                }}
              >
                {t('scan.cancel')}
              </Button>
            ) : null
          }
        >
          <div className={styles.progressBlock}>
            <Progress
              label={t(task.progress?.phase_key ?? 'scan.phase.preparing')}
              value={percent}
              state={
                task.state === 'failed'
                  ? 'error'
                  : task.state === 'cancelled'
                    ? 'cancelled'
                    : percent > 0
                      ? 'normal'
                      : 'indeterminate'
              }
            />
            <div className={styles.progressMeta}>
              <span>{t(task.progress?.phase_key ?? 'scan.phase.preparing')}</span>
              <span>
                {formatNumber(task.progress?.processed ?? 0, language)}
                {task.progress?.total !== null && task.progress?.total !== undefined
                  ? ` / ${formatNumber(task.progress.total, language)}`
                  : ''}{' '}
                {t('scan.nodes')}
              </span>
              <span>
                {task.progress?.bytes_per_second === null ||
                task.progress?.bytes_per_second === undefined
                  ? ''
                  : `${formatBytes(task.progress.bytes_per_second, units)}/s`}
              </span>
            </div>
            {task.progress?.current_path !== null &&
            task.progress?.current_path !== undefined ? (
              <p className={styles.currentPath}>{task.progress.current_path}</p>
            ) : null}
            {task.state === 'completed' ? (
              <Banner
                tone="info"
                action={
                  <Button
                    variant="primary"
                    onClick={() => {
                      useShellStore.getState().setScreen('tree');
                    }}
                  >
                    {t('scan.view_tree')}
                  </Button>
                }
              >
                {t('scan.completed')} · {formatNumber(task.progress?.processed ?? 0, language)}{' '}
                {t('scan.nodes')}
              </Banner>
            ) : null}
            {task.state === 'cancelled' ? (
              <Banner tone="warning">{t('status.cancelled')}</Banner>
            ) : null}
            {task.state === 'failed' ? (
              <ErrorState
                title={t('scan.failed')}
                code={task.error?.code}
                details={task.error?.details ?? undefined}
              />
            ) : null}
          </div>
        </Card>
      )}
    </div>
  );
}
