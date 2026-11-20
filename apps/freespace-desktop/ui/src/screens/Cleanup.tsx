import { useCallback, useEffect, useMemo, useState } from 'react';
import { Folder } from 'lucide-react';

import { api } from '../api/runtime';
import type {
  CleanupCandidateView,
  FilterSet,
  NodeRow,
  TreePage,
  UiError,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { openInTree } from '../app/navigation';
import { Button, IconButton } from '../components/Button';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, Loading } from '../components/Feedback';
import { Field } from '../components/Fields';
import { FilterBar } from '../components/FilterBar';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useCleanupStore } from '../stores/cleanupStore';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import { ScreenFrameTabs } from './ScreenTabs';
import { Snapshots } from './Snapshots';
import styles from './cleanup.module.css';

/** The word carries the status; colour only reinforces it (contract §5.2). */
const RISK_CLASS: Record<string, string | undefined> = {
  low: styles.riskLow,
  medium: styles.riskMedium,
  high: styles.riskHigh,
};

/** Cleanup reasons join the shared filter row; they never select candidates. */
const CLEANUP_REASON_FILTERS = [
  { id: 'outdated', labelKey: 'cleanup.reason.outdated' },
  { id: 'cache', labelKey: 'cleanup.reason.cache' },
  { id: 'log', labelKey: 'cleanup.reason.log' },
  { id: 'temp', labelKey: 'cleanup.reason.temp' },
  { id: 'backup', labelKey: 'cleanup.reason.backup' },
] as const;

/** Cleanup recommendations and empty directories, all behind the shared preview. */
export function CleanupCandidates() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');

  // The recommendation result, its query, selection and scroll anchor live in
  // a store so returning to this screen reuses them instead of refetching.
  const olderThan = useCleanupStore((state) => state.olderThan);
  const filter = useCleanupStore((state) => state.filter);
  const [appliedFilter] = useDebounced(filter);
  const candidates = useCleanupStore((state) => state.candidates);
  const selectedCandidates = useCleanupStore((state) => state.selected);
  const candidatesLoading = useCleanupStore((state) => state.loading);
  const candidatesError = useCleanupStore((state) => state.error);
  const scrollTop = useCleanupStore((state) => state.scrollTop);

  const [emptyDirs, setEmptyDirs] = useState<NodeRow[]>([]);
  const [emptyPage, setEmptyPage] = useState<TreePage | null>(null);
  const [selectedDirs, setSelectedDirs] = useState<ReadonlySet<number>>(new Set());
  const [emptyLoading, setEmptyLoading] = useState(false);
  const [emptyError, setEmptyError] = useState<UiError | null>(null);
  const operation = useOperation();

  const loadCandidates = useCallback(async () => {
    if (generation !== null) await useCleanupStore.getState().load(api, generation);
  }, [generation]);

  // A new generation invalidates the cache; an unchanged one keeps it. The
  // `loading` guard makes the load idempotent under StrictMode double effects.
  useEffect(() => {
    if (generation === null) return;
    const current = useCleanupStore.getState();
    if (current.generation !== generation) current.reset(generation);
    const next = useCleanupStore.getState();
    if (!next.loaded && !next.loading) void next.load(api, generation);
  }, [generation]);

  // Reason chips live in the shared filter row. A debounced change reloads the
  // recommendations so the list reflects the AND of the selected categories.
  useEffect(() => {
    if (generation === null) return;
    const current = useCleanupStore.getState();
    if (!current.loaded || current.loading) return;
    if (current.appliedFilter === appliedFilter) return;
    void current.load(api, generation, appliedFilter);
  }, [generation, appliedFilter]);

  useEffect(() => {
    setEmptyDirs([]);
    setEmptyPage(null);
    setSelectedDirs(new Set());
    setEmptyError(null);
  }, [generation]);

  const loadEmptyDirs = useCallback(
    async (cursor?: string | null) => {
      if (generation === null) return;
      setEmptyLoading(true);
      setEmptyError(null);
      try {
        const result = await api.cleanupEmptyDirsPage(generation, {
          cursor: cursor ?? null,
          pageSize: 200,
        });
        setEmptyDirs((current) =>
          cursor === undefined || cursor === null ? result.rows : [...current, ...result.rows],
        );
        setEmptyPage(result);
      } catch (cause) {
        setEmptyError(normalizeError(cause));
      } finally {
        setEmptyLoading(false);
      }
    },
    [generation],
  );

  const candidatePaths = useMemo(
    () =>
      candidates
        .filter((candidate) => selectedCandidates.has(candidate.node_id))
        .map((candidate) => candidate.path),
    [candidates, selectedCandidates],
  );
  const dirPaths = useMemo(
    () => emptyDirs.filter((row) => selectedDirs.has(row.id)).map((row) => row.path),
    [emptyDirs, selectedDirs],
  );
  const totalReclaimable = useMemo(
    () =>
      candidates
        .filter((candidate) => selectedCandidates.has(candidate.node_id))
        .reduce((sum, candidate) => sum + candidate.reclaimable_bytes, 0),
    [candidates, selectedCandidates],
  );
  const totalCandidateBytes = useMemo(
    () => candidates.reduce((sum, candidate) => sum + candidate.reclaimable_bytes, 0),
    [candidates],
  );
  const emptyDirBytes = useMemo(
    () =>
      emptyDirs
        .filter((row) => selectedDirs.has(row.id))
        .reduce((sum, row) => sum + row.on_disk_size, 0),
    [emptyDirs, selectedDirs],
  );

  const setSelection = (
    setter: (value: (current: ReadonlySet<number>) => ReadonlySet<number>) => void,
    id: number,
    checked: boolean,
  ) => {
    setter((current) => {
      const next = new Set(current);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
  };

  const previewDelete = (targets: string[]) => {
    if (targets.length === 0) return;
    void operation.prepare({
      kind: 'delete',
      targets,
      delete_mode: 'recycle_bin',
      reason: 'cleanup',
    });
  };

  const allCandidatesSelected =
    candidates.length > 0 && candidates.every((item) => selectedCandidates.has(item.node_id));
  const someCandidatesSelected = candidates.some((item) => selectedCandidates.has(item.node_id));
  const allDirsSelected =
    emptyDirs.length > 0 && emptyDirs.every((row) => selectedDirs.has(row.id));
  const someDirsSelected = emptyDirs.some((row) => selectedDirs.has(row.id));

  const candidateColumns = useMemo<Column<CleanupCandidateView>[]>(
    () => [
      {
        id: 'select',
        header: '',
        width: '40px',
        headerCheckbox: {
          checked: allCandidatesSelected,
          indeterminate: someCandidatesSelected && !allCandidatesSelected,
          disabled: candidates.length === 0,
          label: t('cleanup.select_all_filtered'),
          onChange: (checked) =>
            useCleanupStore
              .getState()
              .selectIds(
                candidates.map((item) => item.node_id),
                checked,
              ),
        },
        render: (row) => (
          <input
            type="checkbox"
            aria-label={`${t('tree.selected')}: ${row.name}`}
            checked={selectedCandidates.has(row.node_id)}
            onChange={(event) =>
              useCleanupStore.getState().toggle(row.node_id, event.target.checked)
            }
          />
        ),
      },
      {
        id: 'path',
        header: t('top.path'),
        width: 'minmax(220px, 1fr)',
        render: (row) => (
          <span className={styles.path} title={row.path}>
            {row.path}
          </span>
        ),
      },
      {
        id: 'risk',
        header: t('cleanup.risk_label'),
        width: '90px',
        render: (row) => {
          const known = RISK_CLASS[row.risk] !== undefined;

          return (
            <span className={RISK_CLASS[row.risk] ?? styles.riskUnknown ?? ''}>
              {known ? t(`cleanup.risk.${row.risk}`) : t('cleanup.panels.risk_unknown')}
            </span>
          );
        },
      },
      {
        id: 'reason',
        header: t('cleanup.reason_label'),
        width: '100px',
        render: (row) => t(row.reason_key),
      },
      {
        id: 'size',
        header: t('top.size'),
        width: '100px',
        align: 'end',
        render: (row) => formatBytes(row.reclaimable_bytes, units),
      },
      {
        id: 'open',
        header: '',
        width: '54px',
        render: (row) => (
          <IconButton
            label={t('treemap.open_tree')}
            onClick={() => openInTree(row.node_id)}
          >
            <Folder size={15} strokeWidth={1.7} aria-hidden="true" />
          </IconButton>
        ),
      },
    ],
    [allCandidatesSelected, candidates, selectedCandidates, someCandidatesSelected, units],
  );

  const emptyDirColumns = useMemo<Column<NodeRow>[]>(
    () => [
      {
        id: 'select',
        header: '',
        width: '40px',
        headerCheckbox: {
          checked: allDirsSelected,
          indeterminate: someDirsSelected && !allDirsSelected,
          disabled: emptyDirs.length === 0,
          label: t('cleanup.select_all_filtered'),
          onChange: (checked) =>
            setSelectedDirs(
              checked ? new Set(emptyDirs.map((row) => row.id)) : new Set<number>(),
            ),
        },
        render: (row) => (
          <input
            type="checkbox"
            aria-label={`${t('tree.selected')}: ${row.name}`}
            checked={selectedDirs.has(row.id)}
            onChange={(event) => setSelection(setSelectedDirs, row.id, event.target.checked)}
          />
        ),
      },
      {
        id: 'path',
        header: t('top.path'),
        width: 'minmax(220px, 1fr)',
        render: (row) => (
          <span className={styles.path} title={row.path}>
            {row.path}
          </span>
        ),
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
    ],
    [allDirsSelected, emptyDirs, selectedDirs, someDirsSelected],
  );

  const selectedCount = candidatePaths.length + dirPaths.length;
  const selectedBytes = totalReclaimable + emptyDirBytes;

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
      <Panel title={t('cleanup.panels.settings')} hint={t('cleanup.intro')}>
        <div className={styles.toolbar}>
          <Field
            id="cleanup-older"
            label={t('cleanup.older_than')}
            inline
            className={styles.olderField}
          >
            <input
              id="cleanup-older"
              className={styles.olderInput}
              type="number"
              min={0}
              max={3650}
              value={olderThan}
              onChange={(event) =>
                useCleanupStore.getState().setOlderThan(Math.max(0, Number(event.target.value)))
              }
            />
          </Field>
          <div className={styles.buttonGroup}>
            <Button
              variant="secondary"
              loading={candidatesLoading}
              onClick={() => void loadCandidates()}
            >
              {t('cleanup.analyze')}
            </Button>
            <Button
              variant="secondary"
              loading={emptyLoading}
              onClick={() => void loadEmptyDirs()}
            >
              {t('cleanup.find_empty')}
            </Button>
          </div>
          <span className={styles.spacer} />
          <Button
            variant="danger"
            disabled={selectedCount === 0}
            onClick={() => previewDelete([...candidatePaths, ...dirPaths])}
          >
            {t('analysis.delete_selected', { count: selectedCount })}
          </Button>
        </div>
      </Panel>

      <StatRow>
        <Stat
          label={t('cleanup.panels.candidates')}
          value={formatNumber(candidates.length, language)}
        />
        <Stat
          label={t('cleanup.panels.potential')}
          value={formatBytes(totalCandidateBytes, units)}
        />
        <Stat
          label={t('cleanup.panels.selected_count')}
          value={formatNumber(selectedCount, language)}
        />
        <Stat
          label={t('cleanup.panels.selected_total')}
          value={formatBytes(selectedBytes, units)}
        />
      </StatRow>

      <Panel title={t('cleanup.recommendations')} hint={t('cleanup.panels.risk_hint')}>
        <FilterBar
          filter={filter}
          onChange={(next) => useCleanupStore.getState().setFilter(next)}
          extraCategories={CLEANUP_REASON_FILTERS}
        />

        {candidatesError !== null ? (
          <ErrorState
            title={t(candidatesError.message_key)}
            code={candidatesError.code}
            details={candidatesError.details ?? undefined}
            actions={
              <Button variant="primary" onClick={() => void loadCandidates()}>
                {t('error.retry')}
              </Button>
            }
          />
        ) : null}

        <DataTable
          label={t('cleanup.recommendations')}
          rows={candidates}
          columns={candidateColumns}
          rowKey={(row) => row.node_id}
          loading={candidatesLoading}
          height={360}
          initialScrollTop={scrollTop}
          onScrollTopChange={(value) => useCleanupStore.getState().setScrollTop(value)}
          empty={
            candidatesLoading ? (
              <Loading label={t('common.loading')} rows={3} />
            ) : (
              <EmptyState title={t('cleanup.none')} description={t('cleanup.intro')} />
            )
          }
        />
      </Panel>

      {emptyPage !== null || emptyDirs.length > 0 ? (
        <Panel
          title={t('cleanup.empty_dirs')}
          actions={
            <Button
              variant="danger"
              disabled={dirPaths.length === 0}
              onClick={() => previewDelete(dirPaths)}
            >
              {t('cleanup.delete_empty')}
            </Button>
          }
        >
          {emptyError !== null ? (
            <ErrorState
              title={t(emptyError.message_key)}
              code={emptyError.code}
              details={emptyError.details ?? undefined}
              actions={
                <Button variant="primary" onClick={() => void loadEmptyDirs()}>
                  {t('error.retry')}
                </Button>
              }
            />
          ) : null}
          {emptyDirs.length === 0 ? (
            <EmptyState title={t('cleanup.none')} />
          ) : (
            <>
              <p className={styles.hint}>
                {t('cleanup.panels.empty_selected', { count: dirPaths.length })}
              </p>
              {emptyPage?.stale_cursor === true ? (
                <Banner tone="warning">{t('error.stale_cursor')}</Banner>
              ) : null}
              <DataTable
                label={t('cleanup.empty_dirs')}
                rows={emptyDirs}
                columns={emptyDirColumns}
                rowKey={(row) => row.id}
                loading={emptyLoading}
                height={280}
                empty={<EmptyState title={t('cleanup.none')} />}
              />
            </>
          )}
          {emptyPage?.next_cursor !== null && emptyPage?.next_cursor !== undefined ? (
            <div className={styles.panelFoot}>
              <Button variant="secondary" loading={emptyLoading} onClick={() => void loadEmptyDirs(emptyPage.next_cursor)}>
                {t('tree.load_more')}
              </Button>
            </div>
          ) : null}
        </Panel>
      ) : null}

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

const CLEANUP_TABS = [
  { id: 'candidates', labelKey: 'nav.cleanup' },
  { id: 'snapshots', labelKey: 'nav.snapshots' },
] as const;

type CleanupTab = (typeof CLEANUP_TABS)[number]['id'];

/** Cleanup: recommendations and snapshots as two tabs (T050). */
export function Cleanup() {
  const [tab, setTab] = useState<CleanupTab>('candidates');

  return (
    <ScreenFrameTabs
      idBase="cleanup"
      tabs={CLEANUP_TABS.map((item) => ({ id: item.id, label: t(item.labelKey) }))}
      active={tab}
      onChange={(id) => {
        setTab(id as CleanupTab);
      }}
    >
      {(id) => (id === 'candidates' ? <CleanupCandidates /> : <Snapshots />)}
    </ScreenFrameTabs>
  );
}

/** Debounces a filter and keeps object identity when only a new scan reset it. */
function useDebounced(filter: FilterSet): [FilterSet] {
  const [value, setValue] = useState(filter);
  useEffect(() => {
    const timer = setTimeout(() => {
      setValue((current) =>
        JSON.stringify(current) === JSON.stringify(filter) ? current : filter,
      );
    }, 250);

    return () => {
      clearTimeout(timer);
    };
  }, [filter]);

  return [value];
}
