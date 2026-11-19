import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Copy, FolderOpen, PenLine, Trash2 } from 'lucide-react';

import { api } from '../api/runtime';
import type {
  AttributeFlags,
  FilterSet,
  NodeRow,
  SizeDetailsView,
  SortKey,
  SortSpec,
  TreePage,
  UiError,
} from '../api/dto';
import { fetchSizeDetails } from '../api/filesystem';
import { Breadcrumbs } from '../app/AppShell';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Badge } from '../components/Choice';
import { ContextMenu, type ContextMenuItem } from '../components/ContextMenu';
import { DeleteObjectDialog } from '../components/DeleteObjectDialog';
import {
  DataTable,
  type Column,
  type RowAnchor,
  type SortState,
} from '../components/DataTable';
import { ExportAction } from '../components/ExportAction';
import { Banner, EmptyState, ErrorState, Loading } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { SearchField } from '../components/Fields';
import {
  HoverCard,
  HoverCardHead,
  HoverCardMetric,
  HoverCardMetrics,
  HoverCardNote,
  HoverCardPath,
} from '../components/HoverCard';
import { OperationProgressView } from '../components/OperationProgress';
import { RenameDialog, type RenamedObject } from '../components/RenameDialog';
import { TreeRowLabel, TreeRowShareBar, isVolumeRow, sharePercent } from '../components/TreeRow';
import { useOperation } from '../operations/useOperation';
import { t } from '../i18n';
import { formatBytes, formatDateCompact, formatDateTimeCompact, formatNumber } from '../i18n/format';
import { copyText } from '../lib/clipboard';
import { emptyFilter } from '../lib/filters';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Screen } from './ScreenFrame';
import styles from './tree.module.css';

type Mode = 'browse' | 'search';

/** Shown wherever the core has no value for a field; never a made-up number. */
const MISSING = '—';

/** Wait before the size card appears, so passing over rows does not flash it. */
const HOVER_DELAY = 450;

interface RowMenu {
  node: NodeRow;
  x: number;
  y: number;
}

interface TreeNotice {
  tone: 'info' | 'error';
  text: string;
}

/**
 * True when a successful operation removed this path (or an ancestor) from the
 * immutable scan tree.
 */
function isHiddenPath(path: string, hidden: ReadonlySet<string>): boolean {
  if (hidden.size === 0) return false;
  const lower = path.toLowerCase();
  for (const prefix of hidden) {
    if (lower === prefix || lower.startsWith(`${prefix}\\`)) return true;
  }

  return false;
}

function toSortSpec(sort: SortState): SortSpec {
  return {
    key: sort.key as SortKey,
    descending: sort.descending,
  };
}

/**
 * Tree: the hierarchy of a snapshot with inline share bars and a hover card
 * with the size details of the row under the pointer (plan.md, «UX-перекомпоновка»).
 *
 * The list is the existing virtualised `DataTable`; each row paints its share of
 * the parent behind the cells. Clicking a row selects it, and the details are
 * looked up only after the pointer or keyboard focus rests on a row.
 */
export function Tree() {
  const generation = useSnapshotStore((state) => state.generation);
  const volume = useSnapshotStore((state) => state.volume);
  const hiddenPaths = useSnapshotStore((state) => state.hiddenPaths);
  const hidePath = useSnapshotStore((state) => state.hidePath);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [rows, setRows] = useState<NodeRow[]>([]);
  const [page, setPage] = useState<TreePage | null>(null);
  const [trail, setTrail] = useState<NodeRow[]>([]);
  const [parentId, setParentId] = useState<number | null>(null);
  const [sort, setSort] = useState<SortState>({ key: 'size', descending: true });
  const [filter, setFilter] = useState<FilterSet>(emptyFilter);
  const [mode, setMode] = useState<Mode>('browse');
  const [query, setQuery] = useState('');
  const [activeQuery, setActiveQuery] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [selected, setSelected] = useState<NodeRow | null>(null);
  const [hoverRow, setHoverRow] = useState<{ node: NodeRow; anchor: RowAnchor } | null>(
    null,
  );
  const [details, setDetails] = useState<SizeDetailsView | null>(null);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<number>>(() => new Set());
  const [childrenByParent, setChildrenByParent] = useState<Record<number, NodeRow[]>>(
    {},
  );
  const pendingNodeId = useShellStore((state) => state.pendingNodeId);
  const [menu, setMenu] = useState<RowMenu | null>(null);
  const [renameTarget, setRenameTarget] = useState<NodeRow | null>(null);
  /** Row whose deletion is being previewed or executed. */
  const [deleteTarget, setDeleteTarget] = useState<NodeRow | null>(null);
  const [notice, setNotice] = useState<TreeNotice | null>(null);
  const operation = useOperation();
  // Guards the async path lookup so a slow answer for an older row cannot
  // overwrite the trail of the row selected last.
  const selectedId = useRef<number | null>(null);
  // One size answer per node: returning to a row reuses it instead of asking
  // again. A newer hover invalidates the answer of an older one.
  const detailsCache = useRef(new Map<number, SizeDetailsView>());
  const detailsRequest = useRef(0);
  const hoverTimer = useRef<number | null>(null);

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
                filter,
                pageSize: 500,
                cursor,
              })
            : parent === null
              ? await api.treeRoot(generation, { sort: sortSpec, pageSize: 500, cursor })
              : await api.treeChildren(generation, parent, {
                  sort: sortSpec,
                  filter,
                  pageSize: 500,
                  cursor,
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
    [activeQuery, filter, generation, mode, sort],
  );

  useEffect(() => {
    if (generation === null) return;
    void load(mode === 'search' ? null : parentId);
  }, [generation, load, mode, parentId]);

  const closeHover = useCallback(() => {
    if (hoverTimer.current !== null) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = null;
    }
    detailsRequest.current += 1;
    setHoverRow(null);
    setDetails(null);
    setDetailsLoading(false);
  }, []);

  // Expanded branches belong to one listing; a new directory, sort, query or
  // filter starts collapsed again and drops the size card of the old rows.
  useEffect(() => {
    setExpanded(new Set());
    setChildrenByParent({});
    setNotice(null);
    closeHover();
  }, [closeHover, generation, parentId, mode, activeQuery, sort.key, sort.descending, filter]);

  const expand = useCallback(
    async (row: NodeRow) => {
      if (generation === null || expanded.has(row.id)) return;
      const cached = childrenByParent[row.id];
      if (cached !== undefined) {
        setExpanded((current) => new Set(current).add(row.id));
        return;
      }
      try {
        const result = await api.treeChildren(generation, row.id, {
          sort: toSortSpec(sort),
          filter,
          pageSize: 500,
        });
        setChildrenByParent((current) => ({ ...current, [row.id]: result.rows }));
        setExpanded((current) => new Set(current).add(row.id));
      } catch (cause) {
        setError(normalizeError(cause));
      }
    },
    [childrenByParent, expanded, filter, generation, sort],
  );

  const collapse = useCallback((row: NodeRow) => {
    setExpanded((current) => {
      if (!current.has(row.id)) return current;
      const next = new Set(current);
      next.delete(row.id);
      return next;
    });
  }, []);

  /**
   * Starts the delay for one row. The card opens on the row data the scan
   * already has and is enriched by `size_details` when that answer arrives; a
   * cached answer skips the request entirely.
   */
  const openHover = useCallback(
    (node: NodeRow, anchor: RowAnchor) => {
      if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
      hoverTimer.current = window.setTimeout(() => {
        hoverTimer.current = null;
        detailsRequest.current += 1;
        const request = detailsRequest.current;
        setHoverRow({ node, anchor });

        const cached = detailsCache.current.get(node.id);
        if (cached !== undefined) {
          setDetails(cached);
          setDetailsLoading(false);
          return;
        }

        setDetails(null);
        if (generation === null) {
          setDetailsLoading(false);
          return;
        }

        setDetailsLoading(true);
        void fetchSizeDetails(generation, node.id)
          .then((value) => {
            detailsCache.current.set(node.id, value);
            if (detailsRequest.current === request) setDetails(value);
          })
          .catch(() => {
            if (detailsRequest.current === request) setDetails(null);
          })
          .finally(() => {
            if (detailsRequest.current === request) setDetailsLoading(false);
          });
      }, HOVER_DELAY);
    },
    [generation],
  );

  useEffect(
    () => () => {
      if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
    },
    [],
  );

  // A card that stays put while the row scrolls away would point at nothing.
  useEffect(() => {
    if (hoverRow === null) return undefined;
    const hide = () => {
      closeHover();
    };
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

  const visible = useMemo(() => {
    const levels = new Map<number, number>();
    const flattened: NodeRow[] = [];
    const walk = (list: NodeRow[], level: number) => {
      for (const row of list) {
        if (isHiddenPath(row.path, hiddenPaths)) continue;
        flattened.push(row);
        levels.set(row.id, level);
        if (expanded.has(row.id)) {
          const children = childrenByParent[row.id];
          if (children !== undefined) walk(children, level + 1);
        }
      }
    };
    walk(rows, 1);

    return { rows: flattened, levels };
  }, [childrenByParent, expanded, hiddenPaths, rows]);

  /** Every node the screen knows about, so a child can find its parent size. */
  const nodesById = useMemo(() => {
    const map = new Map<number, NodeRow>();
    for (const node of trail) map.set(node.id, node);
    for (const node of rows) map.set(node.id, node);
    for (const list of Object.values(childrenByParent)) {
      for (const node of list) map.set(node.id, node);
    }

    return map;
  }, [childrenByParent, rows, trail]);

  /** The whole scope: the volume row when known, otherwise the volume itself. */
  const totalSize = useMemo(() => {
    const root = rows.find((row) => row.parent_id === null) ?? trail[0];
    if (root !== undefined) return Math.max(0, root.on_disk_size);

    return Math.max(0, volume?.used_bytes ?? 0);
  }, [rows, trail, volume]);

  const parentSizeOf = useCallback(
    (row: NodeRow): number => {
      if (row.parent_id === null) return totalSize;
      const parent = nodesById.get(row.parent_id);

      return parent === undefined ? totalSize : Math.max(0, parent.on_disk_size);
    },
    [nodesById, totalSize],
  );

  useEffect(() => {
    if (generation === null || pendingNodeId === null) return;
    let active = true;
    void Promise.all([
      api.treeNode(generation, pendingNodeId),
      api.treePath(generation, pendingNodeId),
    ])
      .then(([node, ancestors]) => {
        if (!active) return;
        selectedId.current = node.id;
        setMode('browse');
        setActiveQuery('');
        setQuery('');
        setTrail(ancestors);
        setParentId(node.parent_id);
        setSelected(node);
        useShellStore.getState().setPendingNode(null);
      })
      .catch((cause) => {
        if (active) setError(normalizeError(cause));
      });

    return () => {
      active = false;
    };
  }, [generation, pendingNodeId]);

  const selectRow = useCallback((row: NodeRow) => {
    selectedId.current = row.id;
    setSelected(row);
  }, []);

  const openMenu = useCallback(
    (row: NodeRow, anchor: RowAnchor) => {
      closeHover();
      setNotice(null);
      setMenu({ node: row, x: anchor.x, y: anchor.y });
    },
    [closeHover],
  );

  const revealInExplorer = useCallback(async (row: NodeRow) => {
    try {
      await api.revealInExplorer(row.path);
    } catch (cause) {
      const normalized = normalizeError(cause);
      setNotice({ tone: 'error', text: t(normalized.message_key) });
    }
  }, []);

  const copyPath = useCallback(async (row: NodeRow) => {
    try {
      await copyText(row.path);
      setNotice({ tone: 'info', text: t('tree2.copied') });
    } catch {
      setNotice({ tone: 'error', text: t('tree2.copy_failed') });
    }
  }, []);

  const startDelete = useCallback(
    (row: NodeRow) => {
      setNotice(null);
      setDeleteTarget(row);
      void operation.prepare({
        kind: 'delete',
        targets: [row.path],
        delete_mode: 'recycle_bin',
        reason: 'tree context menu',
      });
    },
    [operation],
  );

  /**
   * Drops a deleted node and every expanded branch below it, and subtracts its
   * totals from the loaded ancestors so the tree stays consistent until the
   * next scan.
   */
  const removeSubtree = useCallback(
    (root: NodeRow) => {
      const removed = new Set<number>();
      const walk = (id: number) => {
        if (removed.has(id)) return;
        removed.add(id);
        for (const child of childrenByParent[id] ?? []) walk(child.id);
      };
      walk(root.id);

      const adjusted = new Map<number, NodeRow>();
      let parentId = root.parent_id;
      while (parentId !== null) {
        const parent = nodesById.get(parentId);
        if (parent === undefined) break;
        adjusted.set(parent.id, {
          ...parent,
          on_disk_size: Math.max(0, parent.on_disk_size - root.on_disk_size),
          logical_size: Math.max(0, parent.logical_size - root.logical_size),
          physical_size: Math.max(0, parent.physical_size - root.physical_size),
          ads_size: Math.max(0, parent.ads_size - root.ads_size),
          file_count: Math.max(0, parent.file_count - root.file_count),
          // Only the direct parent loses one child; higher levels keep theirs.
          child_count: Math.max(
            0,
            parent.child_count - (parent.id === root.parent_id ? 1 : 0),
          ),
        });
        parentId = parent.parent_id;
      }

      const patch = (row: NodeRow): NodeRow | null =>
        removed.has(row.id) ? null : (adjusted.get(row.id) ?? row);
      const patchList = (list: NodeRow[]): NodeRow[] =>
        list.flatMap((row) => {
          const next = patch(row);
          return next === null ? [] : [next];
        });

      setRows(patchList);
      setTrail(patchList);
      setChildrenByParent((current) => {
        const next: Record<number, NodeRow[]> = {};
        for (const [key, list] of Object.entries(current)) {
          const id = Number(key);
          if (removed.has(id)) continue;
          next[id] = patchList(list);
        }
        return next;
      });
      setExpanded((current) => {
        const next = new Set<number>();
        for (const id of current) {
          if (!removed.has(id)) next.add(id);
        }
        return next;
      });
      setSelected((current) => {
        if (current === null) return null;
        return patch(current);
      });
    },
    [childrenByParent, nodesById],
  );

  /** Applies a finished rename to the loaded rows until the next scan. */
  const applyRenamed = useCallback(
    (result: RenamedObject) => {
      const name = result.newName;
      const oldLower = result.oldPath.toLowerCase();
      const patch = (row: NodeRow): NodeRow => {
        if (row.path.toLowerCase() === oldLower) {
          return { ...row, name, path: result.newPath };
        }
        // Children of a renamed directory keep their names but move with it.
        return row.path.toLowerCase().startsWith(`${oldLower}\\`)
          ? {
              ...row,
              path: `${result.newPath}${row.path.slice(result.oldPath.length)}`,
            }
          : row;
      };
      setRows((current) => current.map(patch));
      setTrail((current) => current.map(patch));
      setChildrenByParent((current) => {
        const next: Record<number, NodeRow[]> = {};
        for (const [key, list] of Object.entries(current)) {
          next[Number(key)] = list.map(patch);
        }
        return next;
      });
      setSelected((current) => (current === null ? null : patch(current)));
      closeHover();
      setNotice({ tone: 'info', text: t('tree2.renamed', { name }) });
    },
    [closeHover],
  );

  // The tree keeps its rows after an operation; a completed recycle removes the
  // branch locally so the list never shows an object that is already gone.
  useEffect(() => {
    const progress = operation.progress;
    if (progress === null || deleteTarget === null) return;
    if (progress.state === 'completed') {
      // Keep the object hidden after leaving and re-entering the screen: the
      // scan snapshot is immutable and would otherwise return the row.
      hidePath(deleteTarget.path);
      removeSubtree(deleteTarget);
      setNotice({
        tone: 'info',
        text: t('tree2.deleted', { name: deleteTarget.name }),
      });
      setDeleteTarget(null);
      operation.close();
      return;
    }
    if (
      progress.state === 'failed' ||
      progress.state === 'partial' ||
      progress.state === 'cancelled'
    ) {
      const firstError = progress.errors[0];
      setNotice({
        tone: progress.state === 'cancelled' ? 'info' : 'error',
        text:
          firstError !== undefined
            ? t(firstError.message_key)
            : t(
                progress.state === 'cancelled'
                  ? 'tree2.delete_cancelled'
                  : 'tree2.delete_failed',
              ),
      });
      setDeleteTarget(null);
      operation.close();
    }
  }, [deleteTarget, hidePath, operation, removeSubtree]);

  const open = async (row: NodeRow) => {
    if (generation === null || row.kind !== 'directory') return;

    try {
      const ancestors = await api.treePath(generation, row.id);
      selectedId.current = row.id;
      setTrail([...ancestors, row]);
      setParentId(row.id);
      setMode('browse');
      setActiveQuery('');
      setQuery('');
      setSelected(row);
    } catch (cause) {
      setError(normalizeError(cause));
    }
  };

  const goToTrail = (index: number) => {
    const nextTrail = trail.slice(0, index + 1);
    const target = nextTrail[index] ?? null;
    selectedId.current = target?.id ?? null;
    setTrail(nextTrail);
    setParentId(target === null ? null : target.id);
    setMode('browse');
    setActiveQuery('');
    setQuery('');
    setSelected(target);
  };

  const up = () => {
    if (trail.length === 0) return;
    const nextTrail = trail.slice(0, -1);
    const target = nextTrail.at(-1) ?? null;
    selectedId.current = target?.id ?? null;
    setTrail(nextTrail);
    setParentId(target === null ? null : target.id);
    setMode('browse');
    setActiveQuery('');
    setQuery('');
    setSelected(target);
  };

  const goRoot = () => {
    selectedId.current = null;
    setTrail([]);
    setParentId(null);
    setMode('browse');
    setActiveQuery('');
    setQuery('');
    setSelected(null);
  };

  const runSearch = () => {
    const needle = query.trim();
    if (needle === '') return;
    selectedId.current = null;
    setMode('search');
    setActiveQuery(needle);
    setTrail([]);
    setParentId(null);
    setSelected(null);
  };

  const breadcrumbs = [
    {
      label: t('nav.tree'),
      onClick: goRoot,
    },
    ...trail.map((node, index) => ({
      label: node.name,
      onClick: index === trail.length - 1 ? undefined : () => goToTrail(index),
    })),
    ...(mode === 'search' ? [{ label: t('tree.search_results') }] : []),
  ];

  const columns: Column<NodeRow>[] = [
    {
      id: 'name',
      header: t('tree.sort.name'),
      width: 'minmax(240px, 2fr)',
      sortable: true,
      render: (row) => (
        <TreeRowLabel
          row={row}
          expanded={expanded.has(row.id)}
          onToggle={(target) => {
            if (expanded.has(target.id)) {
              collapse(target);
            } else {
              void expand(target);
            }
          }}
        />
      ),
    },
    {
      id: 'size',
      header: t('tree.sort.size'),
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
      render: (row) => formatDateCompact(row.modified_at),
    },
  ];

  if (generation === null) {
    return (
      <EmptyState
        title={t('scan.required')}
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

  const scope =
    mode === 'search'
      ? t('tree.search_results')
      : trail.at(-1)?.path ?? volume?.mount_point ?? t('nav.tree');
  const nextCursor = page?.next_cursor ?? null;
  const total = page?.total_estimate ?? visible.rows.length;
  const hasInaccessible = visible.rows.some((row) => !row.accessible);
  const emptyLabel = mode === 'search' ? t('tree.no_matches') : t('tree.empty');

  // A volume root is the device itself: neither its name nor its contents can
  // be renamed or recycled from here.
  const menuItems: ContextMenuItem[] =
    menu === null
      ? []
      : [
          {
            id: 'open',
            label: t('tree2.action.open'),
            icon: <FolderOpen size={15} strokeWidth={1.7} />,
            onSelect: () => {
              void revealInExplorer(menu.node);
            },
          },
          {
            id: 'copy',
            label: t('tree2.action.copy_path'),
            icon: <Copy size={15} strokeWidth={1.7} />,
            onSelect: () => {
              void copyPath(menu.node);
            },
          },
          {
            id: 'rename',
            label: t('tree2.action.rename'),
            icon: <PenLine size={15} strokeWidth={1.7} />,
            disabled: isVolumeRow(menu.node),
            onSelect: () => {
              setRenameTarget(menu.node);
            },
          },
          {
            id: 'delete',
            label: t('tree2.action.delete'),
            icon: <Trash2 size={15} strokeWidth={1.7} />,
            danger: true,
            disabled: isVolumeRow(menu.node),
            onSelect: () => {
              startDelete(menu.node);
            },
          },
        ];

  return (
    <Screen
      context={t('tree2.context', {
        scope,
        count: formatNumber(total, language),
      })}
      actions={
        <>
          <form
            className={styles.searchForm}
            onSubmit={(event) => {
              event.preventDefault();
              runSearch();
            }}
            onKeyDown={(event) => {
              // The first Escape clears the written text (SearchField); a second
              // one on an empty field leaves the search results.
              if (event.key !== 'Escape' || mode !== 'search' || query !== '') return;
              goRoot();
            }}
          >
            <SearchField
              className={styles.search}
              label={t('tree.search')}
              placeholder={t('tree.search_placeholder')}
              value={query}
              loading={loading}
              onValueChange={setQuery}
            />
            <Button type="submit" variant="secondary">
              {t('tree.search')}
            </Button>
          </form>
          <ExportAction sections={['tree']} />
        </>
      }
    >
      {error !== null ? (
        <Banner
          tone="error"
          action={
            <Button variant="ghost" onClick={() => void load(parentId)}>
              {t('error.retry')}
            </Button>
          }
        >
          {t(error.message_key)}
        </Banner>
      ) : null}

      {notice !== null ? <Banner tone={notice.tone}>{notice.text}</Banner> : null}

      {operation.error !== null && operation.preview === null ? (
        <Banner
          tone="error"
          action={
            <Button
              variant="ghost"
              onClick={() => {
                operation.close();
                setDeleteTarget(null);
              }}
            >
              {t('common.close')}
            </Button>
          }
        >
          {t(operation.error.message_key)}
        </Banner>
      ) : null}

      <Panel hint={t('tree2.hover_hint')}>
        <div className={styles.toolbar}>
          <Breadcrumbs items={breadcrumbs} />
        </div>

        <FilterBar
          screen="tree"
          filter={filter}
          disabled={loading}
          onChange={setFilter}
        />

        {hasInaccessible ? <Banner tone="warning">{t('tree2.partial')}</Banner> : null}

        <div className={styles.table}>
          <DataTable
            label={t('nav.tree')}
            rows={visible.rows}
            columns={columns}
            rowKey={(row) => row.id}
            selectedKey={selected === null ? null : selected.id}
            showSelectionMark={false}
            sort={sort}
            onSortChange={setSort}
            loading={loading}
            height={520}
            rowBackground={(row) => (
              <TreeRowShareBar
                level={visible.levels.get(row.id) ?? 1}
                share={sharePercent(row.on_disk_size, parentSizeOf(row))}
              />
            )}
            tree={{
              level: (row) => visible.levels.get(row.id) ?? 1,
              hasChildren: (row) => row.kind === 'directory' && row.child_count > 0,
              isExpanded: (row) => expanded.has(row.id),
              onExpand: (row) => {
                void expand(row);
              },
              onCollapse: (row) => {
                if (expanded.has(row.id)) {
                  collapse(row);
                } else {
                  up();
                }
              },
            }}
            onRowHoverStart={openHover}
            onRowHoverEnd={closeHover}
            onRowContextMenu={openMenu}
            onSelect={selectRow}
            onActivate={(row) => {
              if (row.kind === 'directory') {
                void open(row);
              } else {
                selectRow(row);
              }
            }}
            empty={loading ? <Loading label={t('common.loading')} rows={5} /> : emptyLabel}
          />
        </div>

        {nextCursor === null ? null : (
          <div className={styles.footer}>
            <Button
              variant="secondary"
              loading={loading}
              onClick={() => {
                void load(parentId, nextCursor);
              }}
            >
              {t('tree.load_more')}
            </Button>
          </div>
        )}
      </Panel>

      {hoverRow === null ? null : (
        <HoverCard
          anchor={hoverRow.anchor}
          label={t('tree2.title_details')}
          busy={detailsLoading}
        >
          <SizeDetailsContent
            node={hoverRow.node}
            share={sharePercent(hoverRow.node.on_disk_size, parentSizeOf(hoverRow.node))}
            details={details}
            detailsLoading={detailsLoading}
          />
        </HoverCard>
      )}

      <ContextMenu
        open={menu !== null}
        x={menu?.x ?? 0}
        y={menu?.y ?? 0}
        label={
          menu === null
            ? t('tree2.actions')
            : `${t('tree2.actions')}: ${menu.node.name}`
        }
        items={menuItems}
        onClose={() => {
          setMenu(null);
        }}
      />

      <RenameDialog
        open={renameTarget !== null}
        path={renameTarget?.path ?? null}
        name={renameTarget?.name ?? null}
        onClose={() => {
          setRenameTarget(null);
        }}
        onRenamed={applyRenamed}
      />

      <DeleteObjectDialog
        open={operation.preview !== null && deleteTarget !== null}
        target={deleteTarget}
        size={operation.preview?.targets[0]?.size ?? null}
        busy={operation.busy}
        error={operation.error}
        onCancel={() => {
          setDeleteTarget(null);
          operation.close();
        }}
        onConfirm={() => {
          // The tree menu only ever recycles.
          void operation.confirm('recycle_bin', false);
        }}
      />

      {operation.progress !== null ? (
        <OperationProgressView
          progress={operation.progress}
          onCancel={() => void operation.cancel()}
          onUndo={() => void operation.undo()}
        />
      ) : null}
    </Screen>
  );
}

/** Value of an NTFS flag, or an em dash when the core has no attributes. */
function flagValue(attributes: AttributeFlags | undefined, key: 'compressed' | 'sparse'): string {
  const value = attributes?.[key];
  if (value === undefined) return MISSING;

  return value ? t('tree2.yes') : t('tree2.no');
}

/**
 * Content of the tree row hover card: path, logical and on-disk size, item and
 * file counts, hard links, ADS, the NTFS flags, the savings and the timestamp.
 *
 * The scan data paints the card immediately; `size_details` fills the fields the
 * row does not carry. A field without data — for example the hard links before
 * the answer arrives — shows an em dash rather than a guess.
 */
function SizeDetailsContent({
  node,
  share,
  details,
  detailsLoading,
}: {
  node: NodeRow;
  /** Whole percent of the parent on-disk size. */
  share: number;
  details: SizeDetailsView | null;
  detailsLoading: boolean;
}) {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const attributes = details?.attributes ?? node.attributes;
  const volume = isVolumeRow(node);
  // A drive root is the volume itself: its hidden/system bits describe the
  // device, not a hidden object, and the OS lists the volume anyway.
  const flags = volume
    ? { hidden: false, system: false, reparse: false }
    : {
        hidden: attributes.hidden,
        system: attributes.system,
        reparse: attributes.reparse,
      };
  const savings =
    details === null
      ? MISSING
      : formatBytes(details.compression_savings + details.sparse_savings, units);

  return (
    <>
      <HoverCardHead
        title={node.name}
        meta={volume ? t('kind.volume') : t(`kind.${node.kind}`)}
      />
      <HoverCardPath title={node.path}>{node.path}</HoverCardPath>
      <HoverCardMetrics>
        <HoverCardMetric
          label={t('details.logical')}
          value={formatBytes(details?.logical_size ?? node.logical_size, units)}
        />
        <HoverCardMetric
          label={t('details.size')}
          value={formatBytes(details?.on_disk_size ?? node.on_disk_size, units)}
        />
        <HoverCardMetric
          label={t('details.physical')}
          value={formatBytes(details?.physical_size ?? node.physical_size, units)}
        />
        <HoverCardMetric
          label={t('tree2.share')}
          value={`${formatNumber(share, language)} %`}
        />
        <HoverCardMetric
          label={t('details.items')}
          value={formatNumber(node.child_count, language)}
        />
        <HoverCardMetric
          label={t('tree2.files')}
          value={formatNumber(node.file_count, language)}
        />
        <HoverCardMetric
          label={t('tree2.hardlinks')}
          value={
            details === null ? MISSING : formatBytes(details.hardlink_contribution, units)
          }
        />
        <HoverCardMetric
          label={t('tree2.ads')}
          value={formatBytes(details?.ads_size ?? node.ads_size, units)}
        />
        <HoverCardMetric
          label={t('tree2.compression')}
          value={flagValue(attributes, 'compressed')}
        />
        <HoverCardMetric
          label={t('tree2.sparse')}
          value={flagValue(attributes, 'sparse')}
        />
        <HoverCardMetric label={t('tree2.savings')} value={savings} />
        <HoverCardMetric
          label={t('details.modified')}
          value={formatDateTimeCompact(node.modified_at, language)}
        />
      </HoverCardMetrics>
      {node.link_target === null ? null : (
        <p className={styles.hoverTarget}>
          <span>{t('details.reparse')}</span>
          <span className={styles.hoverTargetPath}>{node.link_target}</span>
        </p>
      )}
      {flags.hidden || flags.system || flags.reparse || !node.accessible ? (
        <div className={styles.badges}>
          {!node.accessible ? (
            <Badge tone="warning">{t('tree.access_denied')}</Badge>
          ) : null}
          {flags.hidden ? <Badge>{t('attr.hidden')}</Badge> : null}
          {flags.system ? <Badge>{t('attr.system')}</Badge> : null}
          {flags.reparse ? <Badge>{t('attr.reparse')}</Badge> : null}
        </div>
      ) : null}
      {details?.estimated === true ? (
        <HoverCardNote>{t('tree2.estimated')}</HoverCardNote>
      ) : null}
      {detailsLoading || details !== null ? null : (
        <HoverCardNote>{t('tree2.details_fallback')}</HoverCardNote>
      )}
    </>
  );
}
