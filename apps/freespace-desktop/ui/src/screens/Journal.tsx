import { useCallback, useEffect, useMemo, useState } from 'react';

import { api } from '../api/runtime';
import type {
  JournalEntryView,
  JournalFilter,
  JournalPage,
  JournalResult,
  OperationKind,
  UiError,
} from '../api/dto';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Badge } from '../components/Choice';
import { DataTable, type Column } from '../components/DataTable';
import { EmptyState, ErrorState, Loading } from '../components/Feedback';
import { SearchField, Select } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes, formatDate, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import styles from './screens.module.css';

const KINDS: readonly OperationKind[] = [
  'delete',
  'move',
  'rename',
  'permissions',
  'shred',
  'restore',
];
const RESULTS: readonly JournalResult[] = ['success', 'partial', 'failed', 'cancelled'];

/** Operation journal with server-side kind, result and text filters. */
export function Journal() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [entries, setEntries] = useState<JournalEntryView[]>([]);
  const [page, setPage] = useState<JournalPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [query, setQuery] = useState('');
  const [appliedQuery, setAppliedQuery] = useState('');
  const [kind, setKind] = useState('');
  const [result, setResult] = useState('');
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (query === appliedQuery) return undefined;
    const timer = setTimeout(() => {
      setAppliedQuery(query);
    }, 250);

    return () => {
      clearTimeout(timer);
    };
  }, [appliedQuery, query]);

  const filter = useMemo<JournalFilter>(() => {
    const selectedKind = KINDS.find((value) => value === kind);
    const selectedResult = RESULTS.find((value) => value === result);

    return {
      text: appliedQuery.trim(),
      kinds: selectedKind === undefined ? [] : [selectedKind],
      results: selectedResult === undefined ? [] : [selectedResult],
    };
  }, [appliedQuery, kind, result]);

  const load = useCallback(
    async (cursor?: string | null) => {
      setLoading(true);
      setError(null);
      try {
        const next = await api.journalPage({
          filter,
          cursor: cursor ?? null,
          pageSize: 200,
        });
        setEntries((current) =>
          cursor === undefined || cursor === null ? next.rows : [...current, ...next.rows],
        );
        setPage(next);
      } catch (cause) {
        setError(normalizeError(cause));
      } finally {
        setLoading(false);
      }
    },
    [filter],
  );

  // A new filter resets the cursor and reloads the first page.
  useEffect(() => {
    setEntries([]);
    setPage(null);
    void load();
  }, [load]);

  const act = async (entry: JournalEntryView) => {
    setError(null);
    try {
      if (entry.quarantine_ids.length > 0) {
        await api.journalRestore(entry.id);
      } else {
        await api.journalUndo(entry.id);
      }
      setNotice(t('journal.restored', { restored: 1, failed: 0 }));
      await load();
    } catch (cause) {
      setError(normalizeError(cause));
    }
  };

  const resetFilters = () => {
    setQuery('');
    setAppliedQuery('');
    setKind('');
    setResult('');
  };

  const columns: Column<JournalEntryView>[] = [
    {
      id: 'time',
      header: t('journal.time'),
      width: '180px',
      render: (row) => formatDate(row.occurred_at, language),
    },
    {
      id: 'user',
      header: t('journal.user'),
      width: '140px',
      render: (row) => row.user,
    },
    {
      id: 'kind',
      header: t('journal.kind_label'),
      width: '120px',
      render: (row) => t(`journal.kind.${row.kind}`),
    },
    {
      id: 'targets',
      header: t('journal.targets'),
      width: '100px',
      align: 'end',
      render: (row) => formatNumber(row.targets_count, language),
    },
    {
      id: 'size',
      header: t('top.size'),
      width: '120px',
      align: 'end',
      render: (row) => formatBytes(row.total_size, units),
    },
    {
      id: 'result',
      header: t('journal.result_label'),
      width: '120px',
      render: (row) => (
        <Badge
          tone={
            row.result === 'success'
              ? 'success'
              : row.result === 'partial'
                ? 'warning'
                : row.result === 'cancelled'
                  ? 'neutral'
                  : 'danger'
          }
        >
          {t(`journal.result.${row.result}`)}
        </Badge>
      ),
    },
    {
      id: 'actions',
      header: t('snapshots.actions'),
      width: '180px',
      render: (row) =>
        row.reversible ? (
          <Button variant="secondary" onClick={() => void act(row)}>
            {row.quarantine_ids.length > 0 ? t('journal.restore') : t('journal.undo')}
          </Button>
        ) : (
          <span>{t('journal.not_reversible')}</span>
        ),
    },
  ];

  return (
    <div className={styles.stack}>
      <Card title={t('nav.journal')}>
        <p>{t('journal.intro')}</p>
        <div className={styles.toolbar}>
          <SearchField
            label={t('journal.search')}
            placeholder={t('journal.search_placeholder')}
            value={query}
            onValueChange={setQuery}
          />
          <Select
            aria-label={t('journal.kind_label')}
            value={kind}
            options={[
              { value: '', label: t('journal.all_kinds') },
              ...KINDS.map((value) => ({ value, label: t(`journal.kind.${value}`) })),
            ]}
            onChange={(event) => setKind(event.target.value)}
          />
          <Select
            aria-label={t('journal.result_label')}
            value={result}
            options={[
              { value: '', label: t('journal.all_results') },
              ...RESULTS.map((value) => ({ value, label: t(`journal.result.${value}`) })),
            ]}
            onChange={(event) => setResult(event.target.value)}
          />
          <Button variant="ghost" onClick={resetFilters}>
            {t('filter.reset')}
          </Button>
          <span className={styles.spacer} />
          <Button variant="secondary" loading={loading} onClick={() => void load()}>
            {t('journal.refresh')}
          </Button>
        </div>
        {notice !== null ? <p role="status">{notice}</p> : null}
      </Card>

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void load()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      <Card title={t('journal.entries')}>
        <DataTable
          label={t('nav.journal')}
          rows={entries}
          columns={columns}
          rowKey={(row) => row.id}
          loading={loading}
          height={520}
          empty={
            loading ? (
              <Loading label={t('common.loading')} />
            ) : (
              <EmptyState title={t('journal.empty')} />
            )
          }
        />
        {page !== null && page.next_cursor !== null ? (
          <div className={styles.analyzeRow}>
            <Button
              variant="secondary"
              loading={loading}
              onClick={() => void load(page.next_cursor)}
            >
              {t('tree.load_more')}
            </Button>
          </div>
        ) : null}
      </Card>
    </div>
  );
}
