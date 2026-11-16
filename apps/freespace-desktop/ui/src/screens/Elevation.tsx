import { useCallback, useEffect, useState } from 'react';

import type { ElevationStatus, UiError } from '../api/dto';
import { elevationRequest, elevationStatus } from '../api/settingsOps';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { Badge } from '../components/Choice';
import { Banner, ErrorState, Loading } from '../components/Feedback';
import { t } from '../i18n';
import local from './settings.module.css';

/**
 * Read-only elevation status with an explained request.
 *
 * Nothing on this screen changes files: a refusal only returns to the limited
 * mode described by the specification.
 */
export function Elevation() {
  const [status, setStatus] = useState<ElevationStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [requesting, setRequesting] = useState(false);
  const [denied, setDenied] = useState(false);
  const [error, setError] = useState<UiError | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setStatus(await elevationStatus());
    } catch (cause) {
      setError(normalizeError(cause));
      setStatus(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const request = async () => {
    if (status === null) return;
    setRequesting(true);
    setDenied(false);
    setError(null);
    try {
      const result = await elevationRequest(
        status.reason_key ?? 'elevation.protected_hint',
      );
      setStatus({
        elevated: result.elevated,
        can_elevate: false,
        reason_key: status.reason_key,
      });
    } catch (cause) {
      const normalized = normalizeError(cause);
      if (normalized.code === 'E_ELEVATION_DENIED') {
        setDenied(true);
      } else {
        setError(normalized);
      }
    } finally {
      setRequesting(false);
    }
  };

  const reasonText =
    status?.reason_key === null || status?.reason_key === undefined
      ? t('elevation.intro')
      : t(status.reason_key) === status.reason_key
        ? t('elevation.intro')
        : t(status.reason_key);

  return (
    <div className={local.page}>
      <Card title={t('nav.elevation')}>
        <p className={local.note}>{t('elevation.intro')}</p>

        {loading ? (
          <Loading label={t('common.loading')} rows={2} />
        ) : status === null ? null : (
          <div className={local.statusRow}>
            <span className={local.note}>{t('elevation.status')}</span>
            <Badge tone={status.elevated ? 'success' : 'warning'}>
              {t(status.elevated ? 'elevation.elevated' : 'elevation.limited')}
            </Badge>
            {status.elevated ? null : (
              <Badge tone={status.can_elevate ? 'info' : 'neutral'}>
                {t(status.can_elevate ? 'elevation.available' : 'elevation.denied')}
              </Badge>
            )}
          </div>
        )}

        {denied ? <Banner tone="warning">{t('elevation.declined')}</Banner> : null}
      </Card>

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

      <Card title={t('elevation.reason')}>
        <p className={local.note}>{reasonText}</p>
      </Card>

      <Card title={t('elevation.protected')}>
        <p className={local.note}>{t('elevation.protected_hint')}</p>
        <div className={local.footer}>
          <Button
            variant="primary"
            loading={requesting}
            disabled={status === null || status.elevated || !status.can_elevate || loading}
            onClick={() => void request()}
          >
            {t('elevation.request')}
          </Button>
          {status !== null && !status.elevated ? (
            <span className={local.hint}>{t('elevation.restart')}</span>
          ) : null}
        </div>
      </Card>
    </div>
  );
}
