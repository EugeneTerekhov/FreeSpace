import { useCallback, useEffect, useMemo, useState } from 'react';

import { api } from '../api/runtime';
import type { FilterSet, NodeRow, SortKey, SortSpec, TreePage, UiError } from '../api/dto';
import { normalizeError } from '../app/errors';
import { openInTree } from '../app/navigation';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { DataTable, type Column, type SortState } from '../components/DataTable';
import { EmptyState, ErrorState } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { emptyFilter } from '../lib/filters';
import { Select } from '../components/Fields';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes, formatDate } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';

const LIMITS = [100, 1000, 5000];
const DEFAULT_FILTER = emptyFilter();

/** Largest files with shared filters and the common operation preview. */
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

  const selectedRows = useMemo(
    () => rows.filter((row) => selected.has(row.id)),
    [rows, selected],
  );

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
      width: 'minmax(240px, 2fr)',
      sortable: true,
      render: (row) => (
        <span className={styles.pathCell} title={row.path}>
          <span>{row.name}</span>
        </span>
      ),
    },
    {
      id: 'path',
      header: t('details.path'),
      width: 'minmax(200px, 1.4fr)',
      render: (row) => <span className={styles.pathCell}>{row.path}</span>,
    },
    {
      id: 'size',
      header: t('top.size'),
      width: '120px',
      align: 'end',
      sortable: true,
      render: (row) => formatBytes(row.logical_size, units),
    },
    {
      id: 'modified',
      header: t('top.modified'),
      width: '180px',
      sortable: true,
      render: (row) => formatDate(row.modified_at, language),
    },
    {
      id: 'open',
      header: '',
      width: '140px',
      render: (row) => (
        <Button
          variant="ghost"
          onClick={() => {
            openInTree(row.id);
          }}
        >
          {t('treemap.open_tree')}
        </Button>
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
      <div className={styles.toolbar}>
        <label>
          {t('top.limit')}{' '}
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

      <FilterBar screen="top_files" filter={filter} onChange={setFilter} />

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

      <Card title={t('nav.top')}>
        <DataTable
          label={t('nav.top')}
          rows={rows}
          columns={columns}
          rowKey={(row) => row.id}
          sort={sort}
          onSortChange={setSort}
          loading={loading}
          height={520}
          empty={t('top.empty')}
        />
        {page?.next_cursor !== null && page?.next_cursor !== undefined ? (
          <div className={styles.analyzeRow}>
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
      </Card>

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
