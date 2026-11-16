import { useCallback, useEffect, useState } from 'react';

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
import { Card } from '../components/Card';
import { DataTable, type Column } from '../components/DataTable';
import { EmptyState, ErrorState, Progress } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { t } from '../i18n';
import { formatBytes, formatDate, formatPercent } from '../i18n/format';
import { emptyFilter } from '../lib/filters';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './inspection.module.css';
import { ExportDialog } from './Export';

const OWNER_PAGE_SIZE = 5;
const FILE_PAGE_SIZE = 5;

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

/** Owner totals, a cursor-paged file drill-down and a report action. */
export function Owners() {
  const generation = useSnapshotStore((state) => state.generation);
  const volume = useSnapshotStore((state) => state.volume);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [filter, setFilter] = useState<FilterSet>(emptyFilter());
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
          page_size: OWNER_PAGE_SIZE,
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
    [filter, generation],
  );

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

  const summaryColumns: Column<OwnerSummary>[] = [
    {
      id: 'owner',
      header: t('owners.owner'),
      width: 'minmax(180px, 1.4fr)',
      render: (owner) => (
        <span className={local.owner}>
          <span>{ownerLabel(owner)}</span>
          {owner.owner_id === null ? (
            <Badge tone="warning">{t('owners.unknown')}</Badge>
          ) : null}
          {!owner.accessible ? (
            <Badge tone="warning">{t('tree.access_denied')}</Badge>
          ) : null}
        </span>
      ),
    },
    {
      id: 'sid',
      header: t('owners.sid'),
      width: 'minmax(140px, 1fr)',
      render: (owner) => (
        <span className={local.sid}>{owner.sid === '' ? t('owners.unknown') : owner.sid}</span>
      ),
    },
    {
      id: 'files',
      header: t('owners.files'),
      width: '110px',
      align: 'end',
      render: (owner) => owner.file_count,
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
      id: 'open',
      header: '',
      width: '130px',
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
      <Card title={t('nav.owners')}>
        <p>{t('owners.intro')}</p>
        <div className={styles.toolbar}>
          <Button variant="primary" loading={loading} onClick={() => void loadOwners()}>
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
        {loading ? <Progress label={t('owners.analyzing')} state="indeterminate" /> : null}
      </Card>

      <FilterBar
        screen="owners"
        filter={filter}
        onChange={(next) => {
          setFilter(next);
          setPage(null);
          setOwners([]);
          setSelected(null);
          setFiles([]);
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

      {owners.length === 0 && !loading && error === null ? (
        <EmptyState title={t('owners.empty')} />
      ) : null}

      {owners.length > 0 ? (
        <Card title={t('owners.analyze')}>
          <DataTable
            label={t('nav.owners')}
            rows={owners}
            columns={summaryColumns}
            rowKey={(owner) => `${owner.owner_id ?? 'unknown'}-${owner.sid}`}
            loading={loading}
            height={260}
            onActivate={(owner) => void loadFiles(owner)}
            empty={t('owners.empty')}
          />
          {page?.next_cursor !== null && page?.next_cursor !== undefined ? (
            <div className={styles.analyzeRow}>
              <Button
                variant="secondary"
                loading={loading}
                onClick={() => void loadOwners(page.next_cursor)}
              >
                {t('tree.load_more')}
              </Button>
            </div>
          ) : null}
          {page !== null && (page.unknown_bytes > 0 || page.error_count > 0) ? (
            <p className={styles.metaRow}>
              {page.unknown_bytes > 0
                ? `${t('owners.unknown')}: ${formatBytes(page.unknown_bytes, units)}`
                : null}
              {page.error_count > 0 ? ` · ${t('tree.access_denied')}: ${page.error_count}` : null}
            </p>
          ) : null}
        </Card>
      ) : null}

      {selected !== null ? (
        <Card
          title={ownerLabel(selected)}
          actions={<Badge tone="info">{selected.sid === '' ? t('owners.unknown') : selected.sid}</Badge>}
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
                <div className={styles.analyzeRow}>
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
        </Card>
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
