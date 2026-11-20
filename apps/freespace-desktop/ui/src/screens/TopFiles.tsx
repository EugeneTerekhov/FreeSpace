import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Folder } from 'lucide-react';

import { api } from '../api/runtime';
import type { FilterSet, NodeRow, SortKey, SortSpec, TreePage, UiError } from '../api/dto';
import { normalizeError } from '../app/errors';
import { openInTree } from '../app/navigation';
import { Button, IconButton } from '../components/Button';
import { DataTable, type Column, type RowAnchor, type SortState } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, Progress } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { Select } from '../components/Fields';
import {
  HoverCard,
  HoverCardHead,
  HoverCardMetric,
  HoverCardMetrics,
  HoverCardNote,
  HoverCardPath,
} from '../components/HoverCard';
import { emptyFilter } from '../lib/filters';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes, formatDateDmy, formatNumber } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import styles from './analysis-screen.module.css';

const LIMITS = [100, 1000, 5000];
const DEFAULT_FILTER = emptyFilter();
const HOVER_DELAY = 450;

/** Largest files tab: N, filters, a stat row and the shared delete preview. */
export function TopFiles() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [limit, setLimit] = useState(100);
  const [filter, setFilter] = useState<FilterSet>(DEFAULT_FILTER);
  const [applyFilter] = useDebounced(filter);
  const [sort, setSort] = useState<SortState>({ key: 'size', descending: true });
  const [rows, setRows] = useState<NodeRow[]>([]);
  const [page, setPage] = useState<TreePage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<number>>(new Set());
  const [hoverRow, setHoverRow] = useState<{ node: NodeRow; anchor: RowAnchor } | null>(null);
  const hoverTimer = useRef<number | null>(null);
  const operation = useOperation();

  const load = useCallback(
    async (cursor?: string | null) => {
      if (generation === null) return;
      setLoading(true);
      setError(null);
      try {
        const sortSpec: SortSpec = {
          key: sort.key as SortKey,
          descending: sort.descending,
        };
        const result = await api.topFilesPage(generation, {
          limit,
          sort: sortSpec,
          filter: applyFilter,
          cursor: cursor ?? null,
          pageSize: 200,
        });
        setRows((current) =>
          cursor === undefined || cursor === null ? result.rows : [...current, ...result.rows],
        );
        setPage(result);
      } catch (cause) {
        setError(normalizeError(cause));
      } finally {
        setLoading(false);
      }
    },
    [applyFilter, generation, limit, sort],
  );

  useEffect(() => {
    if (generation !== null) void load();
  }, [generation, limit, applyFilter, sort, load]);

  // Selection is keyed by row id and survives filter changes; a new scan
  // invalidates it. This keeps a destructive preview stable when a debounced
  // filter lands while the confirmation dialog is open.
  useEffect(() => {
    setSelected(new Set());
  }, [generation]);

  // The card opens on data the page already has: no extra IPC per row.
  const closeHover = useCallback(() => {
    if (hoverTimer.current !== null) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = null;
    }
    setHoverRow(null);
  }, []);

  const openHover = useCallback((node: NodeRow, anchor: RowAnchor) => {
    if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => {
      hoverTimer.current = null;
      setHoverRow({ node, anchor });
    }, HOVER_DELAY);
  }, []);

  useEffect(
    () => () => {
      if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
    },
    [],
  );

  // A card that stays put while the row scrolls away would point at nothing.
  useEffect(() => {
    if (hoverRow === null) return undefined;
    const hide = () => closeHover();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') hide();
    };
    document.addEventListener('scroll', hide, true);
    document.addEventListener('keydown', onKeyDown);
    window.addEventListener('resize', hide);

    return () => {
      document.removeEventListener('scroll', hide, true);
      document.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('resize', hide);
    };
  }, [closeHover, hoverRow]);

  const selectedRows = useMemo(
    () => rows.filter((row) => selected.has(row.id)),
    [rows, selected],
  );
  const totalSize = useMemo(
    () => rows.reduce((sum, row) => sum + row.logical_size, 0),
    [rows],
  );
  const inaccessible = useMemo(
    () => rows.filter((row) => !row.accessible).length,
    [rows],
  );
  const totalEstimate = page?.total_estimate ?? rows.length;

  const columns: Column<NodeRow>[] = [
    {
      id: 'select',
      header: '',
      width: '40px',
      render: (row) => (
        <input
          type="checkbox"
          aria-label={`${t('tree.selected')}: ${row.name}`}
          checked={selected.has(row.id)}
          onChange={(event) => {
            setSelected((current) => {
              const next = new Set(current);
              if (event.target.checked) next.add(row.id);
              else next.delete(row.id);
              return next;
            });
          }}
        />
      ),
    },
    {
      id: 'name',
      header: t('top.path'),
      width: 'minmax(220px, 1.6fr)',
      sortable: true,
      render: (row) => (
        <span className={`${styles.pathCell} ${styles.fileName}`}>{row.name}</span>
      ),
    },
    {
      id: 'path',
      header: t('details.path'),
      width: 'minmax(200px, 1.4fr)',
      sortable: true,
      render: (row) => <span className={styles.pathCell}>{row.path}</span>,
    },
    {
      id: 'size',
      header: t('top.size'),
      width: '100px',
      align: 'end',
      sortable: true,
      render: (row) => formatBytes(row.logical_size, units),
    },
    {
      id: 'modified',
      header: t('top.modified'),
      width: '112px',
      sortable: true,
      render: (row) => formatDateDmy(row.modified_at),
    },
    {
      id: 'open',
      header: '',
      width: '54px',
      render: (row) => (
        <IconButton
          label={t('treemap.open_tree')}
          onClick={() => {
            openInTree(row.id);
          }}
        >
          <Folder size={15} strokeWidth={1.7} aria-hidden="true" />
        </IconButton>
      ),
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

  return (
    <div className={styles.stack}>
      <p className={styles.context} data-testid="analysis-context-top">
        {t('analysis_ui.context_top', {
          shown: formatNumber(rows.length, language),
          total: formatNumber(totalEstimate, language),
          size: formatBytes(totalSize, units),
        })}
      </p>

      <StatRow>
        <Stat
          label={t('analysis_ui.stat_files')}
          value={<span data-testid="analysis-top-shown">{formatNumber(rows.length, language)}</span>}
        />
        <Stat
          label={t('analysis_ui.stat_size')}
          value={<span data-testid="analysis-top-size">{formatBytes(totalSize, units)}</span>}
        />
        <Stat
          label={t('analysis_ui.stat_total')}
          value={
            <span data-testid="analysis-top-total">
              {formatNumber(totalEstimate, language)}
            </span>
          }
        />
      </StatRow>

      <div className={styles.toolbar}>
        <label className={styles.field}>
          {t('top.limit')}
          <Select
            aria-label={t('top.limit')}
            value={String(limit)}
            options={LIMITS.map((value) => ({ value: String(value), label: String(value) }))}
            onChange={(event) => {
              setLimit(Number(event.target.value));
            }}
          />
        </label>
        <span className={styles.spacer} />
        <Button
          variant="danger"
          disabled={selectedRows.length === 0}
          onClick={() => {
            void operation.prepare({
              kind: 'delete',
              targets: selectedRows.map((row) => row.path),
              delete_mode: 'recycle_bin',
              reason: 'top files cleanup',
            });
          }}
        >
          {t('analysis.delete_selected', { count: selectedRows.length })}
        </Button>
      </div>

      <FilterBar filter={filter} onChange={setFilter} />

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void load()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {inaccessible > 0 ? (
        <Banner tone="warning">
          {t('analysis_ui.partial')} · {t('tree.access_denied')}: {inaccessible}
        </Banner>
      ) : null}

      <Panel title={t('nav.top')}>
        {loading && rows.length === 0 ? (
          <Progress label={t('common.loading')} state="indeterminate" />
        ) : null}

        {!loading && rows.length === 0 && error === null ? (
          <EmptyState
            title={t('top.empty')}
            actions={
              <Button
                variant="secondary"
                onClick={() => {
                  setFilter(DEFAULT_FILTER);
                }}
              >
                {t('filter.reset')}
              </Button>
            }
          />
        ) : (
          <DataTable
            label={t('nav.top')}
            rows={rows}
            columns={columns}
            rowKey={(row) => row.id}
            sort={sort}
            onSortChange={setSort}
            onRowHoverStart={openHover}
            onRowHoverEnd={closeHover}
            loading={loading}
            height={520}
            empty={t('top.empty')}
          />
        )}

        {page?.next_cursor !== null && page?.next_cursor !== undefined ? (
          <div className={styles.loadMore}>
            <Button
              variant="secondary"
              loading={loading}
              onClick={() => {
                void load(page.next_cursor);
              }}
            >
              {t('tree.load_more')}
            </Button>
          </div>
        ) : null}
      </Panel>

      {hoverRow === null ? null : (
        <HoverCard anchor={hoverRow.anchor} label={t('tree2.title_details')}>
          <HoverCardHead
            title={hoverRow.node.name}
            meta={t(`kind.${hoverRow.node.kind}`)}
          />
          <HoverCardPath title={hoverRow.node.path}>{hoverRow.node.path}</HoverCardPath>
          <HoverCardMetrics>
            <HoverCardMetric label={t('details.logical')} value={formatBytes(hoverRow.node.logical_size, units)} />
            <HoverCardMetric label={t('details.allocated')} value={formatBytes(hoverRow.node.on_disk_size, units)} />
            <HoverCardMetric label={t('details.modified')} value={formatDateDmy(hoverRow.node.modified_at)} />
          </HoverCardMetrics>
          {hoverRow.node.accessible ? null : (
            <HoverCardNote>{t('error.access_denied')}</HoverCardNote>
          )}
        </HoverCard>
      )}

      {operation.progress !== null ? (
        <OperationProgressView
          progress={operation.progress}
          onCancel={() => void operation.cancel()}
          onUndo={() => void operation.undo()}
        />
      ) : null}

      <OperationPreviewDialog
        open={operation.preview !== null}
        preview={operation.preview}
        busy={operation.busy}
        error={operation.error}
        onCancel={operation.close}
        onConfirm={(mode, extra) => void operation.confirm(mode, extra)}
      />
    </div>
  );
}

function useDebounced(filter: FilterSet): [FilterSet] {
  const [value, setValue] = useState(filter);
  useEffect(() => {
    const timer = setTimeout(() => {
      setValue(filter);
    }, 250);

    return () => {
      clearTimeout(timer);
    };
  }, [filter]);

  return [value];
}
