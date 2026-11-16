import type { ButtonHTMLAttributes, ReactNode } from 'react';

import { cx } from '../lib/cx';
import styles from './kit.module.css';

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
type ButtonSize = 'sm' | 'md';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  icon?: ReactNode;
}

const VARIANT_CLASS: Record<ButtonVariant, string> = {
  primary: styles.primary ?? '',
  secondary: styles.secondary ?? '',
  ghost: styles.ghost ?? '',
  danger: styles.danger ?? '',
};

/** Primary action button with loading and destructive variants. */
export function Button({
  variant = 'secondary',
  size = 'md',
  loading = false,
  icon,
  children,
  className,
  disabled,
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      className={cx(
        styles.button,
        VARIANT_CLASS[variant],
        size === 'sm' && styles.small,
        size === 'md' && styles.large,
        className,
      )}
      disabled={disabled === true || loading}
      aria-busy={loading || undefined}
    >
      {loading ? <span className={styles.spinner} aria-hidden="true" /> : icon}
      {children}
    </button>
  );
}

export interface IconButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  /** Accessible name; icon-only buttons always need it. */
  label: string;
  children: ReactNode;
}

/** Square icon-only button; the label is announced by screen readers. */
export function IconButton({
  label,
  children,
  className,
  type = 'button',
  ...rest
}: IconButtonProps) {
  return (
    <button
      {...rest}
      type={type}
      className={cx(styles.iconButton, className)}
      aria-label={label}
      title={label}
    >
      {children}
    </button>
  );
}
