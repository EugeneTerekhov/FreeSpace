import { useCallback, useEffect, useState } from 'react';

import { api } from '../api/runtime';
import type {
  SnapshotDiffPage,
  SnapshotDiffRow,
  SnapshotDiffSort,
  SnapshotSummary,
  UiError,
  UnitSystem,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Badge } from '../components/Choice';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, Loading } from '../components/Feedback';
import { Field, Select } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import styles from './cleanup.module.css';

/** Signed size delta with the typographic minus used across the interface. */
function signedBytes(delta: number, units: UnitSystem): string {
  const sign = delta > 0 ? '+' : delta < 0 ? '−' : '';
  return `${sign}${formatBytes(Math.abs(delta), units)}`;
}

/** Stored snapshots, creation and the comparison table. */
export function Snapshots() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [snapshots, setSnapshots] = useState<SnapshotSummary[]>([]);
  const [base, setBase] = useState('');
  const [target, setTarget] = useState('');
  const [diff, setDiff] = useState<SnapshotDiffPage | null>(null);
  const [sort, setSort] = useState<SnapshotDiffSort>({
    key: 'size_delta',
    descending: true,
  });
  const [loading, setLoading] = useState(false);
  const [listLoading, setListLoading] = useState(true);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    setListLoading(true);
    try {
      const list = await api.snapshotsList();
      setSnapshots(list);
      setBase((current) => (current !== '' ? current : list[0]?.snapshot_id ?? ''));
      setTarget((current) => (current !== '' ? current : list[1]?.snapshot_id ?? ''));
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setListLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const create = async () => {
    if (generation === null) return;
    setLoading(true);
    setError(null);
    try {
      const created = await api.snapshotsCreate({
        path: useSnapshotStore.getState().volume?.mount_point ?? null,
        out: null,
      });
      setNotice(t('snapshots.created', { count: created.node_count }));
      await refresh();
      setBase(created.snapshot_id);
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  };

  const compare = async () => {
    if (base === '' || target === '') return;
    setLoading(true);
    setError(null);
    try {
      const started = await api.snapshotsCompare(base, target);
      const page = await api.snapshotsDiffPage(started.task_id, {
        sort,
        pageSize: 200,
      });
      setDiff(page);
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  };

  const remove = async (snapshotId: string) => {
    setError(null);
    try {
      await api.snapshotsDelete(snapshotId);
      await refresh();
    } catch (cause) {
      setError(normalizeError(cause));
    }
  };

  const snapshotBytes = snapshots.reduce((sum, snapshot) => sum + snapshot.file_size, 0);
  const grown =
    diff === null
      ? 0
      : diff.rows.filter((row) => row.change === 'changed' && row.size_delta > 0).length;
  const shrunk =
    diff === null
      ? 0
      : diff.rows.filter((row) => row.change === 'changed' && row.size_delta < 0).length;

  const columns: Column<SnapshotDiffRow>[] = [
    {
      id: 'path',
      header: t('top.path'),
      width: 'minmax(240px, 2fr)',
      render: (row) => (
        <span className={styles.path} title={row.path}>
          {row.path}
        </span>
      ),
    },
    {
      id: 'change',
      header: t('snapshots.change_label'),
      width: '120px',
      render: (row) => (
        <Badge
          tone={row.change === 'added' ? 'success' : row.change === 'removed' ? 'danger' : 'warning'}
        >
          {t(`snapshots.change.${row.change}`)}
        </Badge>
      ),
    },
    {
      id: 'delta',
      header: t('snapshots.delta'),
      width: '108px',
      align: 'end',
      render: (row) => signedBytes(row.size_delta, units),
    },
    {
      id: 'base',
      header: t('snapshots.base'),
      width: '100px',
      align: 'end',
      render: (row) => formatBytes(row.base_size, units),
    },
    {
      id: 'target',
      header: t('snapshots.target'),
      width: '100px',
      align: 'end',
      render: (row) => formatBytes(row.target_size, units),
    },
  ];

  return (
    <div className={styles.stack}>
      <Panel title={t('nav.snapshots')} hint={t('snapshots.intro')}>
        <div className={styles.toolbar}>
          <Button
            variant="primary"
            loading={loading}
            disabled={generation === null}
            onClick={() => void create()}
          >
            {t('snapshots.create')}
          </Button>
          <Button variant="secondary" onClick={() => void refresh()}>
            {t('journal.refresh')}
          </Button>
          {generation === null ? (
            <span className={styles.hint}>{t('snapshots.scan_required')}</span>
          ) : null}
        </div>
        {notice !== null ? <Banner tone="info">{notice}</Banner> : null}
      </Panel>

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void refresh()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      <Panel title={t('snapshots.list')}>
        <StatRow>
          <Stat
            label={t('cleanup.panels.snapshot_count')}
            value={formatNumber(snapshots.length, language)}
          />
          <Stat
            label={t('cleanup.panels.snapshot_size')}
            value={formatBytes(snapshotBytes, units)}
          />
        </StatRow>
        {listLoading && snapshots.length === 0 ? (
          <Loading label={t('common.loading')} rows={3} />
        ) : snapshots.length === 0 ? (
          <EmptyState title={t('snapshots.none')} description={t('snapshots.intro')} />
        ) : (
          <table className={styles.table} aria-label={t('snapshots.list')}>
            <thead>
              <tr>
                <th scope="col">{t('snapshots.created_at')}</th>
                <th scope="col">{t('snapshots.mount')}</th>
                <th scope="col" className={styles.numeric}>
                  {t('snapshots.nodes')}
                </th>
                <th scope="col" className={styles.numeric}>
                  {t('details.size')}
                </th>
                <th scope="col">{t('snapshots.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {snapshots.map((snapshot) => (
                <tr key={snapshot.snapshot_id}>
                  <td>{snapshot.created_at}</td>
                  <td className={styles.path} title={snapshot.path}>
                    {snapshot.mount_point}
                  </td>
                  <td className={styles.numeric}>
                    {formatNumber(snapshot.node_count, language)}
                  </td>
                  <td className={styles.numeric}>{formatBytes(snapshot.file_size, units)}</td>
                  <td>
                    <Button variant="ghost" onClick={() => void remove(snapshot.snapshot_id)}>
                      {t('confirm.apply')}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Panel>

      <Panel title={t('snapshots.compare')}>
        <div className={styles.toolbar}>
          <div className={styles.fieldWide}>
            <Field id="snapshot-base" label={t('snapshots.base')}>
              <Select
                id="snapshot-base"
                aria-label={t('snapshots.base')}
                value={base}
                options={snapshots.map((snapshot) => ({
                  value: snapshot.snapshot_id,
                  label: `${snapshot.created_at} · ${snapshot.snapshot_id}`,
                }))}
                onChange={(event) => setBase(event.target.value)}
              />
            </Field>
          </div>
          <div className={styles.fieldWide}>
            <Field id="snapshot-target" label={t('snapshots.target')}>
              <Select
                id="snapshot-target"
                aria-label={t('snapshots.target')}
                value={target}
                options={snapshots.map((snapshot) => ({
                  value: snapshot.snapshot_id,
                  label: `${snapshot.created_at} · ${snapshot.snapshot_id}`,
                }))}
                onChange={(event) => setTarget(event.target.value)}
              />
            </Field>
          </div>
          <div className={styles.fieldNarrow}>
            <Field id="snapshot-sort" label={t('snapshots.sort')}>
              <Select
                id="snapshot-sort"
                aria-label={t('snapshots.sort')}
                value={sort.key}
                options={[
                  { value: 'size_delta', label: t('snapshots.delta') },
                  { value: 'path', label: t('top.path') },
                  { value: 'change', label: t('snapshots.change_label') },
                ]}
                onChange={(event) =>
                  setSort((current) => ({
                    ...current,
                    key: event.target.value as SnapshotDiffSort['key'],
                  }))
                }
              />
            </Field>
          </div>
          <Button
            variant="primary"
            loading={loading}
            disabled={base === '' || target === '' || base === target}
            onClick={() => void compare()}
          >
            {t('snapshots.diff')}
          </Button>
        </div>
      </Panel>

      {diff !== null ? (
        <>
          <StatRow>
            <Stat label={t('snapshots.added')} value={formatNumber(diff.added, language)} />
            <Stat label={t('snapshots.removed')} value={formatNumber(diff.removed, language)} />
            <Stat label={t('cleanup.panels.grown')} value={formatNumber(grown, language)} />
            <Stat label={t('cleanup.panels.shrunk')} value={formatNumber(shrunk, language)} />
            <Stat
              label={t('snapshots.total_growth')}
              value={signedBytes(diff.total_growth_bytes, units)}
            />
          </StatRow>
          <Panel
            title={t('snapshots.diff')}
            hint={
              diff.next_cursor !== null
                ? t('cleanup.panels.diff_page_hint', {
                    loaded: diff.rows.length,
                    total: diff.total_estimate,
                  })
                : undefined
            }
          >
            <DataTable
              label={t('snapshots.diff')}
              rows={diff.rows}
              columns={columns}
              rowKey={(row) => `${row.change}-${row.path}`}
              height={360}
              empty={t('snapshots.no_diff')}
            />
          </Panel>
          {diff.growth_by_directory.length > 0 ? (
            <Panel title={t('snapshots.growth')}>
              <ul className={styles.growthList}>
                {diff.growth_by_directory.map((entry) => (
                  <li key={entry.path} className={styles.growthRow}>
                    <span className={styles.growthPath} title={entry.path}>
                      {entry.path}
                    </span>
                    <span className={styles.growthValue}>
                      {signedBytes(entry.delta, units)}
                    </span>
                  </li>
                ))}
              </ul>
            </Panel>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
