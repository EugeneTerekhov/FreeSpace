import { useCallback, useEffect, useState } from 'react';

import { api } from '../api/runtime';
import type { NodeRow, SortKey, SortSpec, TreePage, UiError } from '../api/dto';
import { Breadcrumbs, SplitPane } from '../app/AppShell';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Badge } from '../components/Choice';
import { DataTable, type Column, type SortState } from '../components/DataTable';
import { DetailsPanel } from '../components/DetailsPanel';
import { EmptyState, ErrorState } from '../components/Feedback';
import { SearchField } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes, formatDate, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';

type Mode = 'browse' | 'search';

function toSortSpec(sort: SortState): SortSpec {
  return {
    key: sort.key as SortKey,
    descending: sort.descending,
  };
}

/** Virtualised tree with breadcrumbs, search and a details panel. */
export function Tree() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [rows, setRows] = useState<NodeRow[]>([]);
  const [page, setPage] = useState<TreePage | null>(null);
  const [trail, setTrail] = useState<NodeRow[]>([]);
  const [parentId, setParentId] = useState<number | null>(null);
  const [sort, setSort] = useState<SortState>({ key: 'size', descending: true });
  const [mode, setMode] = useState<Mode>('browse');
  const [query, setQuery] = useState('');
  const [activeQuery, setActiveQuery] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [selected, setSelected] = useState<NodeRow | null>(null);
  const [selectedPath, setSelectedPath] = useState<NodeRow[]>([]);
  const pendingNodeId = useShellStore((state) => state.pendingNodeId);

  const load = useCallback(
    async (parent: number | null, cursor?: string | null) => {
      if (generation === null) return;
      setLoading(true);
      setError(null);
      try {
        const sortSpec = toSortSpec(sort);
        const result =
          mode === 'search'
            ? await api.treeSearch(generation, activeQuery, {
                sort: sortSpec,
                pageSize: 500,
                cursor,
              })
            : parent === null
              ? await api.treeRoot(generation, { sort: sortSpec, pageSize: 500, cursor })
              : await api.treeChildren(generation, parent, {
                  sort: sortSpec,
                  pageSize: 500,
                  cursor,
                });

        setRows((current) => (cursor === undefined || cursor === null ? result.rows : [...current, ...result.rows]));
        setPage(result);
      } catch (cause) {
        setError(normalizeError(cause));
      } finally {
        setLoading(false);
      }
    },
    [activeQuery, generation, mode, sort],
  );

  useEffect(() => {
    if (generation === null) return;
    void load(mode === 'search' ? null : parentId);
  }, [generation, load, mode, parentId]);

  useEffect(() => {
    if (generation === null || pendingNodeId === null) return;
    let active = true;
    void Promise.all([
      api.treeNode(generation, pendingNodeId),
      api.treePath(generation, pendingNodeId),
    ])
      .then(([node, ancestors]) => {
        if (!active) return;
        setMode('browse');
        setActiveQuery('');
        setQuery('');
        setTrail(ancestors);
        setParentId(node.parent_id);
        setSelected(node);
        setSelectedPath(ancestors);
        useShellStore.getState().setPendingNode(null);
      })
      .catch((cause) => {
        if (active) setError(normalizeError(cause));
      });

    return () => {
      active = false;
    };
  }, [generation, pendingNodeId]);

  const open = async (row: NodeRow) => {
    if (generation === null) return;
    if (row.kind !== 'directory') return;

    try {
      const ancestors = await api.treePath(generation, row.id);
      setTrail([...ancestors, row]);
      setParentId(row.id);
      setMode('browse');
      setActiveQuery('');
      setQuery('');
    } catch (cause) {
      setError(normalizeError(cause));
    }
  };

  const up = () => {
    if (trail.length === 0) return;
    const nextTrail = trail.slice(0, -1);
    const parent = nextTrail.at(-1);
    setTrail(nextTrail);
    setParentId(parent?.id ?? null);
    setMode('browse');
  };

  const breadcrumbs = [
    {
      label: t('nav.tree'),
      onClick: () => {
        setTrail([]);
        setParentId(null);
        setMode('browse');
      },
    },
    ...trail.map((node, index) => ({
      label: node.name,
      onClick:
        index === trail.length - 1
          ? undefined
          : () => {
              const nextTrail = trail.slice(0, index + 1);
              setTrail(nextTrail);
              setParentId(node.id);
              setMode('browse');
            },
    })),
    ...(mode === 'search' ? [{ label: t('tree.search_results') }] : []),
  ];

  const columns: Column<NodeRow>[] = [
    {
      id: 'name',
      header: t('nav.tree'),
      width: 'minmax(240px, 2fr)',
      sortable: true,
      render: (row) => (
        <span className={styles.pathCell}>
          <span className={styles.kindIcon} aria-hidden="true">
            {row.kind === 'directory' ? '▸' : '·'}
          </span>
          <span>{row.name}</span>
          {row.accessible ? null : (
            <Badge tone="warning">{t('tree.access_denied')}</Badge>
          )}
        </span>
      ),
    },
    {
      id: 'size',
      header: t('details.size'),
      width: '120px',
      align: 'end',
      sortable: true,
      render: (row) => formatBytes(row.on_disk_size, units),
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
      id: 'modified',
      header: t('details.modified'),
      width: '180px',
      sortable: true,
      render: (row) => formatDate(row.modified_at, language),
    },
  ];

  if (generation === null) {
    return (
      <EmptyState
        title={t('tree.empty')}
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

  if (error !== null && rows.length === 0) {
    return (
      <ErrorState
        title={t(error.message_key)}
        code={error.code}
        details={error.details ?? undefined}
        actions={
          <Button variant="primary" onClick={() => void load(parentId)}>
            {t('error.retry')}
          </Button>
        }
      />
    );
  }

  return (
    <div className={styles.treeLayout}>
      <div className={styles.toolbar}>
        <Button variant="ghost" disabled={trail.length === 0} onClick={up}>
          ↑ {t('tree.up')}
        </Button>
        <Breadcrumbs items={breadcrumbs} />
        <span className={styles.spacer} />
        <SearchField
          className={styles.search}
          label={t('tree.search')}
          placeholder={t('tree.search_placeholder')}
          value={query}
          loading={loading}
          onValueChange={setQuery}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              setMode('search');
              setActiveQuery(query);
              setTrail([]);
              setParentId(null);
            }
          }}
        />
        <Button
          variant="secondary"
          onClick={() => {
            setMode('search');
            setActiveQuery(query);
            setTrail([]);
            setParentId(null);
          }}
        >
          {t('tree.search')}
        </Button>
      </div>

      {error !== null ? (
        <ErrorState title={t(error.message_key)} code={error.code} details={error.details ?? undefined} />
      ) : null}

      <SplitPane
        left={
          <div className={styles.tableWrap}>
            <DataTable
              label={t('nav.tree')}
              rows={rows}
              columns={columns}
              rowKey={(row) => row.id}
              selectedKey={selected?.id ?? null}
              sort={sort}
              onSortChange={setSort}
              loading={loading}
              height={520}
              tree={{
                level: () => Math.max(1, trail.length + 1),
                hasChildren: (row) => row.kind === 'directory' && row.child_count > 0,
                isExpanded: () => false,
                onExpand: (row) => {
                  void open(row);
                },
                onCollapse: () => {
                  up();
                },
              }}
              onSelect={(row) => {
                setSelected(row);
                if (generation !== null) {
                  void api
                    .treePath(generation, row.id)
                    .then(setSelectedPath)
                    .catch(() => setSelectedPath([]));
                }
              }}
              onActivate={(row) => {
                void open(row);
                if (row.kind !== 'directory') setSelected(row);
              }}
              empty={
                mode === 'search' ? t('tree.no_matches') : t('tree.empty')
              }
            />
            {page?.next_cursor !== null && page?.next_cursor !== undefined ? (
              <div className={styles.analyzeRow}>
                <Button
                  variant="secondary"
                  loading={loading}
                  onClick={() => {
                    void load(parentId, page.next_cursor);
                  }}
                >
                  {t('tree.load_more')}
                </Button>
              </div>
            ) : null}
          </div>
        }
        right={
          <div className={styles.aside}>
            <DetailsPanel node={selected} ancestors={selectedPath} />
          </div>
        }
      />
    </div>
  );
}
