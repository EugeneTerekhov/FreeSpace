import { useEffect, useState } from 'react';

import type {
  ExportFormat,
  ExportPreview,
  ExportSection,
  ExportStatus,
  ReportFormat,
} from '../api/dto-inspection';
import type { UiError } from '../api/dto';
import {
  EXPORT_FORMATS,
  cancelExport,
  fetchExportStatus,
  previewExport,
  startExport,
} from '../api/inspection';
import { normalizeError, validationError } from '../app/errors';
import { Badge, Checkbox } from '../components/Choice';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Dialog } from '../components/Dialog';
import { Banner, EmptyState, ErrorState, Progress } from '../components/Feedback';
import { Field, Select } from '../components/Fields';
import { t, tPlural } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './inspection.module.css';

const TERMINAL = new Set(['completed', 'partial', 'failed', 'cancelled']);

const SECTIONS: readonly ExportSection[] = [
  'tree',
  'top',
  'types',
  'duplicates',
  'owners',
];

const ALL_SECTIONS: readonly ExportSection[] = [...SECTIONS];

const FORMAT_LABELS: Record<ExportFormat, string> = {
  csv: 'CSV',
  xml: 'XML',
  html: 'HTML',
  xlsx: 'Excel',
  json: 'JSON',
};

function joinPath(root: string | null, file: string): string {
  const base = root ?? 'C:\\';

  return `${base}${base.endsWith('\\') ? '' : '\\'}${file}`;
}

function stateLabel(state: string): string {
  if (state === 'queued') return t('status.ready');

  return t(`operation.state.${state}`);
}

function stateTone(
  state: string,
): 'neutral' | 'success' | 'warning' | 'danger' | 'info' {
  if (state === 'completed') return 'success';
  if (state === 'failed') return 'danger';
  if (state === 'partial' || state === 'cancelling') return 'warning';
  if (state === 'cancelled') return 'neutral';

  return 'info';
}

interface ExportFormProps {
  generation: number;
  defaultOutput: string;
  initialSections: readonly ExportSection[];
}

/** Composition, preview and the polling export lifecycle. */
function ExportForm({
  generation,
  defaultOutput,
  initialSections,
}: ExportFormProps) {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [format, setFormat] = useState<ReportFormat>('csv');
  const [output, setOutput] = useState(defaultOutput);
  const [sections, setSections] = useState<ExportSection[]>([...initialSections]);
  const [preview, setPreview] = useState<ExportPreview | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [task, setTask] = useState<ExportStatus | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<UiError | null>(null);

  const running = task !== null && !TERMINAL.has(task.state);

  useEffect(() => {
    if (taskId === null) return undefined;

    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      try {
        const status = await fetchExportStatus(taskId);
        if (!active) return;
        setTask(status);
        if (!TERMINAL.has(status.state)) {
          timer = setTimeout(() => void tick(), 300);
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
  }, [taskId]);

  const invalidate = () => {
    setPreview(null);
  };

  const toggle = (section: ExportSection, checked: boolean) => {
    setSections((current) =>
      checked
        ? [...current.filter((item) => item !== section), section]
        : current.filter((item) => item !== section),
    );
    invalidate();
  };

  const prepare = async () => {
    const path = output.trim();
    setError(null);
    if (path === '') {
      setError(validationError(t('export.output')));
      return;
    }
    if (sections.length === 0) {
      setError(validationError(t('export.include')));
      return;
    }

    setPreparing(true);
    setTask(null);
    setTaskId(null);
    try {
      const next = await previewExport({
        scope: {
          generation,
          out: path,
        },
        format,
        composition: {
          sections,
          max_tree_rows: null,
          overwrite: false,
        },
      });
      setPreview(next);
    } catch (cause) {
      setError(normalizeError(cause));
      setPreview(null);
    } finally {
      setPreparing(false);
    }
  };

  const start = async () => {
    if (preview === null) return;
    setStarting(true);
    setError(null);
    try {
      const started = await startExport(
        preview.preview_id,
        preview.confirmation_token,
      );
      setTaskId(started.task_id);
      setPreview(null);
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setStarting(false);
    }
  };

  const cancel = async () => {
    if (taskId === null) return;
    try {
      await cancelExport(taskId);
    } catch (cause) {
      setError(normalizeError(cause));
    }
  };

  return (
    <div className={styles.stack}>
      <Card title={t('nav.export')}>
        <p>{t('export.intro')}</p>
        <div className={local.filterGrid}>
          <Field id="export-format" label={t('export.format')}>
            <Select
              id="export-format"
              value={format}
              options={EXPORT_FORMATS.map((value) => ({
                value,
                label: FORMAT_LABELS[value],
              }))}
              onChange={(event) => {
                setFormat(event.target.value as ReportFormat);
                invalidate();
              }}
            />
          </Field>
          <Field id="export-output" label={t('export.output')}>
            <input
              id="export-output"
              className={local.input}
              value={output}
              onChange={(event) => {
                setOutput(event.target.value);
                invalidate();
              }}
            />
          </Field>
        </div>

        <fieldset className={local.composition}>
          <legend>{t('export.include')}</legend>
          {SECTIONS.map((section) => (
            <Checkbox
              key={section}
              id={`export-section-${section}`}
              checked={sections.includes(section)}
              label={t(`export.section.${section}`)}
              onChange={(event) => {
                toggle(section, event.target.checked);
              }}
            />
          ))}
        </fieldset>

        <div className={styles.toolbar}>
          <Button variant="secondary" loading={preparing} onClick={() => void prepare()}>
            {t('rename.preview')}
          </Button>
          <Button
            variant="primary"
            loading={starting}
            disabled={preview === null || running}
            onClick={() => void start()}
          >
            {t('export.export')}
          </Button>
          <span className={styles.spacer} />
          {running ? (
            <Button variant="ghost" onClick={() => void cancel()}>
              {t('operation.cancel')}
            </Button>
          ) : null}
        </div>
      </Card>

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void prepare()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {preview !== null ? (
        <Card
          title={t('rename.preview')}
          actions={<Badge tone="info">{FORMAT_LABELS[preview.format]}</Badge>}
        >
          <p className={local.path}>{preview.out}</p>
          <ul className={local.chips} aria-label={t('export.include')}>
            {preview.sections.map((section) => (
              <li key={section}>{t(`export.section.${section}`)}</li>
            ))}
          </ul>
          <div className={styles.metaRow}>
            <span>
              {preview.row_estimate === null
                ? t('scan.objects', { count: t('metadata.none') })
                : tPlural('scan.objects', preview.row_estimate, {
                    count: formatNumber(preview.row_estimate, language),
                  })}
            </span>
            <span>
              {t('archives.total', {
                size: formatBytes(preview.total_bytes, units),
              })}
            </span>
            <span>{formatBytes(preview.on_disk_bytes, units)}</span>
            <span>{formatBytes(preview.physical_bytes, units)}</span>
          </div>
          {preview.warnings.length > 0 ? (
            <ul className={local.warnings}>
              {preview.warnings.map((warning) => (
                <li key={warning}>{t(warning)}</li>
              ))}
            </ul>
          ) : null}
        </Card>
      ) : null}

      {task !== null ? (
        <Card
          title={t('export.exporting')}
          actions={<Badge tone={stateTone(task.state)}>{stateLabel(task.state)}</Badge>}
        >
          <Progress
            label={t('export.exporting')}
            value={
              task.total === 0
                ? 0
                : Math.round((task.processed / task.total) * 100)
            }
            state={
              task.state === 'failed'
                ? 'error'
                : task.state === 'cancelled'
                  ? 'cancelled'
                  : 'normal'
            }
          />
          <p className={styles.metaRow}>
            {formatNumber(task.processed, language)} /{' '}
            {formatNumber(task.total, language)} ·{' '}
            {formatNumber(task.rows_written, language)} ·{' '}
            {formatBytes(task.bytes_written ?? 0, units)}
          </p>
          {task.state === 'completed' ? (
            <Banner tone="info">
              {t('export.done', { path: task.out ?? output })}
            </Banner>
          ) : null}
          {task.state === 'partial' ? (
            <Banner tone="warning">{t('error.operation_partial')}</Banner>
          ) : null}
          {task.state === 'failed' ? (
            <Banner tone="error">
              {task.error === null ? t('common.error') : t(task.error.message_key)}
            </Banner>
          ) : null}
          {task.state === 'cancelled' ? (
            <Banner tone="warning">{t('status.cancelled')}</Banner>
          ) : null}
        </Card>
      ) : null}
    </div>
  );
}

/** Report composition, preview and offline export lifecycle. */
export function Export() {
  const generation = useSnapshotStore((state) => state.generation);
  const volume = useSnapshotStore((state) => state.volume);

  if (generation === null) {
    return (
      <EmptyState
        title={t('analysis.scan_required')}
        description={t('scan.required')}
        actions={
          <Button
            variant="primary"
            onClick={() => {
              useShellStore.getState().setScreen('scan');
            }}
          >
            {t('scan.start')}
          </Button>
        }
      />
    );
  }

  return (
    <ExportForm
      generation={generation}
      defaultOutput={joinPath(volume?.mount_point ?? null, 'FreeSpace-report.csv')}
      initialSections={ALL_SECTIONS}
    />
  );
}

export interface ExportDialogProps {
  open: boolean;
  generation: number | null;
  defaultOutput: string;
  initialSections: readonly ExportSection[];
  onClose: () => void;
}

/** Reuses the report composer from the owners drill-down. */
export function ExportDialog({
  open,
  generation,
  defaultOutput,
  initialSections,
  onClose,
}: ExportDialogProps) {
  return (
    <Dialog
      open={open}
      title={t('export.export')}
      onClose={onClose}
      footer={
        <Button variant="ghost" onClick={onClose}>
          {t('common.close')}
        </Button>
      }
    >
      {generation === null ? (
        <EmptyState title={t('analysis.scan_required')} description={t('scan.required')} />
      ) : (
        <ExportForm
          generation={generation}
          defaultOutput={defaultOutput}
          initialSections={initialSections}
        />
      )}
    </Dialog>
  );
}
