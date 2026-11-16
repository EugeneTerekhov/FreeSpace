import { useCallback, useEffect, useMemo, useState } from 'react';

import { api } from '../api/runtime';
import type { CleanupCandidateView, FilterSet, NodeRow, TreePage, UiError } from '../api/dto';
import { normalizeError } from '../app/errors';
import { openInTree } from '../app/navigation';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Badge } from '../components/Choice';
import { EmptyState, ErrorState } from '../components/Feedback';
import { FilterBar } from '../components/FilterBar';
import { emptyFilter } from '../lib/filters';
import { Field } from '../components/Fields';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes } from '../i18n/format';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './analysis.module.css';

const RISK_TONE = {
  low: 'success',
  medium: 'warning',
  high: 'danger',
} as const;

/** Cleanup recommendations and empty directories, all behind the shared preview. */
export function Cleanup() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
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

  const toggle = (
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
      <Card title={t('nav.cleanup')}>
        <p>{t('cleanup.intro')}</p>
        <div className={styles.toolbar}>
          <Field id="cleanup-older" label={t('cleanup.older_than')}>
            <input
              id="cleanup-older"
              className={local.input}
              type="number"
              min={0}
              max={3650}
              value={olderThan}
              onChange={(event) => setOlderThan(Math.max(0, Number(event.target.value)))}
            />
          </Field>
          <Button variant="secondary" loading={loading} onClick={() => void loadCandidates()}>
            {t('cleanup.analyze')}
          </Button>
          <Button variant="secondary" loading={loading} onClick={() => void loadEmptyDirs()}>
            {t('cleanup.find_empty')}
          </Button>
          <span className={styles.spacer} />
          <Button
            variant="danger"
            disabled={candidatePaths.length === 0 && dirPaths.length === 0}
            onClick={() => {
              void operation.prepare({
                kind: 'delete',
                targets: [...candidatePaths, ...dirPaths],
                delete_mode: 'recycle_bin',
                reason: 'cleanup',
              });
            }}
          >
            {t('analysis.delete_selected', {
              count: candidatePaths.length + dirPaths.length,
            })}
          </Button>
        </div>
        {selectedCandidates.size > 0 ? (
          <p className={styles.metaRow}>
            {t('cleanup.total', { size: formatBytes(totalReclaimable, units) })}
          </p>
        ) : null}
      </Card>

      <FilterBar screen="cleanup" filter={filter} onChange={setFilter} />

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={<Button variant="primary" onClick={() => void loadCandidates()}>{t('error.retry')}</Button>}
        />
      ) : null}

      <Card title={t('cleanup.recommendations')}>
        {candidates.length === 0 && !loading ? (
          <EmptyState title={t('cleanup.none')} />
        ) : (
          <table className={local.table} aria-label={t('cleanup.recommendations')}>
            <thead>
              <tr>
                <th scope="col">{t('duplicates.delete')}</th>
                <th scope="col">{t('top.path')}</th>
                <th scope="col">{t('cleanup.risk_label')}</th>
                <th scope="col">{t('duplicates.kept_hint')}</th>
                <th scope="col" className={local.numeric}>{t('top.size')}</th>
                <th scope="col">{t('details.items')}</th>
              </tr>
            </thead>
            <tbody>
              {candidates.map((candidate) => (
                <tr key={candidate.node_id}>
                  <td>
                    <input
                      type="checkbox"
                      aria-label={`${t('tree.selected')}: ${candidate.name}`}
                      checked={selectedCandidates.has(candidate.node_id)}
                      onChange={(event) =>
                        toggle(setSelectedCandidates, candidate.node_id, event.target.checked)
                      }
                    />
                  </td>
                  <td className={styles.pathCell} title={candidate.path}>{candidate.path}</td>
                  <td>
                    <Badge tone={RISK_TONE[candidate.risk as keyof typeof RISK_TONE] ?? 'neutral'}>
                      {t(`cleanup.risk.${candidate.risk}`)}
                    </Badge>
                  </td>
                  <td>{t(candidate.reason_key)}</td>
                  <td className={local.numeric}>
                    {formatBytes(candidate.reclaimable_bytes, units)}
                  </td>
                  <td>
                    <Button variant="ghost" onClick={() => openInTree(candidate.node_id)}>
                      {t('treemap.open_tree')}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      {emptyPage !== null || emptyDirs.length > 0 ? (
        <Card title={t('cleanup.empty_dirs')}>
          {emptyDirs.length === 0 ? (
            <EmptyState title={t('cleanup.none')} />
          ) : (
            <table className={local.table} aria-label={t('cleanup.empty_dirs')}>
              <thead>
                <tr>
                  <th scope="col">{t('duplicates.delete')}</th>
                  <th scope="col">{t('top.path')}</th>
                  <th scope="col">{t('details.items')}</th>
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
                        onChange={(event) => toggle(setSelectedDirs, row.id, event.target.checked)}
                      />
                    </td>
                    <td className={styles.pathCell} title={row.path}>{row.path}</td>
                    <td>
                      <Button variant="ghost" onClick={() => openInTree(row.id)}>
                        {t('treemap.open_tree')}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {emptyPage?.next_cursor !== null && emptyPage?.next_cursor !== undefined ? (
            <div className={styles.analyzeRow}>
              <Button variant="secondary" onClick={() => void loadEmptyDirs(emptyPage.next_cursor)}>
                {t('tree.load_more')}
              </Button>
            </div>
          ) : null}
        </Card>
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
