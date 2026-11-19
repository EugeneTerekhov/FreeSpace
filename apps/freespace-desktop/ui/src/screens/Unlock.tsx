import { useState } from 'react';
import { ShieldAlert } from 'lucide-react';

import type { UiError } from '../api/dto';
import type { FileLock } from '../api/dto-inspection';
import {
  fetchLocks,
  releaseLocks,
  scheduleUnlockReboot,
} from '../api/inspection';
import { normalizeError, validationError } from '../app/errors';
import { Badge, Checkbox } from '../components/Choice';
import { Button } from '../components/Button';
import { Dialog } from '../components/Dialog';
import { Banner, EmptyState, ErrorState, Progress } from '../components/Feedback';
import { Field } from '../components/Fields';
import { t } from '../i18n';
import { formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { useShellStore } from '../stores/shellStore';
import { Panel, Stat, StatRow } from './ScreenFrame';
import local from './inspect.module.css';

type PendingAction = 'soft' | 'force' | 'reboot';

const ACTION_LABEL: Record<PendingAction, string> = {
  soft: 'unlock.soft',
  force: 'unlock.force',
  reboot: 'unlock.reboot',
};

/**
 * Read-only lock diagnostics with guarded soft, force and reboot actions.
 *
 * Listing handles never changes the file. Every release goes through one
 * confirmation dialog: `soft` asks applications to close, `force` terminates
 * the selected processes and `reboot` registers deletion before the next
 * sign-in, which is the only safe path for critical handles.
 */
export function Unlock() {
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [path, setPath] = useState('');
  const [locks, setLocks] = useState<FileLock[]>([]);
  const [selected, setSelected] = useState<ReadonlySet<number>>(new Set());
  const [inspected, setInspected] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<UiError | null>(null);
  const [elevationRequired, setElevationRequired] = useState(false);
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [acknowledged, setAcknowledged] = useState(false);
  const [result, setResult] = useState<'released' | 'scheduled' | null>(null);

  const inspect = async () => {
    const target = path.trim();
    if (target === '') {
      setError(validationError(t('details.path_required')));
      return;
    }

    setLoading(true);
    setError(null);
    setElevationRequired(false);
    setResult(null);
    setSelected(new Set());
    try {
      const found = await fetchLocks(target);
      setLocks(found);
      setInspected(true);
    } catch (cause) {
      const normalized = normalizeError(cause);
      setError(normalized);
      setLocks([]);
      setElevationRequired(normalized.code === 'E_ELEVATION_REQUIRED');
    } finally {
      setLoading(false);
    }
  };

  const openConfirm = (action: PendingAction) => {
    setPending(action);
    setAcknowledged(false);
    setError(null);
    setElevationRequired(false);
  };

  const closeConfirm = () => {
    setPending(null);
    setAcknowledged(false);
  };

  const apply = async () => {
    if (pending === null) return;
    setBusy(true);
    setError(null);

    try {
      if (pending === 'reboot') {
        await scheduleUnlockReboot(path.trim());
        setResult('scheduled');
      } else {
        const processIds =
          pending === 'soft' ? locks.map((lock) => lock.process_id) : [...selected];
        await releaseLocks(path.trim(), pending, processIds);
        setLocks([]);
        setSelected(new Set());
        setResult('released');
      }
      closeConfirm();
    } catch (cause) {
      const normalized = normalizeError(cause);
      setError(normalized);
      setElevationRequired(normalized.code === 'E_ELEVATION_REQUIRED');
      closeConfirm();
    } finally {
      setBusy(false);
    }
  };

  const canConfirm = pending === 'soft' || acknowledged;
  const pendingTitle = pending === null ? '' : t(ACTION_LABEL[pending]);
  const critical = locks.filter((lock) => lock.critical).length;
  const reboot = locks.filter((lock) => lock.requires_reboot).length;
  const blockedSoft = locks.some((lock) => lock.critical && !lock.can_soft_close);

  return (
    <div className={local.stack}>
      <Panel title={t('nav.unlock')} hint={t('inspect.unlock.hint')}>
        <form
          className={local.toolbar}
          onSubmit={(event) => {
            event.preventDefault();
            void inspect();
          }}
        >
          <Field id="unlock-path" label={t('unlock.path')}>
            <input
              id="unlock-path"
              className={local.input}
              value={path}
              spellCheck={false}
              onChange={(event) => {
                setPath(event.target.value);
                setLocks([]);
                setInspected(false);
                setResult(null);
                setError(null);
                setElevationRequired(false);
              }}
            />
          </Field>
          <Button type="submit" variant="primary" loading={loading}>
            {t('unlock.inspect')}
          </Button>
        </form>
        {loading ? <Progress label={t('unlock.inspect')} state="indeterminate" /> : null}
      </Panel>

      {elevationRequired ? (
        <Banner
          tone="warning"
          action={
            <Button
              variant="ghost"
              onClick={() => {
                useShellStore.getState().setScreen('settings');
              }}
            >
              {t('elevation.request')}
            </Button>
          }
        >
          {t('confirm.elevation_required')}
        </Banner>
      ) : null}

      {error !== null && !elevationRequired ? (
        <ErrorState
          title={t(error.message_key)}
          code={error.code}
          details={error.details ?? undefined}
          actions={
            <Button variant="primary" onClick={() => void inspect()}>
              {t('error.retry')}
            </Button>
          }
        />
      ) : null}

      {result === 'released' ? <Banner tone="info">{t('unlock.released')}</Banner> : null}
      {result === 'scheduled' ? <Banner tone="info">{t('unlock.scheduled')}</Banner> : null}

      {inspected && locks.length === 0 && result === null && error === null ? (
        <EmptyState title={t('unlock.no_locks')} description={t('unlock.available')} />
      ) : null}

      {locks.length > 0 ? (
        <>
          <StatRow>
            <Stat
              label={t('inspect.unlock.processes')}
              value={formatNumber(locks.length, language)}
            />
            <Stat
              label={t('inspect.unlock.critical')}
              value={formatNumber(critical, language)}
            />
            <Stat label={t('inspect.unlock.reboot')} value={formatNumber(reboot, language)} />
          </StatRow>

          {blockedSoft ? (
            <Banner tone="warning">
              <span className={local.status}>
                <ShieldAlert size={14} aria-hidden="true" />
                <span>{t('inspect.unlock.partial')}</span>
              </span>
            </Banner>
          ) : null}

          <Panel
            title={t('inspect.unlock.locks')}
            actions={
              result === 'scheduled' ? (
                <Badge tone="warning">{t('unlock.needs_reboot')}</Badge>
              ) : (
                <Badge tone="danger">{t('unlock.locked')}</Badge>
              )
            }
          >
            <table className={local.table} aria-label={t('inspect.unlock.locks')}>
              <thead>
                <tr>
                  <th scope="col">
                    <span className={local.visuallyHidden}>{t('tree.selected')}</span>
                  </th>
                  <th scope="col">{t('unlock.process')}</th>
                  <th scope="col">{t('unlock.pid')}</th>
                  <th scope="col">{t('unlock.type')}</th>
                  <th scope="col">{t('unlock.service')}</th>
                  <th scope="col">{t('unlock.state')}</th>
                </tr>
              </thead>
              <tbody>
                {locks.map((lock) => (
                  <tr key={lock.process_id}>
                    <td>
                      <Checkbox
                        id={`unlock-lock-${lock.process_id}`}
                        checked={selected.has(lock.process_id)}
                        disabled={lock.critical}
                        label={
                          <span className={local.visuallyHidden}>
                            {`${t('unlock.process')}: ${lock.process_name}`}
                          </span>
                        }
                        onChange={(event) => {
                          setSelected((current) => {
                            const next = new Set(current);
                            if (event.target.checked) next.add(lock.process_id);
                            else next.delete(lock.process_id);
                            return next;
                          });
                        }}
                      />
                    </td>
                    <td>{lock.process_name}</td>
                    <td className={local.numeric}>{lock.process_id}</td>
                    <td>{lock.application_type}</td>
                    <td>{lock.service_name === '' ? t('metadata.none') : lock.service_name}</td>
                    <td>
                      {lock.critical ? (
                        <Badge tone="warning">{t('unlock.critical')}</Badge>
                      ) : lock.requires_reboot ? (
                        <Badge tone="warning">{t('unlock.needs_reboot')}</Badge>
                      ) : (
                        t('unlock.locked')
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>

            <div className={local.toolbar}>
              <Button
                variant="secondary"
                disabled={busy || !locks.some((lock) => lock.can_soft_close)}
                onClick={() => {
                  openConfirm('soft');
                }}
              >
                {t('unlock.soft')}
              </Button>
              <Button
                variant="danger"
                disabled={busy || selected.size === 0}
                onClick={() => {
                  openConfirm('force');
                }}
              >
                {t('unlock.force')}
              </Button>
              <Button
                variant="danger"
                disabled={busy}
                onClick={() => {
                  openConfirm('reboot');
                }}
              >
                {t('unlock.reboot')}
              </Button>
              <span className={local.spacer} />
              <span className={local.metaRow}>
                {formatNumber(selected.size, language)} / {formatNumber(locks.length, language)}
              </span>
            </div>
          </Panel>
        </>
      ) : null}

      <Dialog
        open={pending !== null}
        title={pendingTitle}
        onClose={closeConfirm}
        busy={busy}
        footer={
          <>
            <Button variant="ghost" disabled={busy} onClick={closeConfirm}>
              {t('common.cancel')}
            </Button>
            <Button
              variant={pending === 'soft' ? 'primary' : 'danger'}
              loading={busy}
              disabled={!canConfirm}
              onClick={() => void apply()}
            >
              {pendingTitle}
            </Button>
          </>
        }
      >
        <p>{t('unlock.intro')}</p>
        <p className={local.mono}>{path.trim()}</p>
        {pending !== null && pending !== 'soft' ? (
          <Checkbox
            id="unlock-confirm"
            checked={acknowledged}
            label={
              pending === 'force' ? t('unlock.confirm_force') : t('unlock.confirm_reboot')
            }
            onChange={(event) => {
              setAcknowledged(event.target.checked);
            }}
          />
        ) : null}
        {pending === 'force' ? (
          <p className={local.metaRow}>
            {formatNumber(selected.size, language)} / {formatNumber(locks.length, language)}
          </p>
        ) : null}
      </Dialog>
    </div>
  );
}
