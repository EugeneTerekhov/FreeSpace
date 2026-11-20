import { useState } from 'react';
import { Archive, Lock } from 'lucide-react';

import type { UiError } from '../api/dto';
import type {
  ArchiveEntry,
  ArchiveEntryPage,
  ArchiveExportFormat,
} from '../api/dto-inspection';
import {
  ARCHIVE_EXPORT_FORMATS,
  exportArchiveList,
  fetchArchiveEntries,
} from '../api/inspection';
import { normalizeError, validationError } from '../app/errors';
import { Badge } from '../components/Choice';
import { Button } from '../components/Button';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState } from '../components/Feedback';
import { Field, Select } from '../components/Fields';
import { t } from '../i18n';
import { formatBytes, formatNumber, formatPercent } from '../i18n/format';
import { emptyFilter } from '../lib/filters';
import { useSettingsStore } from '../stores/settingsStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import local from './inspect.module.css';

const ARCHIVE_PAGE_SIZE = 8;

const FORMAT_LABELS: Record<ArchiveExportFormat, string> = {
  csv: 'CSV',
  json: 'JSON',
  xml: 'XML',
  html: 'HTML',
};

/** Containers that are themselves archives, listed without extracting them. */
const NESTED_EXTENSIONS = new Set([
  'zip',
  '7z',
  'rar',
  'tar',
  'gz',
  'tgz',
  'bz2',
  'xz',
  'iso',
  'cab',
]);

function isNestedArchive(entry: ArchiveEntry): boolean {
  const extension = entry.extension ?? '';

  return extension !== '' && NESTED_EXTENSIONS.has(extension);
}

/**
 * Read-only archive contents with paging, a nested-archive marker and a list
 * export.
 *
 * Nothing is unpacked and nothing is written inside the archive: the backend
 * only enumerates the container, so browsing a 480 000-entry 7z is as safe as
 * reading a directory listing.
 */
export function Archives() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [path, setPath] = useState('');
  const [format, setFormat] = useState<ArchiveExportFormat>('csv');
  const [output, setOutput] = useState('');
  const [page, setPage] = useState<ArchiveEntryPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const inspect = async (cursor: string | null = null) => {
    const archive = path.trim();
    if (archive === '') {
      setError(validationError(t('archives.empty')));
      return;
    }

    setLoading(true);
    setError(null);
    setNotice(null);
    try {
      const result = await fetchArchiveEntries({
        path: archive,
        filter: emptyFilter(),
        cursor,
        pageSize: ARCHIVE_PAGE_SIZE,
      });
      setPage((current) =>
        cursor === null || current === null
          ? result
          : { ...result, entries: [...current.entries, ...result.entries] },
      );
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setLoading(false);
    }
  };

  const exportList = async () => {
    if (page === null) return;
    const out = output.trim();
    if (out === '') {
      setError(validationError(t('archives.export_path')));
      return;
    }

    setExporting(true);
    setError(null);
    setNotice(null);
    try {
      const result = await exportArchiveList({
        path: page.path,
        format,
        out,
      });
      setNotice(t('archives.exported', { path: result.path }));
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setExporting(false);
    }
  };

  const entryColumns: Column<ArchiveEntry>[] = [
    {
      id: 'name',
      header: t('tree.sort.name'),
      width: 'minmax(260px, 2fr)',
      render: (entry) => (
        <span className={local.archiveName}>
          <span className={local.pathCell} title={entry.internal_path}>
            {entry.internal_path}
            {entry.is_directory ? '\\' : ''}
          </span>
          {isNestedArchive(entry) ? (
            <Badge tone="info">
              <span className={local.status}>
                <Archive size={12} aria-hidden="true" />
                <span>{t('inspect.archives.nested')}</span>
              </span>
            </Badge>
          ) : null}
          {entry.encrypted ? (
            <Badge tone="warning">
              <span className={local.status}>
                <Lock size={12} aria-hidden="true" />
                <span>{t('common.warning')}</span>
              </span>
            </Badge>
          ) : null}
        </span>
      ),
    },
    {
      id: 'size',
      header: t('top.size'),
      width: '100px',
      align: 'end',
      render: (entry) => formatBytes(entry.size, units),
    },
    {
      id: 'packed',
      header: t('inspect.archives.packed'),
      width: '112px',
      align: 'end',
      render: (entry) =>
        entry.compressed_size === null
          ? t('metadata.none')
          : formatBytes(entry.compressed_size, units),
    },
    {
      id: 'ratio',
      header: t('inspect.archives.ratio'),
      width: '92px',
      align: 'end',
      render: (entry) => {
        const compressed = entry.compressed_size ?? entry.size;
        const ratio = entry.size === 0 ? 0 : compressed / entry.size;

        return formatPercent(ratio, language, 0);
      },
    },
    {
      id: 'share',
      header: t('owners.share'),
      width: '92px',
      align: 'end',
      render: (entry) => formatPercent(entry.share, language, 1),
    },
  ];

  const largest = page?.entries.slice(0, 5) ?? [];
  const archiveTotal = page?.total_size ?? 0;

  return (
    <div className={local.stack}>
      <Panel title={t('nav.archives')} hint={t('inspect.archives.hint')}>
        <form
          className={local.toolbar}
          onSubmit={(event) => {
            event.preventDefault();
            void inspect();
          }}
        >
          <Field id="archives-path" label={t('archives.path')}>
            <input
              id="archives-path"
              className={local.input}
              value={path}
              spellCheck={false}
              onChange={(event) => {
                setPath(event.target.value);
                setPage(null);
                setError(null);
                setNotice(null);
              }}
            />
          </Field>
          <Button type="submit" variant="primary" loading={loading}>
            {t('archives.inspect')}
          </Button>
        </form>
      </Panel>

      {error !== null ? (
        <ErrorState
          title={t('archives.error')}
          code={error.code}
          description={t(error.message_key)}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void inspect()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {notice !== null ? <Banner tone="info">{notice}</Banner> : null}

      {page === null ? (
        !loading && error === null ? (
          <EmptyState title={t('archives.empty')} description={t('archives.intro')} />
        ) : null
      ) : (
        <>
          <StatRow>
            <Stat
              label={t('inspect.archives.entries')}
              value={formatNumber(page.total_entries, language)}
            />
            <Stat
              label={t('inspect.archives.files')}
              value={formatNumber(page.total_files, language)}
            />
            <Stat
              label={t('top.size')}
              value={formatBytes(page.total_size, units)}
            />
            <Stat
              label={t('inspect.archives.encrypted')}
              value={formatNumber(page.encrypted_count, language)}
            />
          </StatRow>

          {page.warnings.length > 0 ? (
            <Banner tone="warning">
              {t('inspect.archives.partial', { count: page.warnings.length })}
            </Banner>
          ) : null}

          <Panel
            title={<span className={local.pathCell}>{page.path}</span>}
            actions={<Badge tone="info">{page.format.toUpperCase()}</Badge>}
          >
            <div className={local.metaRow}>
              <span>{t('archives.entries', { count: formatNumber(page.total_entries, language) })}</span>
              <span>{t('archives.total', { size: formatBytes(page.total_size, units) })}</span>
            </div>
            {page.warnings.length > 0 ? (
              <ul className={local.warnings}>
                {page.warnings.map((warning) => (
                  <li key={warning}>{t(warning)}</li>
                ))}
              </ul>
            ) : null}
            <DataTable
              label={t('archives.archive')}
              rows={page.entries}
              columns={entryColumns}
              rowKey={(entry) => entry.internal_path}
              loading={loading}
              height={360}
              empty={t('archives.empty')}
            />
            {page.next_cursor !== null ? (
              <div className={local.actions}>
                <Button
                  variant="secondary"
                  loading={loading}
                  onClick={() => void inspect(page.next_cursor)}
                >
                  {t('tree.load_more')}
                </Button>
              </div>
            ) : null}
          </Panel>

          {largest.length > 0 ? (
            <Panel title={t('archives.largest')}>
              <table className={local.table} aria-label={t('archives.largest')}>
                <thead>
                  <tr>
                    <th scope="col">{t('tree.sort.name')}</th>
                    <th scope="col" className={local.numeric}>
                      {t('top.size')}
                    </th>
                    <th scope="col" className={local.numeric}>
                      {t('owners.share')}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {largest.map((entry) => (
                    <tr key={entry.internal_path}>
                      <td className={local.pathCell}>{entry.internal_path}</td>
                      <td className={local.numeric}>{formatBytes(entry.size, units)}</td>
                      <td className={local.numeric}>
                        {formatPercent(
                          archiveTotal === 0 ? 0 : entry.size / archiveTotal,
                          language,
                          1,
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </Panel>
          ) : null}

          <Panel title={t('archives.export')}>
            <div className={local.filterGrid}>
              <Field id="archives-format" label={t('export.format')}>
                <Select
                  id="archives-format"
                  value={format}
                  options={ARCHIVE_EXPORT_FORMATS.map((value) => ({
                    value,
                    label: FORMAT_LABELS[value],
                  }))}
                  onChange={(event) => {
                    setFormat(event.target.value as ArchiveExportFormat);
                  }}
                />
              </Field>
              <Field id="archives-output" label={t('archives.export_path')}>
                <input
                  id="archives-output"
                  className={local.input}
                  value={output}
                  spellCheck={false}
                  onChange={(event) => {
                    setOutput(event.target.value);
                  }}
                />
              </Field>
            </div>
            <div className={local.actions}>
              <Button
                variant="secondary"
                loading={exporting}
                disabled={page.entries.length === 0}
                onClick={() => void exportList()}
              >
                {t('archives.export')}
              </Button>
              <span className={local.spacer} />
              <span className={local.metaRow}>
                {t('archives.entries', {
                  count: formatNumber(page.total_entries, language),
                })}
              </span>
            </div>
          </Panel>
        </>
      )}
    </div>
  );
}
