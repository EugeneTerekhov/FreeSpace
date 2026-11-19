import { useCallback, useEffect, useState } from 'react';

import type { AboutInfo, LicenseInfo, UiError } from '../api/dto';
import { aboutInfo, aboutLicenses } from '../api/settingsOps';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { DataTable, type Column } from '../components/DataTable';
import { Banner, EmptyState, ErrorState, Loading } from '../components/Feedback';
import { t } from '../i18n';
import { Panel } from './ScreenFrame';
import screen from './screen.module.css';
import local from './settings-screen.module.css';

/**
 * Version, build, WebView2, data directory and third-party licences.
 *
 * URLs are rendered as text on purpose: the client never opens a network
 * resource and never launches an external browser by itself.
 */
export function About() {
  const [info, setInfo] = useState<AboutInfo | null>(null);
  const [licenses, setLicenses] = useState<LicenseInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<UiError | null>(null);
  const [copied, setCopied] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [about, list] = await Promise.all([aboutInfo(), aboutLicenses()]);
      setInfo(about);
      setLicenses(list.length > 0 ? list : about.licenses);
    } catch (cause) {
      setError(normalizeError(cause));
      setInfo(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const copy = async () => {
    if (info === null) return;
    try {
      if (navigator.clipboard !== undefined) {
        await navigator.clipboard.writeText(info.data_dir);
      }
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };

  const columns: Column<LicenseInfo>[] = [
    {
      id: 'component',
      header: t('about.component'),
      width: 'minmax(180px, 1fr)',
      render: (row) => row.name,
    },
    {
      id: 'license',
      header: t('about.license'),
      width: '180px',
      render: (row) => row.license,
    },
    {
      id: 'source',
      header: t('about.source'),
      width: 'minmax(180px, 2fr)',
      render: (row) =>
        row.url === null ? (
          <span>{t('metadata.none')}</span>
        ) : (
          <span className={local.mono}>{row.url}</span>
        ),
    },
  ];

  return (
    <div className={local.page}>
      {loading ? (
        <Panel title={t('about.title')}>
          <Loading label={t('common.loading')} rows={3} />
        </Panel>
      ) : null}

      {error !== null ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" loading={loading} onClick={() => void load()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {info !== null ? (
        <>
          <Panel title={t('about.title')}>
            <p className={screen.panelHint}>{t('about.intro')}</p>
            <Banner tone="info">{t('about.offline')}</Banner>
            <dl className={local.specList}>
              <dt className={local.specTerm}>{t('about.version')}</dt>
              <dd className={local.specValue}>{info.version}</dd>
              <dt className={local.specTerm}>{t('about.build')}</dt>
              <dd className={local.specValue}>{info.build}</dd>
              <dt className={local.specTerm}>{t('about.webview')}</dt>
              <dd className={local.specValue}>
                {info.webview_version ?? t('metadata.none')}
              </dd>
            </dl>
          </Panel>

          <Panel title={t('about.data_dir')}>
            <div className={local.copyRow}>
              <code className={local.code}>{info.data_dir}</code>
              <Button variant="secondary" onClick={() => void copy()}>
                {t('about.copy')}
              </Button>
            </div>
            {copied ? <Banner tone="info">{t('about.copied')}</Banner> : null}
          </Panel>

          <Panel title={t('about.licenses')}>
            <DataTable
              label={t('about.licenses')}
              rows={licenses}
              columns={columns}
              rowKey={(row) => row.name}
              height={licenses.length > 0 ? 320 : 160}
              empty={<EmptyState title={t('about.licenses')} />}
            />
          </Panel>
        </>
      ) : null}
    </div>
  );
}
