import { useEffect, useId, useState } from 'react';

import type { RenamePreview, UiError } from '../api/dto';
import { applyRename, requestRenamePreview } from '../api/filesystem';
import { normalizeError } from '../app/errors';
import { t } from '../i18n';
import { formatBytes, formatNumber } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { Button } from './Button';
import { Badge, Checkbox } from './Choice';
import { Dialog } from './Dialog';
import { Banner } from './Feedback';
import { Field } from './Fields';
import kit from './kit.module.css';
import styles from './RenameDialog.module.css';

/** The result handed to the tree so the row can be updated without a rescan. */
export interface RenamedObject {
  oldPath: string;
  newPath: string;
  newName: string;
}

export interface RenameDialogProps {
  open: boolean;
  /** Path of the object being renamed; `null` while the dialog is closed. */
  path: string | null;
  /** Current name, pre-filled into the field. */
  name: string | null;
  onClose: () => void;
  onRenamed: (result: RenamedObject) => void;
}

/**
 * Matches the whole file name, so the typed value becomes the new name. The
 * pattern is valid in both the Rust `regex` crate and the browser mock; Windows
 * names never contain a line break.
 */
const FULL_NAME = '^.*$';

/**
 * Rename of one object from the tree context menu.
 *
 * The typed value is never applied directly: the shared rename service builds a
 * preview first, reports conflicts and hands out a one-time token. The apply
 * button stays disabled until the user has seen the old → new pair and checked
 * the confirmation box (constitution, principle I).
 */
export function RenameDialog({
  open,
  path,
  name,
  onClose,
  onRenamed,
}: RenameDialogProps) {
  const fieldId = useId();
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');
  const [value, setValue] = useState(name ?? '');
  const [preview, setPreview] = useState<RenamePreview | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [loading, setLoading] = useState(false);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState<UiError | null>(null);

  useEffect(() => {
    if (!open) return;
    setValue(name ?? '');
    setPreview(null);
    setConfirmed(false);
    setLoading(false);
    setApplying(false);
    setError(null);
  }, [name, open, path]);

  const resetPreview = () => {
    setPreview(null);
    setConfirmed(false);
  };

  const buildPreview = async () => {
    if (path === null) return;
    const next = value.trim();
    if (next === '') {
      resetPreview();
      setError(validationError('rename.invalid'));
      return;
    }

    setLoading(true);
    setError(null);
    setConfirmed(false);
    try {
      setPreview(
        await requestRenamePreview({
          targets: [path],
          regex: FULL_NAME,
          replacement: escapeReplacement(next),
          scope: parentDirectory(path),
          case: 'preserve',
        }),
      );
    } catch (cause) {
      setError(normalizeError(cause));
      setPreview(null);
    } finally {
      setLoading(false);
    }
  };

  const apply = async () => {
    if (preview === null || !confirmed || applying) return;
    const row = preview.rows[0];
    if (row === undefined) return;

    setApplying(true);
    setError(null);
    try {
      await applyRename(preview.preview_id, preview.confirmation_token);
      onRenamed({
        oldPath: row.old_path,
        newPath: row.new_path,
        newName: row.new_name,
      });
      onClose();
    } catch (cause) {
      const normalized = normalizeError(cause);
      setError(normalized);
      setConfirmed(false);
      if (normalized.code === 'E_PREVIEW_EXPIRED') {
        setPreview(null);
      }
    } finally {
      setApplying(false);
    }
  };

  const blocked = preview !== null && preview.conflicts.length > 0;
  const noChange = preview !== null && !blocked && preview.change_count === 0;
  const canApply =
    preview !== null &&
    !blocked &&
    !noChange &&
    confirmed &&
    !loading &&
    !applying;

  return (
    <Dialog
      open={open}
      title={t('rename.object_title')}
      onClose={onClose}
      busy={applying}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={applying}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="primary"
            loading={applying}
            disabled={!canApply}
            onClick={() => void apply()}
          >
            {t('rename.apply')}
          </Button>
        </>
      }
    >
      <div className={styles.body}>
        <p className={styles.path} title={path ?? undefined}>
          {path}
        </p>

        <Field id={fieldId} label={t('rename.new_name')} required>
          <input
            id={fieldId}
            className={kit.input}
            value={value}
            spellCheck={false}
            autoComplete="off"
            data-autofocus
            disabled={applying}
            onChange={(event) => {
              setValue(event.target.value);
              resetPreview();
            }}
            onKeyDown={(event) => {
              if (event.key !== 'Enter') return;
              event.preventDefault();
              if (preview === null) {
                void buildPreview();
              } else if (canApply) {
                void apply();
              }
            }}
          />
        </Field>

        <div className={styles.actions}>
          <Button
            variant="secondary"
            loading={loading}
            disabled={applying}
            onClick={() => void buildPreview()}
          >
            {t('rename.preview')}
          </Button>
        </div>

        {error !== null ? (
          <Banner tone="error">{t(error.message_key)}</Banner>
        ) : null}

        {preview === null ? (
          <p className={styles.hint}>{t('rename.single_hint')}</p>
        ) : (
          <div className={styles.preview}>
            <div className={styles.previewHead}>
              <Badge tone={blocked ? 'danger' : 'success'}>
                {blocked
                  ? t('fileops.rename.blocked')
                  : t('fileops.rename.changes')}
              </Badge>
              <span className={styles.meta}>
                {formatNumber(preview.change_count, language)} ·{' '}
                {formatBytes(preview.total_size, units)}
              </span>
            </div>

            <dl className={styles.diff}>
              <div>
                <dt>{t('rename.old_name')}</dt>
                <dd>{preview.rows[0]?.old_name ?? name}</dd>
              </div>
              <div>
                <dt>{t('rename.new_name')}</dt>
                <dd>{preview.rows[0]?.new_name ?? value}</dd>
              </div>
            </dl>

            {noChange ? (
              <Banner tone="info">{t('rename.no_change')}</Banner>
            ) : null}
            {preview.requires_elevation ? (
              <Banner tone="warning">{t('confirm.elevation_required')}</Banner>
            ) : null}
            {preview.warnings.length > 0 ? (
              <ul className={styles.list}>
                {[...new Set(preview.warnings)].map((warning) => (
                  <li key={warning}>{t(warning)}</li>
                ))}
              </ul>
            ) : null}
            {blocked ? (
              <ul className={styles.list}>
                {preview.conflicts.map((conflict) => (
                  <li key={`${conflict.path}-${conflict.reason}`}>
                    {t(conflict.reason_key)}
                    {conflict.target === null ? '' : ` · ${conflict.target}`}
                  </li>
                ))}
              </ul>
            ) : null}

            <Checkbox
              id={`${fieldId}-confirm`}
              checked={confirmed}
              disabled={blocked || noChange || applying}
              label={t('rename.confirm')}
              onChange={(event) => {
                setConfirmed(event.target.checked);
              }}
            />
          </div>
        )}
      </div>
    </Dialog>
  );
}

/** `$` starts a group reference in the Rust replacement syntax. */
function escapeReplacement(name: string): string {
  return name.split('$').join('$$');
}

/** Directory that bounds the rename, in the form the service expects. */
function parentDirectory(path: string): string | null {
  const trimmed = path.replace(/\\+$/, '');
  const index = trimmed.lastIndexOf('\\');
  if (index < 0) return null;
  const parent = trimmed.slice(0, index);
  if (parent === '') return null;

  return /^[a-z]:$/i.test(parent) ? `${parent}\\` : parent;
}

function validationError(messageKey: string): UiError {
  return {
    code: 'E_VALIDATION',
    category: 'validation',
    message_key: messageKey,
    details: null,
    retryable: false,
    task_id: null,
    journal_entry_id: null,
  };
}
