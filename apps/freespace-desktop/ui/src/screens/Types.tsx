import { useCallback, useEffect, useMemo, useState } from 'react';

import { api } from '../api/runtime';
import type { FilterSet, NodeRow, SortKey, SortSpec, TreePage, TypeSummary, UiError } from '../api/dto';
import { normalizeError } from '../app/errors';
import { openInTree } from '../app/navigation';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Segmented } from '../components/Choice';
import { DataTable, type Column, type SortState } from '../components/DataTable';
import { EmptyState, ErrorState } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { emptyFilter } from '../lib/filters';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes, formatNumber, formatPercent } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './analysis.module.css';

type GroupMode = 'extension' | 'category';

/** Space distribution by extension or category, with a drill-down table. */
export function Types() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [mode, setMode] = useState<GroupMode>('extension');
  const [filter, setFilter] = useState<FilterSet>(emptyFilter());
  const [summary, setSummary] = useState<TypeSummary[]>([]);
  const [selectedType, setSelectedType] = useState<string | null>(null);
  const [rows, setRows] = useState<NodeRow[]>([]);
  const [page, setPage] = useState<TreePage | null>(null);
  const [sort, setSort] = useState<SortState>({ key: 'size', descending: true });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<number>>(new Set());
  const operation = useOperation();

  useEffect(() => {
    if (generation === null) return;
    let active = true;
    setLoading(true);
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
  }, [filter, generation, mode]);

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
      render: (row) => <span className={styles.pathCell} title={row.path}>{row.name}</span>,
    },
    {
      id: 'size',
      header: t('types.size'),
      width: '120px',
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
      width: '140px',
      render: (row) => (
        <Button variant="ghost" onClick={() => openInTree(row.id)}>
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
          <Button variant="primary" onClick={() => useShellStore.getState().setScreen('scan')}>
            {t('scan.start')}
          </Button>
        }
      />
    );
  }

  return (
    <div className={styles.stack}>
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

      <FilterBar screen="types" filter={filter} onChange={setFilter} />

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={<Button variant="primary" onClick={() => void loadFiles()}>{t('error.retry')}</Button>}
        />
      ) : null}

      <div className={local.columns}>
        <Card title={mode === 'category' ? t('types.by_category') : t('types.by_extension')}>
          <table className={local.table} aria-label={t('nav.types')} aria-busy={loading || undefined}>
            <thead>
              <tr>
                <th scope="col">{t('types.extension')}</th>
                <th scope="col" className={local.numeric}>{t('types.count')}</th>
                <th scope="col" className={local.numeric}>{t('types.size')}</th>
                <th scope="col" className={local.numeric}>{t('types.share')}</th>
              </tr>
            </thead>
            <tbody>
              {summary.map((row) => {
                const key = keyOf(row, mode);
                const label =
                  mode === 'extension'
                    ? row.extension === ''
                      ? t('types.no_extension')
                      : `.${row.extension}`
                    : t(`treemap.category.${row.category}`);
                return (
                  <tr key={key} aria-selected={key === selectedType}>
                    <td>
                      <button
                        type="button"
                        className={local.typeButton}
                        onClick={() => {
                          setSelectedType(key);
                        }}
                      >
                        {label}
                      </button>
                      <div className={local.bar} aria-hidden="true">
                        <div
                          className={local.barFill}
                          style={{ width: `${Math.max(1, row.share * 100)}%` }}
                        />
                      </div>
                    </td>
                    <td className={local.numeric}>{formatNumber(row.count, language)}</td>
                    <td className={local.numeric}>{formatBytes(row.logical_size, units)}</td>
                    <td className={local.numeric}>{formatPercent(row.share, language)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {summary.length === 0 && !loading ? (
            <EmptyState title={t('types.empty')} />
          ) : null}
        </Card>

        <Card
          title={
            activeGroup === undefined
              ? t('nav.types')
              : mode === 'extension'
                ? activeGroup.extension === ''
                  ? t('types.no_extension')
                  : `.${activeGroup.extension}`
                : t(`treemap.category.${activeGroup.category}`)
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
            <div className={styles.analyzeRow}>
              <Button variant="secondary" loading={loading} onClick={() => void loadFiles(page.next_cursor)}>
                {t('tree.load_more')}
              </Button>
            </div>
          ) : null}
        </Card>
      </div>

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
