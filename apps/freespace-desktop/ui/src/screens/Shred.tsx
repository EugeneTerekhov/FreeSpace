import { useEffect, useMemo, useState } from 'react';

import type {
  OperationPreview,
  OperationProgress,
  ShredPattern,
  UiError,
} from '../api/dto';
import { shredCancel, shredPreview, shredStart, shredStatus } from '../api/settingsOps';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Badge, Checkbox, Segmented } from '../components/Choice';
import { Dialog } from '../components/Dialog';
import { Banner, ErrorState } from '../components/Feedback';
import { Field } from '../components/Fields';
import { OperationProgressView } from '../components/OperationProgress';
import { t, tPlural } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { Panel, Screen, Stat, StatRow } from './ScreenFrame';
import styles from './operations.module.css';

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);
const MAX_PASSES = 35;

/** English fallback until `shred.pattern.*` lands in `locales/*.toml`. */
const FALLBACK_LABELS: Record<string, string> = {
  'shred.pattern.zeros': 'Zeros',
  'shred.pattern.random': 'Random',
};

function label(key: string): string {
  const text = t(key);

  return text === key ? (FALLBACK_LABELS[key] ?? text) : text;
}

function splitTargets(value: string): string[] {
  return value
    .split(/\r?\n|;/)
    .map((item) => item.trim())
    .filter((item) => item !== '');
}

function clampPasses(value: string): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return 1;

  return Math.min(MAX_PASSES, Math.max(1, Math.trunc(parsed)));
}

/**
 * Irreversible shredder (T061).
 *
 * The form only builds a preview. The start button exists solely inside the
 * preview dialog, is marked `data-agent-confirm` and stays disabled until the
 * explicit acknowledgement is checked, so no caller can begin from the form.
 */
export function Shred() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [targets, setTargets] = useState('');
  const [passes, setPasses] = useState(3);
  const [pattern, setPattern] = useState<ShredPattern>('zeros');
  const [preview, setPreview] = useState<OperationPreview | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [acknowledged, setAcknowledged] = useState(false);
  const [operationId, setOperationId] = useState<string | null>(null);
  const [progress, setProgress] = useState<OperationProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const paths = useMemo(() => splitTargets(targets), [targets]);

  useEffect(() => {
    if (operationId === null) return undefined;

    let disposed = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const tick = async () => {
      try {
        const current = await shredStatus(operationId);
        if (disposed) return;
        setProgress(current);
        if (TERMINAL.has(current.state)) {
          if (current.state === 'completed') {
            setNotice(t('shred.done', { count: current.processed }));
          }
          return;
        }
        timer = setTimeout(() => void tick(), 300);
      } catch (cause) {
        if (!disposed) setError(normalizeError(cause));
      }
    };

    void tick();

    return () => {
      disposed = true;
      if (timer !== null) clearTimeout(timer);
    };
  }, [operationId]);

  const prepare = async () => {
    setError(null);
    setNotice(null);
    setBusy(true);
    try {
      const next = await shredPreview({ targets: paths, passes, pattern });
      setPreview(next);
      setAcknowledged(false);
      setDialogOpen(true);
    } catch (cause) {
      setError(normalizeError(cause));
      setPreview(null);
    } finally {
      setBusy(false);
    }
  };

  const start = async () => {
    if (preview === null || !acknowledged) return;
    setBusy(true);
    setError(null);
    try {
      const targetCount = preview.targets.length;
      const started = await shredStart(preview.preview_id, preview.confirmation_token);
      setDialogOpen(false);
      setPreview(null);
      setAcknowledged(false);
      setProgress({
        operation_id: started.operation_id,
        state: 'preparing',
        processed: 0,
        total: targetCount,
        current_target: null,
        bytes_per_second: null,
        errors: [],
        undo_available: false,
        journal_entry_id: null,
      });
      setOperationId(started.operation_id);
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setBusy(false);
    }
  };

  const cancel = async () => {
    if (operationId === null) return;
    try {
      await shredCancel(operationId);
      const current = await shredStatus(operationId);
      setProgress(current);
    } catch (cause) {
      setError(normalizeError(cause));
    }
  };

  const warningKeys =
    preview === null
      ? []
      : [
          ...new Set(
            preview.warnings.filter(
              (key) =>
                key !== 'shred.ssd_warning' &&
                key !== 'shred.irreversible' &&
                key !== 'confirm.warning.permanent',
            ),
          ),
        ];

  return (
    <Screen context={t('operations.shred.context', { count: paths.length })}>
      <StatRow>
        <Stat label={t('shred.files')} value={formatNumber(paths.length, language)} />
        <Stat label={t('shred.passes')} value={formatNumber(passes, language)} />
        <Stat label={t('shred.pattern')} value={label(`shred.pattern.${pattern}`)} />
      </StatRow>

      <Panel
        title={t('operations.shred.form')}
        hint={t('shred.intro')}
        actions={
          <span className={styles.hint}>
            {t('operations.shred.preview_required')}
          </span>
        }
      >
        <Banner tone="error">{t('shred.irreversible')}</Banner>
        <Banner tone="warning">{t('shred.ssd_warning')}</Banner>

        <div className={styles.formGrid}>
          <div className={styles.wide}>
            <Field id="shred-targets" label={t('shred.files')}>
              <textarea
                id="shred-targets"
                className={styles.control}
                rows={5}
                value={targets}
                placeholder={t('shred.files_hint')}
                onChange={(event) => {
                  setTargets(event.target.value);
                }}
              />
            </Field>
            <p className={styles.hint}>
              {paths.length === 0
                ? t('shred.no_files')
                : tPlural('scan.objects', paths.length)}
            </p>
          </div>

          <Field id="shred-passes" label={t('shred.passes')} hint={`1 – ${MAX_PASSES}`}>
            <input
              id="shred-passes"
              className={`${styles.control} ${styles.number}`}
              type="number"
              min={1}
              max={MAX_PASSES}
              step={1}
              value={passes}
              onChange={(event) => {
                setPasses(clampPasses(event.target.value));
              }}
            />
          </Field>

          <div>
            <span className={styles.hint}>{t('shred.pattern')}</span>
            <Segmented
              label={t('shred.pattern')}
              value={pattern}
              onChange={(value) => {
                setPattern(value === 'random' ? 'random' : 'zeros');
              }}
              options={[
                { value: 'zeros', label: label('shred.pattern.zeros') },
                { value: 'random', label: label('shred.pattern.random') },
              ]}
            />
          </div>

          <div className={styles.inline}>
            <Button
              variant="primary"
              loading={busy}
              disabled={paths.length === 0}
              onClick={() => void prepare()}
            >
              {t('details.inspect')}
            </Button>
            <Badge tone="info">
              {t('shred.passes')}: {formatNumber(passes, language)}
            </Badge>
          </div>
        </div>
      </Panel>

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button
              variant="primary"
              loading={busy}
              disabled={paths.length === 0}
              onClick={() => void prepare()}
            >
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {notice !== null ? <Banner tone="info">{notice}</Banner> : null}

      {progress !== null ? (
        <OperationProgressView
          progress={progress}
          onCancel={() => void cancel()}
          onUndo={() => undefined}
        />
      ) : null}

      <Dialog
        open={dialogOpen && preview !== null}
        title={t('confirm.title')}
        busy={busy}
        onClose={() => {
          if (busy) return;
          setDialogOpen(false);
          setAcknowledged(false);
        }}
        footer={
          <>
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => {
                setDialogOpen(false);
                setAcknowledged(false);
              }}
            >
              {t('common.cancel')}
            </Button>
            <Button
              variant="danger"
              loading={busy}
              disabled={!acknowledged}
              data-agent-confirm="true"
              onClick={() => void start()}
            >
              {t('shred.destroy')}
            </Button>
          </>
        }
      >
        {preview === null ? null : (
          <div className={styles.stack}>
            <p className={styles.note}>
              {t('confirm.summary', {
                count: formatNumber(preview.targets.length, language),
                size: formatBytes(preview.total_size, units),
              })}
            </p>

            <Banner tone="error">{t('shred.irreversible')}</Banner>
            <p className={styles.hint}>{t('operations.shred.irreversible_hint')}</p>
            <Banner tone="warning">{t('shred.ssd_warning')}</Banner>

            {warningKeys.length > 0 ? (
              <ul className={styles.list}>
                {warningKeys.map((key) => (
                  <li key={key}>{t(key)}</li>
                ))}
              </ul>
            ) : null}

            <dl className={styles.details}>
              <dt>{t('shred.passes')}</dt>
              <dd>{formatNumber(passes, language)}</dd>
              <dt>{t('shred.pattern')}</dt>
              <dd>{label(`shred.pattern.${pattern}`)}</dd>
            </dl>

            <table className={styles.table} aria-label={t('confirm.targets')}>
              <thead>
                <tr>
                  <th scope="col">{t('confirm.target')}</th>
                  <th scope="col">{t('details.kind')}</th>
                  <th scope="col" className={styles.numeric}>
                    {t('details.size')}
                  </th>
                </tr>
              </thead>
              <tbody>
                {preview.targets.map((target) => (
                  <tr key={target.path}>
                    <td className={styles.mono} title={target.path}>
                      <span className={styles.pathCell}>{target.path}</span>
                    </td>
                    <td>{t(`kind.${target.kind}`)}</td>
                    <td className={styles.numeric}>{formatBytes(target.size, units)}</td>
                  </tr>
                ))}
              </tbody>
            </table>

            <Checkbox
              id="shred-acknowledge"
              checked={acknowledged}
              disabled={busy}
              label={t('shred.confirm')}
              onChange={(event) => {
                setAcknowledged(event.target.checked);
              }}
            />
          </div>
        )}
      </Dialog>
    </Screen>
  );
}
