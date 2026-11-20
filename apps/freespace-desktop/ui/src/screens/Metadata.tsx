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
import { Banner, EmptyState, ErrorState, Progress } from '../components/Feedback';
import { Field, Select } from '../components/Fields';
import { t, tPlural, type MessageKey } from '../i18n';
import { formatBytes, formatDate, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import local from './inspect.module.css';

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

const STATUS_KEYS: Record<MetadataStatus, MessageKey> = {
  supported: 'common.ok',
  unsupported: 'error.unsupported',
  corrupt: 'common.error',
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

function StatusBadge({ status }: { status: MetadataStatus }) {
  const tone = status === 'supported' ? 'success' : status === 'unsupported' ? 'warning' : 'danger';
  const Icon =
    status === 'supported' ? CheckCircle2 : status === 'unsupported' ? AlertTriangle : XCircle;

  return (
    <Badge tone={tone}>
      <span className={local.status}>
        <Icon size={14} aria-hidden="true" />
        <span>{t(STATUS_KEYS[status])}</span>
      </span>
    </Badge>
  );
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

/**
 * Read-only metadata extraction with kind and media-quality filters.
 *
 * Tags are grouped by file and rendered as label/value panels: EXIF, audio and
 * video properties and PE version fields never share one flat table, so a
 * camera make is not mistaken for a file version. Reading only opens files; it
 * never writes them.
 */
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

  if (generation === null) {
    return (
      <EmptyState
        title={t('analysis.scan_required')}
        actions={
          <Button
            variant="primary"
            onClick={() => {
              useShellStore.getState().setScreen('overview');
            }}
          >
            {t('scan.start')}
          </Button>
        }
      />
    );
  }

  const available = rows.filter((row) => row.metadata.available).length;
  const partial = rows.some((row) => !row.metadata.available);

  return (
    <div className={local.stack}>
      <Panel title={t('nav.metadata')} hint={t('inspect.metadata.hint')}>
        <form
          className={local.toolbar}
          onSubmit={(event) => {
            event.preventDefault();
            void read();
          }}
        >
          <Field id="metadata-path" label={t('metadata.path')}>
            <input
              id="metadata-path"
              className={local.input}
              value={query}
              spellCheck={false}
              onChange={(event) => {
                setQuery(event.target.value);
              }}
            />
          </Field>
          <Button type="submit" variant="primary" loading={extracting}>
            {t('metadata.read')}
          </Button>
        </form>

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
      </Panel>

      {rows.length > 0 ? (
        <>
          <StatRow>
            <Stat
              label={t('inspect.metadata.files')}
              value={formatNumber(rows.length, language)}
            />
            <Stat
              label={t('inspect.metadata.available')}
              value={formatNumber(available, language)}
            />
          </StatRow>
          <p className={local.metaRow}>
            {t('inspect.metadata.matches', {
              visible: formatNumber(visible.length, language),
              total: formatNumber(rows.length, language),
            })}
          </p>
        </>
      ) : null}

      {extracting && progress !== null ? (
        <Panel title={t('metadata.reading')}>
          <Progress
            label={t('metadata.reading')}
            value={
              progress.total === 0
                ? 0
                : Math.round((progress.processed / progress.total) * 100)
            }
          />
          <p className={local.metaRow}>
            {tPlural('scan.objects', progress.processed, {
              count: formatNumber(progress.processed, language),
            })}{' '}
            · {formatNumber(progress.total, language)}
          </p>
        </Panel>
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

      {partial ? <Banner tone="warning">{t('error.operation_partial')}</Banner> : null}

      {!extracting && error === null && visible.length === 0 ? (
        <EmptyState title={t('metadata.empty')} />
      ) : null}

      {visible.length > 0 ? (
        <section className={local.results} aria-label={t('metadata.details')}>
          {visible.map((row) => {
            const status = statusOf(row.metadata);

            return (
              <Panel
                key={`${row.metadata.node_id}-${row.metadata.path}`}
                title={
                  <span className={local.fileTitle} title={row.metadata.path}>
                    {row.metadata.path}
                  </span>
                }
                actions={
                  <span className={local.badgeRow}>
                    <Badge tone="neutral">{t(kindKey(row.metadata.kind))}</Badge>
                    <StatusBadge status={status} />
                    {row.metadata.source_key !== '' ? (
                      <Badge tone="info">{t(row.metadata.source_key)}</Badge>
                    ) : null}
                    {row.metadata.truncated ? (
                      <Badge tone="warning">{t('inspect.metadata.truncated')}</Badge>
                    ) : null}
                  </span>
                }
              >
                <div className={local.metaRow}>
                  <span>
                    {t('metadata.size')}:{' '}
                    <strong>{formatBytes(row.metadata.size_bytes, units)}</strong>
                  </span>
                  <span>
                    {t('details.modified')}:{' '}
                    <strong>{formatDate(row.metadata.modified_at)}</strong>
                  </span>
                </div>
                {row.metadata.tags.length === 0 ? (
                  <p className={local.note}>{t('inspect.metadata.tags_empty')}</p>
                ) : (
                  <dl className={local.tagList}>
                    {row.metadata.tags.map((tag) => (
                      <div className={local.tagRow} key={tag.key}>
                        <dt className={local.tagKey}>{tag.key}</dt>
                        <dd className={local.tagValue}>{tag.value}</dd>
                      </div>
                    ))}
                  </dl>
                )}
              </Panel>
            );
          })}
        </section>
      ) : null}
    </div>
  );
}
