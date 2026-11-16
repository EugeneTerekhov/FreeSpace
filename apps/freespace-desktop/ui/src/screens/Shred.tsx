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
import { Card } from '../components/Card';
import { Badge, Checkbox, Segmented } from '../components/Choice';
import { Dialog } from '../components/Dialog';
import { Banner, ErrorState } from '../components/Feedback';
import { Field } from '../components/Fields';
import { OperationProgressView } from '../components/OperationProgress';
import { t, tPlural } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import local from './settings.module.css';

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
 * Irreversible shredder.
 *
 * The start button exists only inside the preview dialog and stays disabled
 * until the explicit acknowledgement is checked: the operation can never begin
 * from the form itself.
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
    <div className={local.page}>
      <Card title={t('nav.shred')}>
        <p className={local.note}>{t('shred.intro')}</p>

        <Banner tone="error">{t('shred.irreversible')}</Banner>
        <Banner tone="warning">{t('shred.ssd_warning')}</Banner>

        <div className={local.formGrid}>
          <div className={local.wide}>
            <Field id="shred-targets" label={t('shred.files')}>
              <textarea
                id="shred-targets"
                className={local.control}
                rows={5}
                value={targets}
                placeholder={t('shred.files_hint')}
                onChange={(event) => {
                  setTargets(event.target.value);
                }}
              />
            </Field>
            {paths.length === 0 ? (
              <p className={local.hint}>{t('shred.no_files')}</p>
            ) : (
              <p className={local.hint}>
                {tPlural('scan.objects', paths.length)}
              </p>
            )}
          </div>

          <Field id="shred-passes" label={t('shred.passes')} hint={`1 – ${MAX_PASSES}`}>
            <input
              id="shred-passes"
              className={`${local.control} ${local.number}`}
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
            <span className={local.hint}>{t('shred.pattern')}</span>
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

          <div className={local.inline}>
            <Button
              variant="primary"
              loading={busy}
              disabled={paths.length === 0}
              onClick={() => void prepare()}
            >
              {t('details.inspect')}
            </Button>
            <Badge tone="info">{t('shred.passes')}: {formatNumber(passes, language)}</Badge>
          </div>
        </div>
      </Card>

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
              onClick={() => void start()}
            >
              {t('shred.destroy')}
            </Button>
          </>
        }
      >
        {preview === null ? null : (
          <div className={local.page}>
            <p className={local.note}>
              {t('confirm.summary', {
                count: formatNumber(preview.targets.length, language),
                size: formatBytes(preview.total_size, units),
              })}
            </p>

            <Banner tone="error">{t('shred.irreversible')}</Banner>
            <Banner tone="warning">{t('shred.ssd_warning')}</Banner>

            {warningKeys.length > 0 ? (
              <ul className={local.list}>
                {warningKeys.map((key) => (
                  <li key={key}>{t(key)}</li>
                ))}
              </ul>
            ) : null}

            <dl className={local.meta}>
              <dt>{t('shred.passes')}</dt>
              <dd>{formatNumber(passes, language)}</dd>
              <dt>{t('shred.pattern')}</dt>
              <dd>{label(`shred.pattern.${pattern}`)}</dd>
            </dl>

            <table className={local.table} aria-label={t('confirm.targets')}>
              <thead>
                <tr>
                  <th scope="col">{t('confirm.target')}</th>
                  <th scope="col">{t('details.kind')}</th>
                  <th scope="col" className={local.numeric}>
                    {t('details.size')}
                  </th>
                </tr>
              </thead>
              <tbody>
                {preview.targets.map((target) => (
                  <tr key={target.path}>
                    <td className={`${local.mono} ${local.pathCell}`} title={target.path}>
                      {target.path}
                    </td>
                    <td>{t(`kind.${target.kind}`)}</td>
                    <td className={local.numeric}>{formatBytes(target.size, units)}</td>
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
    </div>
  );
}
