import { useCallback, useEffect, useMemo, useState } from 'react';
import { Folder } from 'lucide-react';

import { api } from '../api/runtime';
import type {
  FilterSet,
  NodeRow,
  SortKey,
  SortSpec,
  TreePage,
  TypeSummary,
  UiError,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { openInTree } from '../app/navigation';
import { Button, IconButton } from '../components/Button';
import { Segmented } from '../components/Choice';
import { DataTable, type Column, type SortState } from '../components/DataTable';
import { EmptyState, ErrorState, Progress } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { Select } from '../components/Fields';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { emptyFilter } from '../lib/filters';
import { t } from '../i18n';
import { formatBytes, formatNumber, formatPercent } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import styles from './analysis-screen.module.css';

type GroupMode = 'extension' | 'category';

const GROUP_LIMITS = [10, 25, 50];

/** Space distribution by extension or category, with a drill-down table. */
export function Types() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [mode, setMode] = useState<GroupMode>('extension');
  const [limit, setLimit] = useState(25);
  const [filter, setFilter] = useState<FilterSet>(emptyFilter());
  const [summary, setSummary] = useState<TypeSummary[]>([]);
  const [selectedType, setSelectedType] = useState<string | null>(null);
  const [rows, setRows] = useState<NodeRow[]>([]);
  const [page, setPage] = useState<TreePage | null>(null);
  const [sort, setSort] = useState<SortState>({ key: 'size', descending: true });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<number>>(new Set());
  const [reloadToken, setReloadToken] = useState(0);
  const operation = useOperation();

  useEffect(() => {
    if (generation === null) return undefined;
    let active = true;
    setLoading(true);
    setError(null);
    const timer = setTimeout(() => {
      void api
        .typesSummary(generation, filter, mode)
        .then((result) => {
          if (!active) return;
          setSummary(result);
          setSelectedType((current) => {
            if (current !== null && result.some((row) => keyOf(row, mode) === current)) {
              return current;
            }
            return result[0] === undefined ? null : keyOf(result[0], mode);
          });
        })
        .catch((cause) => {
          if (active) setError(normalizeError(cause));
        })
        .finally(() => {
          if (active) setLoading(false);
        });
    }, 200);

    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [filter, generation, mode, reloadToken]);

  const loadFiles = useCallback(
    async (cursor?: string | null) => {
      if (generation === null || selectedType === null) return;
      setLoading(true);
      setError(null);
      try {
        const sortSpec: SortSpec = {
          key: sort.key as SortKey,
          descending: sort.descending,
        };
        const result = await api.typesFilesPage(generation, selectedType, {
          sort: sortSpec,
          filter,
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
    [filter, generation, selectedType, sort],
  );

  useEffect(() => {
    setRows([]);
    setSelected(new Set());
    if (generation !== null && selectedType !== null) void loadFiles();
  }, [generation, selectedType, loadFiles]);

  const selectedRows = useMemo(
    () => rows.filter((row) => selected.has(row.id)),
    [rows, selected],
  );
  const activeGroup = summary.find((row) => keyOf(row, mode) === selectedType);
  const visibleSummary = summary.slice(0, limit);
  const totalFiles = useMemo(
    () => summary.reduce((sum, row) => sum + row.count, 0),
    [summary],
  );
  const totalSize = useMemo(
    () => summary.reduce((sum, row) => sum + row.logical_size, 0),
    [summary],
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
      width: 'minmax(220px, 2fr)',
      sortable: true,
      render: (row) => (
        <span className={`${styles.pathCell} ${styles.fileName}`} title={row.path}>
          {row.name}
        </span>
      ),
    },
    {
      id: 'size',
      header: t('types.size'),
      width: '100px',
      align: 'end',
      sortable: true,
      render: (row) => formatBytes(row.logical_size, units),
    },
    {
      id: 'items',
      header: t('details.items'),
      width: '100px',
      align: 'end',
      sortable: true,
      render: (row) => formatNumber(row.child_count, language),
    },
    {
      id: 'open',
      header: '',
      width: '54px',
      render: (row) => (
        <IconButton label={t('treemap.open_tree')} onClick={() => openInTree(row.id)}>
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
          <Button variant="primary" onClick={() => useShellStore.getState().setScreen('overview')}>
            {t('scan.start')}
          </Button>
        }
      />
    );
  }

  return (
    <div className={styles.stack}>
      <p className={styles.context} data-testid="analysis-context-types">
        {t('analysis_ui.context_types', {
          groups: formatNumber(summary.length, language),
          files: formatNumber(totalFiles, language),
          size: formatBytes(totalSize, units),
        })}
      </p>

      <StatRow>
        <Stat
          label={t('analysis_ui.stat_groups')}
          value={<span data-testid="analysis-types-groups">{formatNumber(summary.length, language)}</span>}
        />
        <Stat
          label={t('analysis_ui.stat_files')}
          value={<span data-testid="analysis-types-files">{formatNumber(totalFiles, language)}</span>}
        />
        <Stat
          label={t('analysis_ui.stat_size')}
          value={<span data-testid="analysis-types-size">{formatBytes(totalSize, units)}</span>}
        />
      </StatRow>

      <div className={styles.toolbar}>
        <Segmented
          label={t('types.group')}
          value={mode}
          options={[
            { value: 'extension', label: t('types.by_extension') },
            { value: 'category', label: t('types.by_category') },
          ]}
          onChange={(value) => {
            setMode(value as GroupMode);
            setSelectedType(null);
          }}
        />
        <label className={styles.field}>
          {t('analysis_ui.limit_groups')}
          <Select
            aria-label={t('analysis_ui.limit_groups')}
            value={String(limit)}
            options={GROUP_LIMITS.map((value) => ({
              value: String(value),
              label: String(value),
            }))}
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
              reason: 'types cleanup',
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
            <Button
              variant="primary"
              onClick={() => {
                setReloadToken((current) => current + 1);
                void loadFiles();
              }}
            >
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {loading && summary.length === 0 ? (
        <Progress label={t('common.loading')} state="indeterminate" />
      ) : null}

      {!loading && summary.length === 0 && error === null ? (
        <EmptyState title={t('types.empty')} />
      ) : (
        <div className={styles.columns}>
          <Panel
            title={mode === 'category' ? t('types.by_category') : t('types.by_extension')}
            actions={
              <span className={styles.panelMeta}>
                {visibleSummary.length} / {summary.length}
              </span>
            }
          >
            <table className={styles.table} aria-label={t('nav.types')} aria-busy={loading || undefined}>
              <thead>
                <tr>
                  <th scope="col">{t('types.extension')}</th>
                  <th scope="col" className={styles.numeric}>
                    {t('types.count')}
                  </th>
                  <th scope="col" className={styles.numeric}>
                    {t('types.size')}
                  </th>
                  <th scope="col" className={styles.numeric}>
                    {t('types.share')}
                  </th>
                </tr>
              </thead>
              <tbody>
                {visibleSummary.map((row) => {
                  const key = keyOf(row, mode);
                  const label =
                    mode === 'extension'
                      ? row.extension === ''
                        ? t('types.no_extension')
                        : `.${row.extension}`
                      : t(`treemap.category.${row.category}`);
                  const active = key === selectedType;

                  return (
                    <tr
                      key={key}
                      className={`${styles.row} ${active ? styles.rowSelected : ''}`}
                      aria-selected={active}
                      // The whole row is the selection target: the label button only
                      // covers a few pixels, so clicks on counts/size/bar must work too.
                      onClick={() => {
                        setSelectedType(key);
                      }}
                    >
                      <td>
                        <button
                          type="button"
                          className={styles.typeButton}
                          onClick={() => {
                            setSelectedType(key);
                          }}
                        >
                          {label}
                        </button>
                        <div className={styles.bar} aria-hidden="true">
                          <div
                            className={styles.barFill}
                            style={{ width: `${Math.max(1, row.share * 100)}%` }}
                          />
                        </div>
                      </td>
                      <td className={styles.numeric}>{formatNumber(row.count, language)}</td>
                      <td className={styles.numeric}>{formatBytes(row.logical_size, units)}</td>
                      <td className={styles.numeric}>{formatPercent(row.share, language)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </Panel>

          <Panel
            title={
              activeGroup === undefined
                ? t('nav.types')
                : mode === 'extension'
                  ? activeGroup.extension === ''
                    ? t('types.no_extension')
                    : `.${activeGroup.extension}`
                  : t(`treemap.category.${activeGroup.category}`)
            }
            actions={
              activeGroup === undefined ? undefined : (
                <span className={styles.panelMeta}>
                  {formatNumber(activeGroup.count, language)} ·{' '}
                  {formatBytes(activeGroup.logical_size, units)}
                </span>
              )
            }
          >
            <DataTable
              label={t('nav.types')}
              rows={rows}
              columns={columns}
              rowKey={(row) => row.id}
              sort={sort}
              onSortChange={setSort}
              loading={loading}
              height={420}
              empty={t('types.empty')}
            />
            {page?.next_cursor !== null && page?.next_cursor !== undefined ? (
              <div className={styles.loadMore}>
                <Button
                  variant="secondary"
                  loading={loading}
                  onClick={() => void loadFiles(page.next_cursor)}
                >
                  {t('tree.load_more')}
                </Button>
              </div>
            ) : null}
          </Panel>
        </div>
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
        onConfirm={(modeValue, extra) => void operation.confirm(modeValue, extra)}
      />
    </div>
  );
}

function keyOf(row: TypeSummary, mode: GroupMode): string {
  return mode === 'category' ? `category:${row.category}` : row.extension;
}
