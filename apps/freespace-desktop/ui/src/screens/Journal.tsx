import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

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
import { DataTable, type Column, type RowAnchor } from '../components/DataTable';
import { ConfirmDialog } from '../components/Dialog';
import { Banner, EmptyState, ErrorState, Loading } from '../components/Feedback';
import { SearchField, Select } from '../components/Fields';
import {
  HoverCard,
  HoverCardError,
  HoverCardHead,
  HoverCardRow,
  HoverCardRows,
} from '../components/HoverCard';
import { t } from '../i18n';
import { formatBytes, formatDateTimeShort, formatNumber } from '../i18n/format';
import { cx } from '../lib/cx';
import { useSettingsStore } from '../stores/settingsStore';
import { Panel, Screen, Stat, StatRow } from './ScreenFrame';
import styles from './operations.module.css';

const KINDS: readonly OperationKind[] = [
  'delete',
  'move',
  'rename',
  'permissions',
  'shred',
  'restore',
];
const RESULTS: readonly JournalResult[] = ['success', 'partial', 'failed', 'cancelled'];

/** Wait before the details card appears, so passing over rows does not flash it. */
const HOVER_DELAY = 450;

type Range = 'all' | 'day' | 'week' | 'month';

const RANGES: readonly Range[] = ['all', 'day', 'week', 'month'];
const RANGE_KEYS: Record<Range, string> = {
  all: 'operations.journal.range.all',
  day: 'operations.journal.range.day',
  week: 'operations.journal.range.week',
  month: 'operations.journal.range.month',
};

/** Result as coloured text; the word itself carries the meaning. */
const RESULT_CLASS: Record<JournalResult, string> = {
  success: styles.resultTextSuccess ?? '',
  partial: styles.resultTextWarning ?? '',
  failed: styles.resultTextDanger ?? '',
  cancelled: styles.resultTextNeutral ?? '',
};

interface PendingAction {
  entry: JournalEntryView;
  action: 'undo' | 'restore';
}
function inRange(timestamp: number, range: Range): boolean {
  if (range === 'all') return true;

  const now = new Date();
  if (range === 'day') {
    const date = new Date(timestamp);

    return (
      date.getFullYear() === now.getFullYear() &&
      date.getMonth() === now.getMonth() &&
      date.getDate() === now.getDate()
    );
  }

  const spanDays = range === 'week' ? 7 : 30;

  return now.getTime() - timestamp <= spanDays * 24 * 60 * 60 * 1000;
}

/** Small result label: the translated word in the tone of the outcome. */
function ResultBadge({ result }: { result: JournalResult }) {
  return (
    <span className={cx(styles.resultText, RESULT_CLASS[result])}>
      {t(`journal.result.${result}`)}
    </span>
  );
}

/**
 * Operation journal (T060).
 *
 * Filters (`kind`, `result`, date range and text) are applied to the request;
 * the range and facet checks are repeated client-side so a page that arrives
 * unfiltered (the mock transport, a stale cursor) never shows an excluded row.
 * Undo and restore both go through the shared confirmation dialog.
 */
export function Journal() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [entries, setEntries] = useState<JournalEntryView[]>([]);
  const [page, setPage] = useState<JournalPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<UiError | null>(null);
  const [query, setQuery] = useState('');
  const [appliedQuery, setAppliedQuery] = useState('');
  const [kind, setKind] = useState('');
  const [result, setResult] = useState('');
  const [range, setRange] = useState<Range>('all');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [hoverEntry, setHoverEntry] = useState<{
    entry: JournalEntryView;
    anchor: RowAnchor;
  } | null>(null);
  const hoverTimer = useRef<number | null>(null);

  useEffect(() => {
    if (query === appliedQuery) return undefined;
    const timer = setTimeout(() => {
      setAppliedQuery(query);
    }, 250);

    return () => {
      clearTimeout(timer);
    };
  }, [appliedQuery, query]);

  const closeHover = useCallback(() => {
    if (hoverTimer.current !== null) {
      window.clearTimeout(hoverTimer.current);
      hoverTimer.current = null;
    }
    setHoverEntry(null);
  }, []);

  /** Opens the details card after a short delay, like the tree rows do. */
  const openHover = useCallback((entry: JournalEntryView, anchor: RowAnchor) => {
    if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(() => {
      hoverTimer.current = null;
      setHoverEntry({ entry, anchor });
    }, HOVER_DELAY);
  }, []);

  useEffect(
    () => () => {
      if (hoverTimer.current !== null) window.clearTimeout(hoverTimer.current);
    },
    [],
  );

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
    setSelectedId(null);
    closeHover();
    void load();
  }, [closeHover, load]);

  const visible = useMemo(
    () =>
      entries.filter(
        (entry) =>
          inRange(entry.occurred_at, range) &&
          (kind === '' || entry.kind === kind) &&
          (result === '' || entry.result === result),
      ),
    [entries, kind, range, result],
  );

  const selected = useMemo(
    () => visible.find((entry) => entry.id === selectedId) ?? null,
    [selectedId, visible],
  );

  // A card that stays put while the row scrolls away would point at nothing.
  useEffect(() => {
    if (hoverEntry === null) return undefined;
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
  }, [closeHover, hoverEntry]);

  // Reloads and filters can drop the hovered row; its card must go with it.
  useEffect(() => {
    if (hoverEntry === null) return;
    if (!visible.some((entry) => entry.id === hoverEntry.entry.id)) closeHover();
  }, [closeHover, hoverEntry, visible]);

  const resetFilters = () => {
    setQuery('');
    setAppliedQuery('');
    setKind('');
    setResult('');
    setRange('all');
  };

  const startAction = (entry: JournalEntryView, action: 'undo' | 'restore') => {
    setNotice(null);
    closeHover();
    setPending({ entry, action });
  };

  const confirmAction = async () => {
    if (pending === null) return;
    setBusy(true);
    setError(null);
    try {
      if (pending.action === 'restore') {
        await api.journalRestore(pending.entry.id);
      } else {
        await api.journalUndo(pending.entry.id);
      }
      setNotice(
        pending.action === 'restore'
          ? t('journal.restored', { restored: 1, failed: 0 })
          : t('operations.journal.undone'),
      );
      setPending(null);
      closeHover();
      await load();
    } catch (cause) {
      setError(normalizeError(cause));
      setPending(null);
    } finally {
      setBusy(false);
    }
  };

  const successCount = visible.filter((entry) => entry.result === 'success').length;
  const problemCount = visible.filter(
    (entry) => entry.result === 'partial' || entry.result === 'failed',
  ).length;
  const visibleSize = visible.reduce((sum, entry) => sum + entry.total_size, 0);

  const columns: Column<JournalEntryView>[] = [
    {
      id: 'time',
      header: t('journal.time'),
      // Every column takes a share of the free width, so the table fills the
      // panel without one huge empty last column. `210px` is what
      // `DD/MM/YYYY HH:MM` needs at the table font size.
      width: 'minmax(210px, 1fr)',
      render: (row) => formatDateTimeShort(row.occurred_at),
    },
    {
      id: 'kind',
      header: t('journal.kind_label'),
      width: 'minmax(200px, 1.6fr)',
      render: (row) => t(`journal.kind.${row.kind}`),
    },
    {
      id: 'targets',
      header: t('journal.targets'),
      width: 'minmax(100px, 0.8fr)',
      align: 'end',
      render: (row) => formatNumber(row.targets_count, language),
    },
    {
      id: 'size',
      header: t('top.size'),
      width: 'minmax(120px, 0.9fr)',
      align: 'end',
      render: (row) => formatBytes(row.total_size, units),
    },
    {
      id: 'result',
      header: t('journal.result_label'),
      width: 'minmax(140px, 1fr)',
      render: (row) => <ResultBadge result={row.result} />,
    },
    {
      id: 'user',
      header: t('journal.user'),
      width: 'minmax(160px, 1.2fr)',
      render: (row) => row.user,
    },
  ];

  const filters = (
    <div className={styles.toolbar}>
      <SearchField
        className={styles.search}
        label={t('journal.search')}
        placeholder={t('journal.search_placeholder')}
        value={query}
        onValueChange={setQuery}
      />
      <Select
        className={styles.selectField}
        aria-label={t('journal.kind_label')}
        value={kind}
        options={[
          { value: '', label: t('journal.all_kinds') },
          ...KINDS.map((value) => ({ value, label: t(`journal.kind.${value}`) })),
        ]}
        onChange={(event) => {
          setKind(event.target.value);
        }}
      />
      <Select
        className={styles.selectField}
        aria-label={t('journal.result_label')}
        value={result}
        options={[
          { value: '', label: t('journal.all_results') },
          ...RESULTS.map((value) => ({ value, label: t(`journal.result.${value}`) })),
        ]}
        onChange={(event) => {
          setResult(event.target.value);
        }}
      />
      <Select
        className={styles.selectField}
        aria-label={t('operations.journal.range')}
        value={range}
        options={RANGES.map((value) => ({ value, label: t(RANGE_KEYS[value]) }))}
        onChange={(event) => {
          const next = RANGES.find((value) => value === event.target.value);
          if (next !== undefined) setRange(next);
        }}
      />
    </div>
  );

  return (
    <Screen
      context={t('operations.journal.context', {
        count: entries.length,
        reversible: entries.filter((entry) => entry.reversible).length,
      })}
      actions={
        <>
          <Button variant="ghost" onClick={resetFilters} disabled={loading}>
            {t('filter.reset')}
          </Button>
          <Button variant="secondary" loading={loading} onClick={() => void load()}>
            {t('journal.refresh')}
          </Button>
        </>
      }
    >
      <StatRow>
        <Stat label={t('journal.entries')} value={formatNumber(visible.length, language)} />
        <Stat label={t('journal.result.success')} value={formatNumber(successCount, language)} />
        <Stat
          label={t('operations.journal.problems')}
          value={formatNumber(problemCount, language)}
        />
        <Stat label={t('top.size')} value={formatBytes(visibleSize, units)} />
      </StatRow>

      {notice !== null ? <Banner tone="info">{notice}</Banner> : null}
      {problemCount > 0 ? (
        <Banner tone="warning">{t('operations.journal.partial_notice')}</Banner>
      ) : null}
      {error !== null && entries.length > 0 ? (
        <Banner
          tone="error"
          action={
            <Button variant="ghost" onClick={() => void load()}>
              {t('error.retry')}
            </Button>
          }
        >
          {t(error.message_key)}
        </Banner>
      ) : null}

      {error !== null && entries.length === 0 && !loading ? (
        <Panel title={t('journal.entries')} hint={t('operations.journal.filter_hint')}>
          {filters}
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
        </Panel>
      ) : (
        <Panel
          title={t('journal.entries')}
          hint={
            <>
              {t('operations.journal.filter_hint')}{' '}
              {t('operations.journal.hover_hint')}
            </>
          }
          actions={
            <>
              <span className={styles.meta}>
                {t('scan.objects', { count: visible.length })}
              </span>
              {selected === null ? null : selected.reversible ? (
                selected.quarantine_ids.length > 0 ? (
                  <Button
                    variant="primary"
                    onClick={() => {
                      startAction(selected, 'restore');
                    }}
                  >
                    {t('journal.restore')}
                  </Button>
                ) : (
                  <Button
                    variant="secondary"
                    onClick={() => {
                      startAction(selected, 'undo');
                    }}
                  >
                    {t('journal.undo')}
                  </Button>
                )
              ) : (
                <span className={styles.hint}>{t('journal.not_reversible')}</span>
              )}
            </>
          }
        >
          {filters}
          <DataTable
            label={t('nav.journal')}
            rows={visible}
            columns={columns}
            rowKey={(row) => row.id}
            selectedKey={selectedId}
            onSelect={(row) => {
              setSelectedId(row.id);
            }}
            loading={loading}
            height={460}
            onRowHoverStart={openHover}
            onRowHoverEnd={closeHover}
            empty={
              loading ? (
                <Loading label={t('common.loading')} />
              ) : (
                <EmptyState
                  title={t('journal.empty')}
                  description={t('operations.journal.empty_hint')}
                  actions={
                    <Button variant="secondary" onClick={resetFilters}>
                      {t('filter.reset')}
                    </Button>
                  }
                />
              )
            }
          />
          {page !== null && page.next_cursor !== null ? (
            <div className={styles.actions}>
              <Button
                variant="secondary"
                loading={loading}
                onClick={() => void load(page.next_cursor)}
              >
                {t('tree.load_more')}
              </Button>
            </div>
          ) : null}
        </Panel>
      )}

      {hoverEntry === null ? null : (
        <HoverCard anchor={hoverEntry.anchor} label={t('operations.journal.details')}>
          <JournalEntryDetails entry={hoverEntry.entry} />
        </HoverCard>
      )}

      <ConfirmDialog
        open={pending !== null}
        title={
          pending?.action === 'restore'
            ? t('operations.journal.restore_title')
            : t('operations.journal.undo_title')
        }
        description={
          pending === null
            ? ''
            : pending.action === 'restore'
              ? t('operations.journal.restore_hint', { count: pending.entry.targets_count })
              : t('operations.journal.undo_hint', { count: pending.entry.targets_count })
        }
        confirmLabel={
          pending?.action === 'restore' ? t('journal.restore') : t('journal.undo')
        }
        cancelLabel={t('common.cancel')}
        busy={busy}
        onCancel={() => {
          if (busy) return;
          setPending(null);
        }}
        onConfirm={() => void confirmAction()}
      />
    </Screen>
  );
}

/**
 * Read-only content of the journal row hover card.
 *
 * It replaces the former «Детали записи» side panel: the row data appears at
 * once and a failed entry also shows its translated message, stable code and
 * the raw detail text the core reported. Undo and restore stay on the entries
 * panel, where a selected row exposes them as buttons.
 */
function JournalEntryDetails({ entry }: { entry: JournalEntryView }) {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');

  return (
    <>
      <HoverCardHead title={t(`journal.kind.${entry.kind}`)} />
      <HoverCardRows>
        <HoverCardRow
          label={t('journal.time')}
          value={formatDateTimeShort(entry.occurred_at)}
        />
        <HoverCardRow label={t('journal.user')} value={entry.user} />
        <HoverCardRow
          label={t('journal.targets')}
          value={formatNumber(entry.targets_count, language)}
        />
        <HoverCardRow
          label={t('top.size')}
          value={formatBytes(entry.total_size, units)}
        />
        <HoverCardRow
          label={t('journal.result_label')}
          value={<ResultBadge result={entry.result} />}
        />
        <HoverCardRow
          label={t('operations.journal.quarantine')}
          value={formatNumber(entry.quarantine_ids.length, language)}
        />
        <HoverCardRow
          label={t('operations.journal.operation_id')}
          value={<span className={styles.mono}>{entry.operation_id}</span>}
        />
      </HoverCardRows>
      {entry.error === null ? null : (
        <HoverCardError
          title={t('operations.journal.error')}
          code={entry.error.code}
          message={t(entry.error.message_key)}
          details={entry.error.details}
        />
      )}
    </>
  );
}
