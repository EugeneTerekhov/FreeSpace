import { useCallback, useEffect, useMemo, useState } from 'react';
import { ShieldAlert, ShieldCheck, ShieldX, type LucideIcon } from 'lucide-react';

import { api } from '../api/runtime';
import type { CleanupCandidateView, FilterSet, NodeRow, TreePage, UiError } from '../api/dto';
import { normalizeError } from '../app/errors';
import { openInTree } from '../app/navigation';
import { Button } from '../components/Button';
import { Badge, Chip, type BadgeTone } from '../components/Choice';
import { Banner, EmptyState, ErrorState, Loading } from '../components/Feedback';
import { Field } from '../components/Fields';
import { FilterBar } from '../components/FilterBar';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { emptyFilter } from '../lib/filters';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import { ScreenFrameTabs } from './ScreenTabs';
import { Snapshots } from './Snapshots';
import styles from './cleanup.module.css';

/**
 * Risk is a word and an icon first: the tone only reinforces it (contract §1.1
 * and §5.2 forbid colour as the only carrier of status).
 */
const RISK: Record<string, { tone: BadgeTone; icon: LucideIcon }> = {
  low: { tone: 'success', icon: ShieldCheck },
  medium: { tone: 'warning', icon: ShieldAlert },
  high: { tone: 'danger', icon: ShieldX },
};

const RISK_FALLBACK: { tone: BadgeTone; icon: LucideIcon } = {
  tone: 'neutral',
  icon: ShieldAlert,
};

interface CategorySummary {
  category: string;
  count: number;
  bytes: number;
  ids: number[];
}

/** Cleanup recommendations and empty directories, all behind the shared preview. */
export function CleanupCandidates() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [olderThan, setOlderThan] = useState(180);
  const [filter, setFilter] = useState<FilterSet>(emptyFilter());
  const [candidates, setCandidates] = useState<CleanupCandidateView[]>([]);
  const [selectedCandidates, setSelectedCandidates] = useState<ReadonlySet<number>>(new Set());
  const [emptyDirs, setEmptyDirs] = useState<NodeRow[]>([]);
  const [emptyPage, setEmptyPage] = useState<TreePage | null>(null);
  const [selectedDirs, setSelectedDirs] = useState<ReadonlySet<number>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const operation = useOperation();

  const loadCandidates = useCallback(async () => {
    if (generation === null) return;
    setLoading(true);
    setError(null);
    try {
      const result = await api.cleanupCandidates(generation, olderThan, filter);
      setCandidates(result);
      setSelectedCandidates(new Set());
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  }, [filter, generation, olderThan]);

  const loadEmptyDirs = useCallback(
    async (cursor?: string | null) => {
      if (generation === null) return;
      setLoading(true);
      setError(null);
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
        setError(normalizeError(cause));
      } finally {
        setLoading(false);
      }
    },
    [generation],
  );

  useEffect(() => {
    if (generation === null) return;
    setCandidates([]);
    setEmptyDirs([]);
    setEmptyPage(null);
    setSelectedDirs(new Set());
    void loadCandidates();
  }, [generation, loadCandidates]);

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
  const categories = useMemo<CategorySummary[]>(() => {
    const groups = new Map<string, CategorySummary>();
    for (const candidate of candidates) {
      const entry = groups.get(candidate.category) ?? {
        category: candidate.category,
        count: 0,
        bytes: 0,
        ids: [],
      };
      entry.count += 1;
      entry.bytes += candidate.reclaimable_bytes;
      entry.ids.push(candidate.node_id);
      groups.set(candidate.category, entry);
    }

    return [...groups.values()].sort((left, right) => right.bytes - left.bytes);
  }, [candidates]);

  const selectedCount = candidatePaths.length + dirPaths.length;
  const selectedBytes = totalReclaimable + emptyDirBytes;

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

  const setCandidatesSelected = (ids: readonly number[], checked: boolean) => {
    setSelectedCandidates((current) => {
      const next = new Set(current);
      for (const id of ids) {
        if (checked) next.add(id);
        else next.delete(id);
      }
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
              onChange={(event) => setOlderThan(Math.max(0, Number(event.target.value)))}
            />
          </Field>
          <div className={styles.buttonGroup}>
            <Button variant="secondary" loading={loading} onClick={() => void loadCandidates()}>
              {t('cleanup.analyze')}
            </Button>
            <Button variant="secondary" loading={loading} onClick={() => void loadEmptyDirs()}>
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

      {categories.length > 0 ? (
        <Panel
          title={t('cleanup.panels.categories')}
          hint={t('cleanup.panels.categories_hint')}
          actions={
            <>
              <Button
                variant="ghost"
                disabled={candidates.length === 0}
                onClick={() => setCandidatesSelected(candidates.map((item) => item.node_id), true)}
              >
                {t('cleanup.panels.select_all')}
              </Button>
              <Button
                variant="ghost"
                disabled={selectedCandidates.size === 0}
                onClick={() => setCandidatesSelected([...selectedCandidates], false)}
              >
                {t('cleanup.panels.clear_selection')}
              </Button>
            </>
          }
        >
          <div className={styles.categories}>
            {categories.map((entry) => {
              const all = entry.ids.every((id) => selectedCandidates.has(id));

              return (
                <Chip
                  key={entry.category}
                  selected={all}
                  onClick={() => setCandidatesSelected(entry.ids, !all)}
                >
                  {`${t(`cleanup.reason.${entry.category}`)} · ${formatNumber(entry.count, language)} · ${formatBytes(entry.bytes, units)}`}
                </Chip>
              );
            })}
          </div>
        </Panel>
      ) : null}

      <Panel title={t('cleanup.recommendations')} hint={t('cleanup.panels.risk_hint')}>
        <FilterBar screen="cleanup" filter={filter} onChange={setFilter} />

        {error !== null ? (
          <ErrorState
            title={t(error.message_key)}
            code={error.code}
            details={error.details ?? undefined}
            actions={
              <Button variant="primary" onClick={() => void loadCandidates()}>
                {t('error.retry')}
              </Button>
            }
          />
        ) : null}

        {loading && candidates.length === 0 ? (
          <Loading label={t('common.loading')} rows={3} />
        ) : candidates.length === 0 ? (
          <EmptyState title={t('cleanup.none')} description={t('cleanup.intro')} />
        ) : (
          <>
            <p className={styles.hint}>{t('duplicates.kept_hint')}</p>
            <div className={styles.tableWrap}>
              <table
                className={`${styles.table} ${styles.tableFixed}`}
                aria-label={t('cleanup.recommendations')}
              >
                <thead>
                  <tr>
                    <th scope="col" className={styles.colSelect}>
                      <span className={styles.visuallyHidden}>{t('duplicates.delete')}</span>
                    </th>
                    <th scope="col">{t('top.path')}</th>
                    <th scope="col" className={styles.colRisk}>
                      {t('cleanup.risk_label')}
                    </th>
                    <th scope="col" className={styles.colReason}>
                      {t('cleanup.reason_label')}
                    </th>
                    <th scope="col" className={`${styles.numeric} ${styles.colSize}`}>
                      {t('top.size')}
                    </th>
                    <th scope="col" className={styles.colAction}>
                      {t('details.items')}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {candidates.map((candidate) => {
                    const risk = RISK[candidate.risk] ?? RISK_FALLBACK;
                    const Icon = risk.icon;
                    const known = RISK[candidate.risk] !== undefined;

                    return (
                      <tr key={candidate.node_id}>
                        <td>
                          <input
                            type="checkbox"
                            aria-label={`${t('tree.selected')}: ${candidate.name}`}
                            checked={selectedCandidates.has(candidate.node_id)}
                            onChange={(event) =>
                              setSelection(
                                setSelectedCandidates,
                                candidate.node_id,
                                event.target.checked,
                              )
                            }
                          />
                        </td>
                        <td className={styles.path} title={candidate.path}>
                          {candidate.path}
                        </td>
                        <td>
                          <Badge tone={risk.tone}>
                            <span className={styles.risk}>
                              <Icon size={12} strokeWidth={2} aria-hidden="true" />
                              {known
                                ? t(`cleanup.risk.${candidate.risk}`)
                                : t('cleanup.panels.risk_unknown')}
                            </span>
                          </Badge>
                        </td>
                        <td>{t(candidate.reason_key)}</td>
                        <td className={styles.numeric}>
                          {formatBytes(candidate.reclaimable_bytes, units)}
                        </td>
                        <td>
                          <Button variant="ghost" onClick={() => openInTree(candidate.node_id)}>
                            {t('treemap.open_tree')}
                          </Button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </>
        )}
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
              <div className={styles.tableWrap}>
                <table
                  className={`${styles.table} ${styles.tableFixed}`}
                  aria-label={t('cleanup.empty_dirs')}
                >
                  <thead>
                    <tr>
                      <th scope="col" className={styles.colSelect}>
                        <span className={styles.visuallyHidden}>{t('duplicates.delete')}</span>
                      </th>
                      <th scope="col">{t('top.path')}</th>
                      <th scope="col" className={styles.colAction}>
                        {t('details.items')}
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {emptyDirs.map((row) => (
                      <tr key={row.id}>
                        <td>
                          <input
                            type="checkbox"
                            aria-label={`${t('tree.selected')}: ${row.name}`}
                            checked={selectedDirs.has(row.id)}
                            onChange={(event) =>
                              setSelection(setSelectedDirs, row.id, event.target.checked)
                            }
                          />
                        </td>
                        <td className={styles.path} title={row.path}>
                          {row.path}
                        </td>
                        <td>
                          <Button variant="ghost" onClick={() => openInTree(row.id)}>
                            {t('treemap.open_tree')}
                          </Button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
          {emptyPage?.next_cursor !== null && emptyPage?.next_cursor !== undefined ? (
            <div className={styles.panelFoot}>
              <Button variant="secondary" onClick={() => void loadEmptyDirs(emptyPage.next_cursor)}>
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
