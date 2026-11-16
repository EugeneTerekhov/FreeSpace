import { useEffect, useId, useState } from 'react';

import type { DeleteMode, OperationPreview, UiError } from '../api/dto';
import { normalizeError } from '../app/errors';
import { Button } from './Button';
import { Badge, Checkbox, Radio } from './Choice';
import { Dialog } from './Dialog';
import { Banner } from './Feedback';
import { t } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import styles from './OperationPreviewDialog.module.css';

const MODES: DeleteMode[] = ['recycle_bin', 'quarantine', 'permanent'];

export interface OperationPreviewDialogProps {
  open: boolean;
  preview: OperationPreview | null;
  busy?: boolean;
  error: UiError | null;
  onCancel: () => void;
  onConfirm: (mode: DeleteMode, extraConfirmation: boolean) => void;
}

/** Shared preview shown before every destructive action. */
export function OperationPreviewDialog({
  open,
  preview,
  busy = false,
  error,
  onCancel,
  onConfirm,
}: OperationPreviewDialogProps) {
  const groupName = useId();
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const [mode, setMode] = useState<DeleteMode>('recycle_bin');
  const [acknowledged, setAcknowledged] = useState(false);
  const [extra, setExtra] = useState(false);

  useEffect(() => {
    if (preview === null) return;
    setMode(preview.delete_mode ?? 'recycle_bin');
    setAcknowledged(false);
    setExtra(false);
  }, [preview]);

  if (preview === null) return null;

  const irreversible =
    mode === 'permanent' || preview.kind === 'shred' || preview.requires_elevation;
  const canConfirm = acknowledged && (!irreversible || extra) && !busy;
  const normalized = error === null ? null : normalizeError(error);

  return (
    <Dialog
      open={open}
      title={t('confirm.title')}
      onClose={onCancel}
      busy={busy}
      footer={
        <>
          <Button variant="ghost" onClick={onCancel} disabled={busy}>
            {t('common.cancel')}
          </Button>
          <Button
            variant={irreversible ? 'danger' : 'primary'}
            loading={busy}
            disabled={!canConfirm}
            onClick={() => {
              onConfirm(mode, extra);
            }}
          >
            {t('confirm.apply')}
          </Button>
        </>
      }
    >
      <div className={styles.body}>
        <p className={styles.summary}>
          {t('confirm.summary', {
            count: formatNumber(preview.targets.length, language),
            size: formatBytes(preview.total_size, units),
          })}
        </p>

        <fieldset className={styles.modes}>
          <legend>{t('confirm.mode')}</legend>
          {MODES.map((item) => (
            <Radio
              key={item}
              id={`${groupName}-${item}`}
              name={groupName}
              checked={mode === item}
              disabled={busy}
              label={
                <span className={styles.modeRow}>
                  <span>{t(`delete.mode.${item}`)}</span>
                  {item === 'permanent' ? <Badge tone="danger">{t('confirm.irreversible')}</Badge> : null}
                </span>
              }
              onChange={() => {
                setMode(item);
                setAcknowledged(false);
                setExtra(false);
              }}
            />
          ))}
        </fieldset>

        {preview.requires_elevation ? (
          <Banner tone="warning">{t('confirm.elevation_required')}</Banner>
        ) : null}

        {preview.warnings.length > 0 ? (
          <ul className={styles.warnings}>
            {[...new Set(preview.warnings)].map((warning) => (
              <li key={warning}>{t(warning)}</li>
            ))}
          </ul>
        ) : null}

        <div className={styles.targets} role="region" aria-label={t('confirm.targets')}>
          <table className={styles.table}>
            <thead>
              <tr>
                <th scope="col">{t('confirm.target')}</th>
                <th scope="col">{t('details.kind')}</th>
                <th scope="col" className={styles.numeric}>
                  {t('details.size')}
                </th>
              </tr>
            </thead>
            <tbody>
              {preview.targets.slice(0, 50).map((target) => (
                <tr key={target.path}>
                  <td className={styles.path} title={target.path}>
                    {target.path}
                  </td>
                  <td>{target.is_system ? t('confirm.system') : target.kind}</td>
                  <td className={styles.numeric}>{formatBytes(target.size, units)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {preview.targets.length > 50 ? (
            <p className={styles.more}>
              {t('confirm.more_targets', { count: preview.targets.length - 50 })}
            </p>
          ) : null}
        </div>

        <Checkbox
          id={`${groupName}-ack`}
          checked={acknowledged}
          disabled={busy}
          label={t('confirm.checkbox')}
          onChange={(event) => {
            setAcknowledged(event.target.checked);
          }}
        />
        {irreversible ? (
          <Checkbox
            id={`${groupName}-extra`}
            checked={extra}
            disabled={busy}
            label={t('confirm.extra')}
            onChange={(event) => {
              setExtra(event.target.checked);
            }}
          />
        ) : null}

        {normalized !== null ? (
          <Banner tone="error">{t(normalized.message_key)}</Banner>
        ) : null}
      </div>
    </Dialog>
  );
}
