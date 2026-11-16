import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AlertTriangle, CheckCircle2, XCircle } from 'lucide-react';

import { api } from '../api/runtime';
import type { NodeRow, UiError } from '../api/dto';
import type {
  MetadataKind,
  MetadataStatus,
  MetadataView,
} from '../api/dto-inspection';
import { fetchMetadata } from '../api/inspection';
import { normalizeError, validationError } from '../app/errors';
import { Badge } from '../components/Choice';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { DataTable, type Column } from '../components/DataTable';
import { EmptyState, ErrorState, Progress } from '../components/Feedback';
import { Field, Select } from '../components/Fields';
import { t, tPlural, type MessageKey } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './inspection.module.css';

interface MetadataRow {
  node: NodeRow;
  metadata: MetadataView;
}

const KIND_KEYS: Record<MetadataKind, MessageKey> = {
  exif: 'treemap.category.image',
  audio: 'treemap.category.audio',
  video: 'treemap.category.video',
  pe: 'treemap.category.executable',
};

const KINDS: readonly MetadataKind[] = ['exif', 'audio', 'video', 'pe'];

const METADATA_PAGE_SIZE = 60;

function tagNumber(metadata: MetadataView, key: string): number | null {
  const value = metadata.tags.find((tag) => tag.key === key)?.value;
  if (value === undefined) return null;
  const parsed = Number(value);

  return Number.isFinite(parsed) ? parsed : null;
}

function statusOf(metadata: MetadataView): MetadataStatus {
  if (metadata.available && metadata.status.toLowerCase() === 'ok') return 'supported';
  if (metadata.status.toLowerCase().includes('corrupt')) return 'corrupt';

  return 'unsupported';
}

function kindKey(kind: string): MessageKey {
  return KIND_KEYS[kind as MetadataKind] ?? 'metadata.none';
}

function optionalNumber(value: string): number | null {
  if (value.trim() === '') return null;
  const parsed = Number(value);

  return Number.isFinite(parsed) ? parsed : null;
}

function matches(
  metadata: MetadataView,
  kind: MetadataKind | 'all',
  minWidth: number | null,
  minHeight: number | null,
  minBitrate: number | null,
  minDuration: number | null,
): boolean {
  if (kind !== 'all' && metadata.kind !== kind) return false;
  if (minWidth !== null && (tagNumber(metadata, 'Width') ?? 0) < minWidth) return false;
  if (minHeight !== null && (tagNumber(metadata, 'Height') ?? 0) < minHeight) return false;
  if (minBitrate !== null && (tagNumber(metadata, 'BitrateKbps') ?? 0) < minBitrate) {
    return false;
  }
  if (minDuration !== null && (tagNumber(metadata, 'DurationSecs') ?? 0) < minDuration) {
    return false;
  }

  return true;
}

function statusLabel(status: MetadataStatus): string {
  if (status === 'supported') return t('common.ok');
  if (status === 'unsupported') return t('error.unsupported');

  return t('common.error');
}

function StatusCell({ status }: { status: MetadataStatus }) {
  const tone = status === 'supported' ? 'success' : status === 'unsupported' ? 'warning' : 'danger';
  const Icon =
    status === 'supported' ? CheckCircle2 : status === 'unsupported' ? AlertTriangle : XCircle;

  return (
    <Badge tone={tone}>
      <span className={local.status}>
        <Icon size={14} aria-hidden="true" />
        <span>{statusLabel(status)}</span>
      </span>
    </Badge>
  );
}

function tagsSummary(metadata: MetadataView): string {
  if (metadata.tags.length === 0) return t('metadata.none');

  return metadata.tags
    .slice(0, 4)
    .map((tag) => `${tag.key}=${tag.value}`)
    .join('; ');
}

function fallbackMetadata(node: NodeRow, error: UiError): MetadataView {
  const corrupt = error.details?.includes('metadata.corrupt') ?? false;

  return {
    node_id: node.id,
    path: node.path,
    kind: '',
    source_key: '',
    status: corrupt ? 'corrupted' : 'unsupported',
    available: false,
    tags: [],
    truncated: false,
    size_bytes: node.logical_size,
    modified_at: node.modified_at,
  };
}

/** Read-only metadata extraction with kind and media-quality filters. */
export function Metadata() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [query, setQuery] = useState('');
  const [kind, setKind] = useState<MetadataKind | 'all'>('all');
  const [minWidth, setMinWidth] = useState('');
  const [minHeight, setMinHeight] = useState('');
  const [minBitrate, setMinBitrate] = useState('');
  const [minDuration, setMinDuration] = useState('');
  const [rows, setRows] = useState<MetadataRow[]>([]);
  const [extracting, setExtracting] = useState(false);
  const [progress, setProgress] = useState<{ processed: number; total: number } | null>(null);
  const [error, setError] = useState<UiError | null>(null);
  const run = useRef(0);

  useEffect(
    () => () => {
      run.current += 1;
    },
    [],
  );

  const read = useCallback(async () => {
    if (generation === null) return;
    const needle = query.trim();
    if (needle === '') {
      setError(validationError(t('details.path_required')));
      return;
    }

    const current = run.current + 1;
    run.current = current;
    setExtracting(true);
    setError(null);
    setRows([]);
    setProgress({ processed: 0, total: 0 });

    try {
      const search = needle.split(/[\\/]/).filter(Boolean).at(-1) ?? needle;
      const page = await api.treeSearch(generation, search, {
        pageSize: METADATA_PAGE_SIZE,
      });
      if (run.current !== current) return;
      const files = page.rows
        .filter((row) => row.kind === 'file')
        .slice(0, METADATA_PAGE_SIZE);
      setProgress({ processed: 0, total: files.length });

      const collected: MetadataRow[] = [];
      for (const [index, node] of files.entries()) {
        if (run.current !== current) return;
        try {
          const metadata = await fetchMetadata(generation, node.id);
          collected.push({ node, metadata });
        } catch (cause) {
          collected.push({ node, metadata: fallbackMetadata(node, normalizeError(cause)) });
        }
        if (run.current !== current) return;
        setRows([...collected]);
        setProgress({ processed: index + 1, total: files.length });
      }
    } catch (cause) {
      if (run.current === current) setError(normalizeError(cause));
    } finally {
      if (run.current === current) {
        setExtracting(false);
        setProgress(null);
      }
    }
  }, [generation, query]);

  const visible = useMemo(() => {
    const width = optionalNumber(minWidth);
    const height = optionalNumber(minHeight);
    const bitrate = optionalNumber(minBitrate);
    const duration = optionalNumber(minDuration);

    return rows.filter((row) =>
      matches(row.metadata, kind, width, height, bitrate, duration),
    );
  }, [kind, minBitrate, minDuration, minHeight, minWidth, rows]);

  const columns: Column<MetadataRow>[] = [
    {
      id: 'path',
      header: t('metadata.path'),
      width: 'minmax(220px, 2fr)',
      render: (row) => (
        <span className={styles.pathCell} title={row.node.path}>
          {row.node.path}
        </span>
      ),
    },
    {
      id: 'kind',
      header: t('metadata.kind'),
      width: '130px',
      render: (row) => (
        <span title={row.metadata.source_key}>{t(kindKey(row.metadata.kind))}</span>
      ),
    },
    {
      id: 'status',
      header: t('metadata.status'),
      width: '190px',
      render: (row) => <StatusCell status={statusOf(row.metadata)} />,
    },
    {
      id: 'size',
      header: t('metadata.size'),
      width: '100px',
      align: 'end',
      render: (row) => formatBytes(row.metadata.size_bytes, units),
    },
    {
      id: 'width',
      header: t('metadata.min_width'),
      width: '100px',
      align: 'end',
      render: (row) => tagNumber(row.metadata, 'Width') ?? t('metadata.none'),
    },
    {
      id: 'height',
      header: t('metadata.min_height'),
      width: '100px',
      align: 'end',
      render: (row) => tagNumber(row.metadata, 'Height') ?? t('metadata.none'),
    },
    {
      id: 'bitrate',
      header: t('metadata.min_bitrate'),
      width: '120px',
      align: 'end',
      render: (row) => tagNumber(row.metadata, 'BitrateKbps') ?? t('metadata.none'),
    },
    {
      id: 'duration',
      header: t('metadata.min_duration'),
      width: '110px',
      align: 'end',
      render: (row) => tagNumber(row.metadata, 'DurationSecs') ?? t('metadata.none'),
    },
    {
      id: 'tags',
      header: t('metadata.details'),
      width: 'minmax(160px, 1.2fr)',
      render: (row) => (
        <span className={local.tags} title={tagsSummary(row.metadata)}>
          {tagsSummary(row.metadata)}
        </span>
      ),
    },
  ];

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
    <div className={styles.stack}>
      <Card title={t('nav.metadata')}>
        <p>{t('metadata.intro')}</p>
        <div className={styles.toolbar}>
          <Field id="metadata-path" label={t('metadata.path')}>
            <input
              id="metadata-path"
              className={local.input}
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  void read();
                }
              }}
            />
          </Field>
          <Button variant="primary" loading={extracting} onClick={() => void read()}>
            {t('metadata.read')}
          </Button>
          <span className={styles.spacer} />
          <span className={styles.metaRow}>
            {formatNumber(visible.length, language)} / {formatNumber(rows.length, language)}
          </span>
        </div>
      </Card>

      <Card title={t('filter.title')}>
        <div className={local.filterGrid}>
          <Field id="metadata-kind" label={t('metadata.kind')}>
            <Select
              id="metadata-kind"
              value={kind}
              options={[
                { value: 'all', label: t('metadata.all') },
                ...KINDS.map((value) => ({ value, label: t(KIND_KEYS[value]) })),
              ]}
              onChange={(event) => {
                setKind(event.target.value as MetadataKind | 'all');
              }}
            />
          </Field>
          <Field id="metadata-min-width" label={t('metadata.min_width')}>
            <input
              id="metadata-min-width"
              className={local.input}
              type="number"
              min={0}
              value={minWidth}
              onChange={(event) => {
                setMinWidth(event.target.value);
              }}
            />
          </Field>
          <Field id="metadata-min-height" label={t('metadata.min_height')}>
            <input
              id="metadata-min-height"
              className={local.input}
              type="number"
              min={0}
              value={minHeight}
              onChange={(event) => {
                setMinHeight(event.target.value);
              }}
            />
          </Field>
          <Field id="metadata-min-bitrate" label={t('metadata.min_bitrate')}>
            <input
              id="metadata-min-bitrate"
              className={local.input}
              type="number"
              min={0}
              value={minBitrate}
              onChange={(event) => {
                setMinBitrate(event.target.value);
              }}
            />
          </Field>
          <Field id="metadata-min-duration" label={t('metadata.min_duration')}>
            <input
              id="metadata-min-duration"
              className={local.input}
              type="number"
              min={0}
              value={minDuration}
              onChange={(event) => {
                setMinDuration(event.target.value);
              }}
            />
          </Field>
        </div>
      </Card>

      {extracting && progress !== null ? (
        <Card title={t('metadata.reading')}>
          <Progress
            label={t('metadata.reading')}
            value={
              progress.total === 0
                ? 0
                : Math.round((progress.processed / progress.total) * 100)
            }
          />
          <p className={styles.metaRow}>
            {tPlural('scan.objects', progress.processed, {
              count: formatNumber(progress.processed, language),
            })}{' '}
            · {formatNumber(progress.total, language)}
          </p>
        </Card>
      ) : null}

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void read()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {rows.length === 0 && !extracting && error === null ? (
        <EmptyState title={t('metadata.empty')} />
      ) : null}

      {rows.length > 0 ? (
        <Card title={t('metadata.details')}>
          <DataTable
            label={t('nav.metadata')}
            rows={visible}
            columns={columns}
            rowKey={(row) => `${row.metadata.node_id}-${row.metadata.path}`}
            loading={extracting}
            height={420}
            empty={t('metadata.empty')}
          />
        </Card>
      ) : null}
    </div>
  );
}
