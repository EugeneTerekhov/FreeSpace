import { useCallback, useEffect, useState } from 'react';

import { api } from '../api/runtime';
import type { UiError, VolumeSummary } from '../api/dto';
import { Badge } from '../components/Choice';
import { Button } from '../components/Button';
import { Card, StatCard } from '../components/Card';
import { Banner, EmptyState, ErrorState, Loading, Meter } from '../components/Feedback';
import { normalizeError } from '../app/errors';
import { t } from '../i18n';
import { formatBytes, formatPercent } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import styles from './screens.module.css';

/** Volume dashboard with quick scan actions. */
export function Dashboard() {
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [volumes, setVolumes] = useState<VolumeSummary[] | null>(null);
  const [error, setError] = useState<UiError | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      setVolumes(await api.volumesRefresh());
    } catch (cause) {
      setError(normalizeError(cause));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const analyze = async (volume: VolumeSummary) => {
    setBusy(volume.volume_id);
    setError(null);
    try {
      await api.scanStart({
        path: volume.mount_point,
        method: 'auto',
        incremental: false,
        exclusions: [],
        include_ads: false,
        metric: 'on_disk',
      });
      useShellStore.getState().setScreen('scan');
    } catch (cause) {
      setError(normalizeError(cause));
    } finally {
      setBusy(null);
    }
  };

  if (error !== null && volumes === null) {
    return (
      <ErrorState
        title={t(error.message_key)}
        code={error.code}
        details={error.details ?? undefined}
        actions={
          <Button
            variant="primary"
            onClick={() => {
              void refresh();
            }}
          >
            {t('error.retry')}
          </Button>
        }
      />
    );
  }

  if (volumes === null) {
    return <Loading label={t('common.loading')} rows={4} />;
  }

  if (volumes.length === 0) {
    return (
      <EmptyState
        title={t('dashboard.no_volumes')}
        description={t('dashboard.no_volumes_hint')}
        actions={
          <Button
            variant="primary"
            onClick={() => {
              void refresh();
            }}
          >
            {t('dashboard.refresh')}
          </Button>
        }
      />
    );
  }

  const total = volumes.reduce((sum, volume) => sum + volume.total_bytes, 0);
  const used = volumes.reduce((sum, volume) => sum + volume.used_bytes, 0);
  const free = volumes.reduce((sum, volume) => sum + volume.free_bytes, 0);

  return (
    <div className={styles.stack}>
      <div className={styles.toolbar}>
        <p>{t('dashboard.intro')}</p>
        <span className={styles.spacer} />
        <Button
          variant="secondary"
          onClick={() => {
            void refresh();
          }}
        >
          {t('dashboard.refresh')}
        </Button>
      </div>

      {error !== null ? (
        <Banner tone="error" action={<Button variant="ghost" onClick={() => void refresh()}>{t('error.retry')}</Button>}>
          {t(error.message_key)}
        </Banner>
      ) : null}

      <section className={styles.stats} aria-label={t('dashboard.volumes')}>
        <StatCard label={t('dashboard.total')} value={formatBytes(total, units)} />
        <StatCard label={t('dashboard.used')} value={formatBytes(used, units)} />
        <StatCard label={t('dashboard.free')} value={formatBytes(free, units)} />
        <StatCard
          label={t('dashboard.volumes')}
          value={volumes.length}
          trend={`${formatPercent(total === 0 ? 0 : used / total, language)} ${t('dashboard.usage').toLowerCase()}`}
        />
      </section>

      <section className={styles.volumeGrid} aria-label={t('dashboard.volumes')}>
        {volumes.map((volume) => {
          const ratio =
            volume.total_bytes === 0 ? 0 : volume.used_bytes / volume.total_bytes;

          return (
            <Card
              key={volume.volume_id}
              title={
                <span className={styles.volumeHeader}>
                  <span className="mono">{volume.mount_point}</span>
                  <span>{volume.label}</span>
                </span>
              }
              actions={
                <Badge tone={volume.status === 'available' ? 'success' : 'warning'}>
                  {t(`volume.status.${volume.status}`)}
                </Badge>
              }
            >
              <div className={styles.stack}>
                <Meter
                  label={t('dashboard.usage')}
                  value={Math.round(ratio * 100)}
                  caption={formatPercent(ratio, language)}
                />
                <div className={styles.metaRow}>
                  <span>
                    {t('dashboard.total')}: <strong>{formatBytes(volume.total_bytes, units)}</strong>
                  </span>
                  <span>
                    {t('dashboard.used')}: <strong>{formatBytes(volume.used_bytes, units)}</strong>
                  </span>
                  <span>
                    {t('dashboard.free')}: <strong>{formatBytes(volume.free_bytes, units)}</strong>
                  </span>
                  <span>
                    {t('dashboard.filesystem')}: <strong>{volume.file_system}</strong>
                  </span>
                  {volume.scan_method !== null ? (
                    <span>
                      {t('scan.method_applied')}: <strong>{volume.scan_method}</strong>
                    </span>
                  ) : null}
                </div>
                <div className={styles.analyzeRow}>
                  <Button
                    variant="primary"
                    loading={busy === volume.volume_id}
                    disabled={volume.status !== 'available'}
                    onClick={() => {
                      void analyze(volume);
                    }}
                  >
                    {t('dashboard.analyze')}
                  </Button>
                </div>
              </div>
            </Card>
          );
        })}
      </section>
    </div>
  );
}
