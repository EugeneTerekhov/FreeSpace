import type { NodeRow, UiError } from '../api/dto';
import { t } from '../i18n';
import { formatBytes, formatDateTimeCompact } from '../i18n/format';
import { useSettingsStore } from '../stores/settingsStore';
import { Button } from './Button';
import { Dialog } from './Dialog';
import { Banner } from './Feedback';
import { isVolumeRow } from './TreeRow';
import styles from './DeleteObjectDialog.module.css';

export interface DeleteObjectDialogProps {
  open: boolean;
  /** Row whose deletion was requested. */
  target: NodeRow | null;
  /** Size measured by the live preview; falls back to the snapshot row. */
  size?: number | null;
  busy?: boolean;
  error?: UiError | null;
  onCancel: () => void;
  onConfirm: () => void;
}

/**
 * Compact confirmation for the tree context menu.
 *
 * The menu only recycles, so the dialog asks one question and shows the object
 * identity: name, type, size and modification date. Multi-object previews and
 * the deletion-mode choice remain on the screens that offer them.
 */
export function DeleteObjectDialog({
  open,
  target,
  size = null,
  busy = false,
  error = null,
  onCancel,
  onConfirm,
}: DeleteObjectDialogProps) {
  const language = useSettingsStore((state) => state.settings?.language ?? 'ru');
  const units = useSettingsStore((state) => state.settings?.units ?? 'binary');

  if (target === null) return null;

  const kind = isVolumeRow(target)
    ? t('kind.volume')
    : t(`kind.${target.kind}`);
  const question =
    target.kind === 'directory'
      ? t('tree2.delete_question_dir')
      : t('tree2.delete_question_file');

  return (
    <Dialog
      open={open}
      title={t('tree2.delete_title')}
      onClose={onCancel}
      busy={busy}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onCancel} disabled={busy}>
            {t('common.cancel')}
          </Button>
          <Button variant="danger" loading={busy} onClick={onConfirm}>
            {t('tree2.action.delete')}
          </Button>
        </>
      }
    >
      <div className={styles.body}>
        <p className={styles.question}>{question}</p>

        <dl className={styles.details}>
          <div>
            <dt>{t('tree2.name')}</dt>
            <dd title={target.path}>{target.name}</dd>
          </div>
          <div>
            <dt>{t('details.kind')}</dt>
            <dd>{kind}</dd>
          </div>
          <div>
            <dt>{t('details.size')}</dt>
            <dd>{formatBytes(size ?? target.on_disk_size, units)}</dd>
          </div>
          <div>
            <dt>{t('details.modified')}</dt>
            <dd>{formatDateTimeCompact(target.modified_at, language)}</dd>
          </div>
        </dl>

        <p className={styles.hint}>{t('tree2.delete_recycle_hint')}</p>

        {error !== null ? (
          <Banner tone="error">{t(error.message_key)}</Banner>
        ) : null}
      </div>
    </Dialog>
  );
}
