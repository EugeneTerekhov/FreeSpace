import { useEffect, useRef, useState } from 'react';

import { api } from '../api/runtime';
import type { NodeRow, TreemapTile, UiError } from '../api/dto';
import { Breadcrumbs, SplitPane } from '../app/AppShell';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { DetailsPanel } from '../components/DetailsPanel';
import { EmptyState, ErrorState, Loading } from '../components/Feedback';
import { t } from '../i18n';
import { formatBytes } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { TreemapCanvas } from '../treemap/TreemapCanvas';
import { treemapLegend } from '../treemap/palette';
import styles from './screens.module.css';

const LEGEND = treemapLegend();

/** Folder colour has no category key: it is a node kind, not a file type. */
function categoryLabel(key: string): string {
  return key === 'folder' ? t('kind.directory') : t(`treemap.category.${key}`);
}

/** Canvas Treemap with drill-down, breadcrumbs and details. */
export function Treemap() {
  const generation = useSnapshotStore((state) => state.generation);
  const volume = useSnapshotStore((state) => state.volume);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const [parentId, setParentId] = useState<number | null>(null);
  const [trail, setTrail] = useState<NodeRow[]>([]);
  const [tiles, setTiles] = useState<TreemapTile[]>([]);
  const [truncated, setTruncated] = useState(false);
  const [selected, setSelected] = useState<NodeRow | null>(null);
  const [selectedPath, setSelectedPath] = useState<NodeRow[]>([]);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [error, setError] = useState<UiError | null>(null);
  const [loading, setLoading] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = wrap.current;
    if (element === null) return undefined;

    const observer = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      if (rect !== undefined) setSize({ width: rect.width, height: rect.height });
    });
    observer.observe(element);
    setSize({ width: element.clientWidth, height: element.clientHeight });

    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    setParentId(null);
    setTrail([]);
    setSelected(null);
    setTiles([]);
  }, [generation]);

  useEffect(() => {
    if (generation === null) return undefined;

    let active = true;
    const timer = setTimeout(() => {
      setLoading(true);
      setError(null);
      void api
        .treemapTiles(generation, {
          node_id: parentId,
          width: Math.max(1, Math.floor(size.width > 0 ? size.width : 800)),
          height: Math.max(1, Math.floor(size.height > 0 ? size.height : 600)),
          metric: 'on_disk',
          max_depth: 6,
          min_tile_px: 4,
          max_tiles: 2_000,
        })
        .then((response) => {
          if (!active) return;
          setTiles(response.tiles);
          setTruncated(response.truncated);
        })
        .catch((cause: unknown) => {
          if (active) setError(normalizeError(cause));
        })
        .finally(() => {
          if (active) setLoading(false);
        });
    }, 120);

    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [generation, parentId, size]);

  const select = (tile: TreemapTile) => {
    if (generation === null || tile.node_id === null) {
      setSelected(null);
      return;
    }

    void api
      .treemapNode(generation, tile.node_id)
      .then(async (node) => {
        setSelected(node);
        setSelectedPath(await api.treePath(generation, node.id));
      })
      .catch((cause: unknown) => {
        setError(normalizeError(cause));
      });
  };

  const activate = (tile: TreemapTile) => {
    if (generation === null || tile.node_id === null || tile.child_count === 0) return;

    void api
      .treemapNode(generation, tile.node_id)
      .then(async (node) => {
        if (node.kind !== 'directory') return;
        const path = await api.treePath(generation, node.id);
        setTrail([...path, node]);
        setParentId(node.id);
        setSelected(node);
      })
      .catch((cause: unknown) => {
        setError(normalizeError(cause));
      });
  };

  const up = () => {
    if (trail.length === 0) return;
    const next = trail.slice(0, -1);
    setTrail(next);
    setParentId(next.at(-1)?.id ?? null);
  };

  if (generation === null) {
    return (
      <EmptyState
        title={t('treemap.empty')}
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

  const crumbs = [
    {
      label: volume?.mount_point ?? 'C:\\',
      onClick: () => {
        setParentId(null);
        setTrail([]);
      },
    },
    ...trail.map((node, index) => ({
      label: node.name,
      onClick:
        index === trail.length - 1
          ? undefined
          : () => {
              const next = trail.slice(0, index + 1);
              setTrail(next);
              setParentId(node.id);
            },
    })),
  ];

  return (
    <div className={styles.treemapLayout}>
      <div className={styles.toolbar}>
        <Button variant="ghost" disabled={trail.length === 0} onClick={up}>
          ↑ {t('treemap.up')}
        </Button>
        <Breadcrumbs items={crumbs} />
        <span className={styles.spacer} />
        <ul className={styles.legend} aria-label={t('treemap.legend')}>
          {LEGEND.map(([category, token]) => (
            <li key={category} className={styles.legendItem}>
              <span
                className={styles.dot}
                style={{ background: `var(${token})` }}
                aria-hidden="true"
              />
              {categoryLabel(category)}
            </li>
          ))}
        </ul>
      </div>

      {selected !== null ? (
        <p className={styles.currentPath}>
          {t('treemap.selected')}: {selected.name} ·{' '}
          {formatBytes(selected.on_disk_size, units)} · {t(`kind.${selected.kind}`)}
        </p>
      ) : null}

      {error !== null ? (
        <ErrorState title={t(error.message_key)} code={error.code} details={error.details ?? undefined} />
      ) : null}

      <SplitPane
        initialLeftPercent={68}
        left={
          <div ref={wrap} className={styles.canvasWrap}>
            {loading && tiles.length === 0 ? (
              <Loading label={t('common.loading')} rows={3} />
            ) : tiles.length === 0 ? (
              <EmptyState title={t('treemap.empty')} />
            ) : (
              <TreemapCanvas
                tiles={tiles}
                selectedNodeId={selected?.id ?? null}
                label={t('treemap.zoom_hint')}
                onSelect={select}
                onActivate={activate}
                onUp={up}
                describeTile={(tile) => {
                  const name =
                    tile.label === '' ? t('treemap.category.other') : tile.label;
                  return `${name} · ${formatBytes(tile.size, units)} · ${categoryLabel(tile.color_key)}`;
                }}
              />
            )}
          </div>
        }
        right={
          <div className={styles.aside}>
            {truncated ? (
              <p className={styles.currentPath}>{t('treemap.more', { count: '…' })}</p>
            ) : null}
            <DetailsPanel node={selected} ancestors={selectedPath} />
            {selected !== null && selected.kind === 'directory' ? (
              <Button
                variant="secondary"
                onClick={() => {
                  useShellStore.getState().setScreen('tree');
                }}
              >
                {t('treemap.open_tree')}
              </Button>
            ) : null}
          </div>
        }
      />
    </div>
  );
}
