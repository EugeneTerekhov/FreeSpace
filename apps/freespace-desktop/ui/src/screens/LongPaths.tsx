import { useCallback, useEffect, useState } from 'react';

import type {
  FilterSet,
  LongPathValidation,
  NodeRow,
  OperationKind,
  TreePage,
  UiError,
} from '../api/dto';
import { fetchLongPathsPage, validateLongPath } from '../api/filesystem';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Badge } from '../components/Choice';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, Loading } from '../components/Feedback';
import { Field } from '../components/Fields';
import { FilterBar } from '../components/FilterBar';
import { OperationPreviewDialog } from '../components/OperationPreviewDialog';
import { OperationProgressView } from '../components/OperationProgress';
import { t } from '../i18n';
import { formatBytes, formatDate } from '../i18n/format';
import { emptyFilter } from '../lib/filters';
import { useOperation } from '../operations/useOperation';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { useSnapshotStore } from '../stores/snapshotStore';
import styles from './screens.module.css';
import local from './filesystem.module.css';

/** Objects beyond the 260-character limit with rename, move, copy and delete. */
export function LongPaths() {
  const generation = useSnapshotStore((state) => state.generation);
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [filter, setFilter] = useState<FilterSet>(emptyFilter());
  const [appliedFilter] = useDebounced(filter);
  const [rows, setRows] = useState<NodeRow[]>([]);
  const [page, setPage] = useState<TreePage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [selected, setSelected] = useState<NodeRow | null>(null);
  const [path, setPath] = useState('');
  const [newName, setNewName] = useState('');
  const [destination, setDestination] = useState('');
  const [validation, setValidation] = useState<LongPathValidation | null>(null);
  const [validating, setValidating] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const operation = useOperation();

  const load = useCallback(
    async (cursor?: string | null) => {
      if (generation === null) return;
      setLoading(true);
      setError(null);
      try {
        const result = await fetchLongPathsPage(generation, {
          filter: appliedFilter,
          cursor: cursor ?? null,
          pageSize: 50,
        });
        setRows((current) =>
          cursor === undefined || cursor === null ? result.rows : [...current, ...result.rows],
        );
        setPage(result);
        setSelected((current) =>
          current === null
            ? null
            : result.rows.find((row) => row.id === current.id) ?? current,
        );
      } catch (cause) {
        setError(normalizeError(cause));
      } finally {
        setLoading(false);
      }
    },
    [appliedFilter, generation],
  );

  useEffect(() => {
    setRows([]);
    setPage(null);
    if (generation !== null) void load();
  }, [generation, load]);

  const choose = (row: NodeRow) => {
    setSelected(row);
    setPath(row.path);
    setNewName(row.name);
    setValidation(null);
    setFormError(null);
  };

  const checkPath = async () => {
    const trimmed = path.trim();
    if (trimmed === '') {
      setFormError('long_paths.path_required');
      setValidation(null);
      return;
    }
    setFormError(null);
    setValidating(true);
    try {
      setValidation(await validateLongPath(trimmed));
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setValidating(false);
    }
  };

  const openPreview = (kind: OperationKind, reason: string | null = null) => {
    const target = path.trim();
    if (target === '') {
      setFormError('long_paths.path_required');
      return;
    }
    setFormError(null);
    void operation.prepare({ kind, targets: [target], reason });
  };

  const rename = () => {
    if (newName.trim() === '') {
      setFormError('error.validation');
      return;
    }
    openPreview('rename', newName.trim());
  };

  const move = (kind: 'move') => {
    if (destination.trim() === '') {
      setFormError('error.validation');
      return;
    }
    openPreview(kind, destination.trim());
  };

  const columns: Column<NodeRow>[] = [
    {
      id: 'path',
      header: t('long_paths.path'),
      width: 'minmax(320px, 2fr)',
      render: (row) => (
        <span className={local.pathText} title={row.path}>
          {row.path}
        </span>
      ),
    },
    {
      id: 'kind',
      header: t('details.kind'),
      width: '100px',
      render: (row) => t(`kind.${row.kind}`),
    },
    {
      id: 'size',
      header: t('details.size'),
      width: '120px',
      align: 'end',
      render: (row) => formatBytes(row.logical_size, units),
    },
    {
      id: 'modified',
      header: t('top.modified'),
      width: '180px',
      render: (row) => formatDate(row.modified_at, language),
    },
  ];

  if (generation === null) {
    return (
      <EmptyState
        title={t('analysis.scan_required')}
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

  return (
    <div className={styles.stack}>
      <Card title={t('nav.long_paths')}>
        <p className={local.note}>{t('long_paths.intro')}</p>
        <div className={styles.toolbar}>
          <Field id="long-paths-path" label={t('long_paths.path')}>
            <input
              id="long-paths-path"
              className={local.input}
              value={path}
              spellCheck={false}
              onChange={(event) => {
                setPath(event.target.value);
                setValidation(null);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  void checkPath();
                }
              }}
            />
          </Field>
          <Button variant="secondary" loading={validating} onClick={() => void checkPath()}>
            {t('details.inspect')}
          </Button>
          {path !== '' ? <p className={local.mono}>{path}</p> : null}
        </div>

        {validation !== null ? (
          <Banner tone={validation.valid ? 'info' : 'error'}>
            {validation.valid
              ? t('status.ready')
              : t(validation.reason_key ?? 'error.validation')}
            {validation.display !== null || validation.normalized !== null
              ? ` · ${validation.display ?? validation.normalized ?? ''}`
              : ''}
          </Banner>
        ) : null}

        <div className={local.form}>
          <Field id="long-paths-name" label={t('long_paths.new_name')}>
            <input
              id="long-paths-name"
              className={local.input}
              value={newName}
              onChange={(event) => {
                setNewName(event.target.value);
              }}
            />
          </Field>
          <div className={local.actions}>
            <Button variant="secondary" onClick={rename}>
              {t('long_paths.rename')}
            </Button>
          </div>
        </div>

        <div className={local.form}>
          <Field id="long-paths-destination" label={t('long_paths.destination')}>
            <input
              id="long-paths-destination"
              className={local.input}
              value={destination}
              spellCheck={false}
              onChange={(event) => {
                setDestination(event.target.value);
              }}
            />
          </Field>
          <div className={local.actions}>
            <Button variant="secondary" onClick={() => move('move')}>
              {t('long_paths.move')}
            </Button>
            <Button variant="secondary" onClick={() => move('move')}>
              {t('long_paths.copy')}
            </Button>
            <span className={styles.spacer} />
            <Button variant="danger" onClick={() => openPreview('delete')}>
              {t('long_paths.delete')}
            </Button>
          </div>
        </div>

        {formError !== null ? (
          <Banner tone="error">{t(formError)}</Banner>
        ) : null}

        {selected !== null ? (
          <div className={local.statusRow}>
            <Badge tone="info">{t(`kind.${selected.kind}`)}</Badge>
            <span className={local.note}>{t('tree.selected')}</span>
          </div>
        ) : null}
      </Card>

      <FilterBar screen="long_paths" filter={filter} onChange={setFilter} />

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

      {operation.error !== null && operation.preview === null ? (
        <Banner tone="error">{t(operation.error.message_key)}</Banner>
      ) : null}

      <Card title={t('nav.long_paths')}>
        <DataTable
          label={t('nav.long_paths')}
          rows={rows}
          columns={columns}
          rowKey={(row) => row.id}
          selectedKey={selected?.id ?? null}
          onSelect={choose}
          loading={loading}
          height={420}
          empty={
            loading ? (
              <Loading label={t('common.loading')} />
            ) : (
              <EmptyState title={t('tree.no_matches')} />
            )
          }
        />
        {page !== null && page.next_cursor !== null ? (
          <div className={styles.analyzeRow}>
            <Button variant="secondary" loading={loading} onClick={() => void load(page.next_cursor)}>
              {t('tree.load_more')}
            </Button>
          </div>
        ) : null}
      </Card>

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

function useDebounced(filter: FilterSet): [FilterSet] {
  const [value, setValue] = useState(filter);
  useEffect(() => {
    if (value === filter) return undefined;
    const timer = setTimeout(() => {
      setValue(filter);
    }, 250);

    return () => {
      clearTimeout(timer);
    };
  }, [filter, value]);

  return [value];
}
