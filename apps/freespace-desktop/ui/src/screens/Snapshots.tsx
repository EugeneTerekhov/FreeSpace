import { useCallback, useEffect, useState } from 'react';

import { api } from '../api/runtime';
import type {
  SnapshotDiffPage,
  SnapshotDiffRow,
  SnapshotDiffSort,
  SnapshotSummary,
  UiError,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Card, StatCard } from '../components/Card';
import { Badge } from '../components/Choice';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState } from '../components/Feedback';
import { Select } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './analysis.module.css';

/** Stored snapshots, creation and the comparison table. */
export function Snapshots() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const [snapshots, setSnapshots] = useState<SnapshotSummary[]>([]);
  const [base, setBase] = useState('');
  const [target, setTarget] = useState('');
  const [diff, setDiff] = useState<SnapshotDiffPage | null>(null);
  const [sort, setSort] = useState<SnapshotDiffSort>({
    key: 'size_delta',
    descending: true,
  });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const list = await api.snapshotsList();
      setSnapshots(list);
      setBase((current) => (current !== '' ? current : list[0]?.snapshot_id ?? ''));
      setTarget((current) => (current !== '' ? current : list[1]?.snapshot_id ?? ''));
    } catch (cause) {
      setError(normalizeError(cause));
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

  const columns: Column<SnapshotDiffRow>[] = [
    {
      id: 'path',
      header: t('top.path'),
      width: 'minmax(240px, 2fr)',
      render: (row) => <span className={styles.pathCell} title={row.path}>{row.path}</span>,
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
      width: '140px',
      align: 'end',
      render: (row) => {
        const sign = row.size_delta > 0 ? '+' : row.size_delta < 0 ? '−' : '';
        return `${sign}${formatBytes(Math.abs(row.size_delta), units)}`;
      },
    },
    {
      id: 'base',
      header: t('snapshots.base'),
      width: '120px',
      align: 'end',
      render: (row) => formatBytes(row.base_size, units),
    },
    {
      id: 'target',
      header: t('snapshots.target'),
      width: '120px',
      align: 'end',
      render: (row) => formatBytes(row.target_size, units),
    },
  ];

  return (
    <div className={styles.stack}>
      <Card title={t('nav.snapshots')}>
        <p>{t('snapshots.intro')}</p>
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
          {generation === null ? <span>{t('snapshots.scan_required')}</span> : null}
        </div>
        {notice !== null ? <Banner tone="info">{notice}</Banner> : null}
      </Card>

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={<Button variant="primary" onClick={() => void refresh()}>{t('error.retry')}</Button>}
        />
      ) : null}

      <Card title={t('snapshots.compare')}>
        <div className={styles.toolbar}>
          <label>
            {t('snapshots.base')}{' '}
            <Select
              aria-label={t('snapshots.base')}
              value={base}
              options={snapshots.map((snapshot) => ({
                value: snapshot.snapshot_id,
                label: `${snapshot.created_at} · ${snapshot.snapshot_id}`,
              }))}
              onChange={(event) => setBase(event.target.value)}
            />
          </label>
          <label>
            {t('snapshots.target')}{' '}
            <Select
              aria-label={t('snapshots.target')}
              value={target}
              options={snapshots.map((snapshot) => ({
                value: snapshot.snapshot_id,
                label: `${snapshot.created_at} · ${snapshot.snapshot_id}`,
              }))}
              onChange={(event) => setTarget(event.target.value)}
            />
          </label>
          <label>
            {t('snapshots.sort')}{' '}
            <Select
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
          </label>
          <Button
            variant="primary"
            loading={loading}
            disabled={base === '' || target === '' || base === target}
            onClick={() => void compare()}
          >
            {t('snapshots.diff')}
          </Button>
        </div>
      </Card>

      <Card title={t('snapshots.list')}>
        {snapshots.length === 0 ? (
          <EmptyState title={t('snapshots.none')} />
        ) : (
          <table className={local.table}>
            <thead>
              <tr>
                <th scope="col">{t('snapshots.created_at')}</th>
                <th scope="col">{t('snapshots.mount')}</th>
                <th scope="col">{t('snapshots.nodes')}</th>
                <th scope="col">{t('details.size')}</th>
                <th scope="col">{t('snapshots.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {snapshots.map((snapshot) => (
                <tr key={snapshot.snapshot_id}>
                  <td>{snapshot.created_at}</td>
                  <td>{snapshot.mount_point}</td>
                  <td>{snapshot.node_count}</td>
                  <td>{formatBytes(snapshot.file_size, units)}</td>
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
      </Card>

      {diff !== null ? (
        <>
          <section className={styles.stats} aria-label={t('snapshots.summary')}>
            <StatCard label={t('snapshots.added')} value={diff.added} />
            <StatCard label={t('snapshots.removed')} value={diff.removed} />
            <StatCard label={t('snapshots.changed')} value={diff.changed} />
            <StatCard
              label={t('snapshots.total_growth')}
              value={`${diff.total_growth_bytes > 0 ? '+' : diff.total_growth_bytes < 0 ? '−' : ''}${formatBytes(Math.abs(diff.total_growth_bytes), units)}`}
            />
          </section>
          <Card title={t('snapshots.diff')}>
            <DataTable
              label={t('snapshots.diff')}
              rows={diff.rows}
              columns={columns}
              rowKey={(row) => `${row.change}-${row.path}`}
              height={360}
              empty={t('snapshots.no_diff')}
            />
          </Card>
          {diff.growth_by_directory.length > 0 ? (
            <Card title={t('snapshots.growth')}>
              <ul>
                {diff.growth_by_directory.map((entry) => (
                  <li key={entry.path}>
                    {entry.path}: {entry.delta > 0 ? '+' : '−'}
                    {formatBytes(Math.abs(entry.delta), units)}
                  </li>
                ))}
              </ul>
            </Card>
          ) : null}
        </>
      ) : null}
    </div>
  );
}
