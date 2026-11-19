import { useCallback, useEffect, useRef, useState } from 'react';

import type { FilterSet, NodeRow, TreePage, UiError } from '../api/dto';
import type {
  ExportSection,
  OwnerSummary,
  OwnerSummaryPage,
} from '../api/dto-inspection';
import { fetchOwnerFiles, fetchOwnersPage } from '../api/inspection';
import { normalizeError } from '../app/errors';
import { Badge } from '../components/Choice';
import { Button } from '../components/Button';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, Progress } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { Select } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes, formatDate, formatNumber, formatPercent } from '../i18n/format';
import { emptyFilter } from '../lib/filters';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import { ExportDialog } from '../components/ExportAction';
import styles from './analysis-screen.module.css';

const OWNER_PAGE_SIZES = [5, 10, 25];
const FILE_PAGE_SIZE = 25;

const OWNERS_COMPOSITION: readonly ExportSection[] = ['owners'];

function ownerLabel(owner: OwnerSummary): string {
  return owner.owner_id === null ? t('owners.unknown') : owner.display_name;
}

function ownerKey(owner: OwnerSummary): string {
  if (owner.owner_id === null) return 'unknown';

  return owner.sid === '' ? owner.display_name : owner.sid;
}

function ownersOutput(root: string | null): string {
  const base = root ?? 'C:\\';

  return `${base}${base.endsWith('\\') ? '' : '\\'}FreeSpace-owners.csv`;
}

/** Owners tab: owner totals with a cursor-paged file drill-down. */
export function Owners() {
  const generation = useSnapshotStore((state) => state.generation);
  const volume = useSnapshotStore((state) => state.volume);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [filter, setFilter] = useState<FilterSet>(emptyFilter());
  const [pageSize, setPageSize] = useState(5);
  const [requested, setRequested] = useState(false);
  const requestedFor = useRef<number | null>(null);
  const [page, setPage] = useState<OwnerSummaryPage | null>(null);
  const [owners, setOwners] = useState<OwnerSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [selected, setSelected] = useState<OwnerSummary | null>(null);
  const [files, setFiles] = useState<NodeRow[]>([]);
  const [filesPage, setFilesPage] = useState<TreePage | null>(null);
  const [filesLoading, setFilesLoading] = useState(false);
  const [filesError, setFilesError] = useState<UiError | null>(null);
  const [exportOpen, setExportOpen] = useState(false);

  useEffect(() => {
    setPage(null);
    setOwners([]);
    setSelected(null);
    setFiles([]);
    setFilesPage(null);
    setError(null);
    setFilesError(null);
    setRequested(false);
    requestedFor.current = null;
  }, [generation]);

  const loadOwners = useCallback(
    async (cursor: string | null = null) => {
      if (generation === null) return;
      setLoading(true);
      setError(null);
      try {
        const result = await fetchOwnersPage(generation, {
          filter,
          cursor,
          page_size: pageSize,
        });
        setPage(result);
        setOwners((current) =>
          cursor === null ? result.rows : [...current, ...result.rows],
        );
      } catch (cause) {
        setError(normalizeError(cause));
      } finally {
        setLoading(false);
      }
    },
    [filter, generation, pageSize],
  );

  // Once the report is built, filter and page-size changes re-read it so the
  // tab behaves like the other analysis tabs. Nothing is read before the first
  // explicit «Построить отчёт», because owner resolution is expensive.
  useEffect(() => {
    if (!requested || generation === null || requestedFor.current !== generation) return;
    void loadOwners(null);
  }, [generation, loadOwners, requested]);

  const loadFiles = useCallback(
    async (owner: OwnerSummary, cursor: string | null = null) => {
      if (generation === null) return;
      setSelected(owner);
      setFilesLoading(true);
      setFilesError(null);
      try {
        const result = await fetchOwnerFiles(
          generation,
          ownerKey(owner),
          { filter, cursor, pageSize: FILE_PAGE_SIZE },
        );
        setFilesPage(result);
        setFiles((current) =>
          cursor === null ? result.rows : [...current, ...result.rows],
        );
      } catch (cause) {
        setFilesError(normalizeError(cause));
      } finally {
        setFilesLoading(false);
      }
    },
    [filter, generation],
  );

  const ownerColumns: Column<OwnerSummary>[] = [
    {
      id: 'owner',
      header: t('owners.owner'),
      width: 'minmax(180px, 1.4fr)',
      render: (owner) => (
        <span className={styles.fileName}>{ownerLabel(owner)}</span>
      ),
    },
    {
      id: 'sid',
      header: t('owners.sid'),
      width: 'minmax(140px, 1fr)',
      render: (owner) => (
        <span className={styles.panelMeta}>
          {owner.sid === '' ? t('owners.unknown') : owner.sid}
        </span>
      ),
    },
    {
      id: 'files',
      header: t('owners.files'),
      width: '110px',
      align: 'end',
      render: (owner) => formatNumber(owner.file_count, language),
    },
    {
      id: 'size',
      header: t('owners.size'),
      width: '120px',
      align: 'end',
      render: (owner) => formatBytes(owner.total_bytes, units),
    },
    {
      id: 'share',
      header: t('owners.share'),
      width: '100px',
      align: 'end',
      render: (owner) => formatPercent(owner.share, language, 1),
    },
    {
      id: 'state',
      header: t('unlock.state'),
      width: '150px',
      render: (owner) =>
        owner.owner_id === null || !owner.accessible ? (
          <Badge tone="warning">
            {owner.owner_id === null ? t('owners.unknown') : t('tree.access_denied')}
          </Badge>
        ) : null,
    },
    {
      id: 'open',
      header: '',
      width: '150px',
      render: (owner) => (
        <Button variant="ghost" onClick={() => void loadFiles(owner)}>
          {t('details.items')}
        </Button>
      ),
    },
  ];

  const fileColumns: Column<NodeRow>[] = [
    {
      id: 'path',
      header: t('top.path'),
      width: 'minmax(240px, 2fr)',
      render: (row) => (
        <span className={styles.pathCell} title={row.path}>
          {row.path}
        </span>
      ),
    },
    {
      id: 'size',
      header: t('top.size'),
      width: '120px',
      align: 'end',
      render: (row) => formatBytes(row.logical_size, units),
    },
    {
      id: 'modified',
      header: t('top.modified'),
      width: '180px',
      render: (row) => formatDate(row.modified_at, language),
    },
    {
      id: 'access',
      header: t('unlock.state'),
      width: '150px',
      render: (row) =>
        row.accessible ? null : <Badge tone="warning">{t('tree.access_denied')}</Badge>,
    },
  ];

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

  const totalOwners = page?.total_estimate ?? owners.length;
  const totalBytes =
    page?.total_bytes ?? owners.reduce((sum, owner) => sum + owner.total_bytes, 0);
  const unknownBytes = page?.unknown_bytes ?? 0;

  const buildReport = () => {
    if (!requested) {
      requestedFor.current = generation;
      setRequested(true);
      return;
    }
    void loadOwners(null);
  };

  return (
    <div className={styles.stack}>
      <p className={styles.context} data-testid="analysis-context-owners">
        {page === null
          ? t('owners.intro')
          : t('analysis_ui.context_owners', {
              owners: formatNumber(totalOwners, language),
              size: formatBytes(totalBytes, units),
            })}
      </p>

      <div className={styles.toolbar}>
        <label className={styles.field}>
          {t('analysis_ui.limit_owners')}
          <Select
            aria-label={t('analysis_ui.limit_owners')}
            value={String(pageSize)}
            options={OWNER_PAGE_SIZES.map((value) => ({ value: String(value), label: String(value) }))}
            onChange={(event) => {
              setPageSize(Number(event.target.value));
            }}
          />
        </label>
        <Button variant="primary" loading={loading} onClick={buildReport}>
          {t('owners.analyze')}
        </Button>
        <span className={styles.spacer} />
        <Button
          variant="secondary"
          disabled={owners.length === 0}
          onClick={() => {
            setExportOpen(true);
          }}
        >
          {t('export.export')}
        </Button>
      </div>

      <FilterBar
        screen="owners"
        filter={filter}
        onChange={(next) => {
          setFilter(next);
          setSelected(null);
          setFiles([]);
          setFilesPage(null);
        }}
      />

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void loadOwners()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {loading ? <Progress label={t('owners.analyzing')} state="indeterminate" /> : null}

      {requested && page !== null ? (
        <StatRow>
          <Stat
            label={t('analysis_ui.stat_owners')}
            value={
              <span data-testid="analysis-owners-count">
                {formatNumber(totalOwners, language)}
              </span>
            }
          />
          <Stat
            label={t('analysis_ui.stat_size')}
            value={
              <span data-testid="analysis-owners-size">{formatBytes(totalBytes, units)}</span>
            }
          />
          <Stat
            label={t('owners.unknown')}
            value={
              <span data-testid="analysis-owners-unknown">
                {formatBytes(unknownBytes, units)}
              </span>
            }
          />
        </StatRow>
      ) : null}

      {!requested ? (
        <EmptyState
          title={t('owners.empty')}
          actions={
            <Button variant="primary" onClick={buildReport}>
              {t('owners.analyze')}
            </Button>
          }
        />
      ) : owners.length === 0 && !loading && error === null ? (
        <EmptyState title={t('owners.empty')} />
      ) : (
        <Panel
          title={t('nav.owners')}
          actions={
            page === null ? undefined : (
              <span className={styles.panelMeta}>
                {t('owners.unknown')}: {formatBytes(unknownBytes, units)}
              </span>
            )
          }
        >
          <DataTable
            label={t('nav.owners')}
            rows={owners}
            columns={ownerColumns}
            rowKey={(owner) => `${owner.owner_id ?? 'unknown'}-${owner.sid}`}
            loading={loading}
            height={320}
            onActivate={(owner) => void loadFiles(owner)}
            empty={t('owners.empty')}
          />
          {page?.next_cursor !== null && page?.next_cursor !== undefined ? (
            <div className={styles.loadMore}>
              <Button
                variant="secondary"
                loading={loading}
                onClick={() => void loadOwners(page.next_cursor)}
              >
                {t('tree.load_more')}
              </Button>
            </div>
          ) : null}
          {page !== null && page.error_count > 0 ? (
            <Banner tone="warning">
              {t('analysis_ui.partial')} · {t('tree.access_denied')}: {page.error_count}
            </Banner>
          ) : null}
        </Panel>
      )}

      {selected !== null ? (
        <Panel
          title={ownerLabel(selected)}
          actions={
            <Badge tone="info">
              {selected.sid === '' ? t('owners.unknown') : selected.sid}
            </Badge>
          }
        >
          {filesError !== null ? (
            <ErrorState
              title={t(filesError.message_key)}
              code={filesError.code}
              details={filesError.details ?? undefined}
              actions={
                <Button variant="primary" onClick={() => void loadFiles(selected)}>
                  {t('error.retry')}
                </Button>
              }
            />
          ) : (
            <>
              <DataTable
                label={t('owners.files')}
                rows={files}
                columns={fileColumns}
                rowKey={(row) => row.id}
                loading={filesLoading}
                height={320}
                empty={t('owners.empty')}
              />
              {filesPage?.next_cursor !== null && filesPage?.next_cursor !== undefined ? (
                <div className={styles.loadMore}>
                  <Button
                    variant="secondary"
                    loading={filesLoading}
                    onClick={() => void loadFiles(selected, filesPage.next_cursor)}
                  >
                    {t('tree.load_more')}
                  </Button>
                </div>
              ) : null}
            </>
          )}
        </Panel>
      ) : null}

      <ExportDialog
        open={exportOpen}
        generation={generation}
        defaultOutput={ownersOutput(volume?.mount_point ?? null)}
        initialSections={OWNERS_COMPOSITION}
        onClose={() => {
          setExportOpen(false);
        }}
      />
    </div>
  );
}
