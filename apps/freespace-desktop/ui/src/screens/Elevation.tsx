import { useCallback, useEffect, useState } from 'react';

import type { ElevationStatus, UiError } from '../api/dto';
import { elevationRequest, elevationStatus } from '../api/settingsOps';
import { normalizeError } from '../app/errors';
import { Button } from '../components/Button';
import { Badge } from '../components/Choice';
import { ConfirmDialog } from '../components/Dialog';
import { Banner, ErrorState, Loading } from '../components/Feedback';
import { t } from '../i18n';
import { Panel } from './ScreenFrame';
import local from './settings-screen.module.css';

/**
 * Read-only elevation status with an explained, explicitly confirmed restart.
 *
 * Nothing on this tab changes files: a refusal only returns to the limited
 * mode described by the specification. The consequences of the restart are
 * spelled out before the request is sent, so the UAC prompt never appears
 * without the user having read them.
 */
export function Elevation() {
  const [status, setStatus] = useState<ElevationStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [requesting, setRequesting] = useState(false);
  const [confirming, setConfirming] = useState(false);
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
      setConfirming(false);
    } catch (cause) {
      setConfirming(false);
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

  const restartDisabled =
    status === null || status.elevated || !status.can_elevate || loading;

  return (
    <div className={local.page}>
      <Panel title={t('elevation.reason')} hint={t('elevation.intro')}>
        <p className={local.note}>{reasonText}</p>

        {loading ? (
          <Loading label={t('common.loading')} rows={2} />
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

        <div className={local.section}>
          <h3 className={local.sectionTitle}>{t('elevation.status')}</h3>
          {status === null ? null : (
            <div className={local.statusRow}>
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
        </div>

        <div className={local.section}>
          <h3 className={local.sectionTitle}>{t('elevation.protected')}</h3>
          <p className={local.note}>{t('elevation.protected_hint')}</p>
          <div className={local.footer}>
            <Button
              variant="primary"
              loading={requesting}
              disabled={restartDisabled}
              onClick={() => {
                setConfirming(true);
              }}
            >
              {t('elevation.restart')}
            </Button>
          </div>
        </div>
      </Panel>

      <ConfirmDialog
        open={confirming}
        title={t('system.elevation.restart_title')}
        description={t('system.elevation.restart_consequences')}
        confirmLabel={t('system.elevation.restart_confirm')}
        cancelLabel={t('system.elevation.restart_cancel')}
        busy={requesting}
        onConfirm={() => void request()}
        onCancel={() => {
          setConfirming(false);
        }}
      />
    </div>
  );
}
