import { useId, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

import { useFocusTrap } from '../app/focus';
import { t } from '../i18n';
import { Button, IconButton } from './Button';
import styles from './kit.module.css';

export interface DialogProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  busy?: boolean;
}

/**
 * Modal dialog: focus trap, `[data-autofocus]` initial focus, Escape handling
 * and focus restoration to the element that opened it.
 */
export function Dialog({
  open,
  title,
  onClose,
  children,
  footer,
  busy = false,
}: DialogProps) {
  const titleId = useId();
  const close = useRef(onClose);
  close.current = onClose;
  const dialog = useFocusTrap<HTMLDivElement>(open, {
    onEscape: () => {
      close.current();
    },
    escapeDisabled: busy,
  });

  if (!open) return null;

  return createPortal(
    <div
      className={styles.overlay}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) {
          onClose();
        }
      }}
    >
      <div
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-busy={busy || undefined}
        tabIndex={-1}
        className={styles.dialog}
      >
        <header className={styles.dialogHeader}>
          <h2 id={titleId} className={styles.dialogTitle}>
            {title}
          </h2>
          <IconButton
            label={t('common.close')}
            disabled={busy}
            onClick={() => {
              onClose();
            }}
          >
            <span aria-hidden="true">×</span>
          </IconButton>
        </header>
        <div className={styles.dialogBody}>{children}</div>
        {footer !== undefined ? (
          <footer className={styles.dialogFooter}>{footer}</footer>
        ) : null}
      </div>
    </div>,
    document.body,
  );
}

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description: ReactNode;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  destructive?: boolean;
  busy?: boolean;
  error?: string;
}

/** Confirmation with visible consequences; destructive actions need a danger button. */
export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  destructive = false,
  busy = false,
  error,
}: ConfirmDialogProps) {
  return (
    <Dialog
      open={open}
      title={title}
      onClose={onCancel}
      busy={busy}
      footer={
        <>
          <Button variant="ghost" onClick={onCancel} disabled={busy}>
            {cancelLabel}
          </Button>
          <Button
            variant={destructive ? 'danger' : 'primary'}
            onClick={onConfirm}
            loading={busy}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className={styles.dialogBody}>
        <p>{description}</p>
      </div>
      {error !== undefined ? (
        <div className={styles.warningText} role="alert">
          <span>{error}</span>
        </div>
      ) : null}
    </Dialog>
  );
}
