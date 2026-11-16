import type { ReactNode } from 'react';

import { VISUALLY_HIDDEN_CLASS } from '../app/focus';
import { cx } from '../lib/cx';
import styles from './kit.module.css';

export interface ProgressProps {
  value?: number;
  label: string;
  state?: 'normal' | 'indeterminate' | 'error' | 'cancelled';
}

/** Determinate or indeterminate progress bar. */
export function Progress({ value, label, state = 'normal' }: ProgressProps) {
  const indeterminate = state === 'indeterminate' || value === undefined;
  const clamped = Math.max(0, Math.min(100, value ?? 0));

  return (
    <div
      className={cx(
        styles.progress,
        indeterminate && styles.indeterminate,
        state === 'error' && styles.progressError,
      )}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : clamped}
      aria-valuetext={indeterminate ? label : `${Math.round(clamped)}%`}
    >
      <div
        className={styles.progressBar}
        style={indeterminate ? undefined : { width: `${clamped}%` }}
      />
    </div>
  );
}

export interface MeterProps {
  label: string;
  value: number;
  max?: number;
  caption?: string;
}

/** Level meter with a textual caption and a warning/danger tone. */
export function Meter({ label, value, max = 100, caption }: MeterProps) {
  const ratio = max <= 0 ? 0 : Math.max(0, Math.min(1, value / max));
  const tone = ratio >= 0.9 ? 'danger' : ratio >= 0.75 ? 'warning' : 'normal';

  return (
    <div
      className={cx(
        styles.meter,
        tone === 'warning' && styles.meterWarning,
        tone === 'danger' && styles.meterDanger,
      )}
      role="meter"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={value}
    >
      <div className={styles.meterLabel}>
        <span>{label}</span>
        <span>{caption ?? `${Math.round(ratio * 100)}%`}</span>
      </div>
      <div className={styles.meterTrack}>
        <div className={styles.meterFill} style={{ width: `${ratio * 100}%` }} />
      </div>
    </div>
  );
}

export interface BannerProps {
  tone?: 'info' | 'warning' | 'error';
  children: ReactNode;
  action?: ReactNode;
}

/** Icon per tone so status is never conveyed by colour alone. */
const TONE_ICON: Record<'info' | 'warning' | 'error', string> = {
  info: 'ℹ',
  warning: '⚠',
  error: '✕',
};

/** Inline notice; errors are announced immediately. */
export function Banner({ tone = 'info', children, action }: BannerProps) {
  return (
    <div
      className={cx(
        styles.banner,
        tone === 'warning' && styles.bannerWarning,
        tone === 'error' && styles.bannerError,
      )}
      role={tone === 'error' ? 'alert' : 'status'}
    >
      <span className={styles.bannerIcon} aria-hidden="true">
        {TONE_ICON[tone]}
      </span>
      <span>{children}</span>
      {action !== undefined ? <span className={styles.toastAction}>{action}</span> : null}
    </div>
  );
}

export interface EmptyStateProps {
  title: string;
  description?: string;
  icon?: ReactNode;
  actions?: ReactNode;
}

/** Explains why a screen has no data and what to do next. */
export function EmptyState({ title, description, icon, actions }: EmptyStateProps) {
  return (
    <div className={styles.emptyState}>
      {icon !== undefined ? <div className={styles.emptyIcon}>{icon}</div> : null}
      <p className={styles.emptyTitle}>{title}</p>
      {description !== undefined ? (
        <p className={styles.emptyDescription}>{description}</p>
      ) : null}
      {actions !== undefined ? <div className={styles.emptyActions}>{actions}</div> : null}
    </div>
  );
}

export interface ErrorStateProps {
  title: string;
  code?: string | undefined;
  description?: string | undefined;
  details?: string | undefined;
  actions?: ReactNode;
}

/** Recoverable or fatal error with the stable code and an optional detail. */
export function ErrorState({ title, code, description, details, actions }: ErrorStateProps) {
  return (
    <div className={styles.errorState} role="alert">
      <span className={styles.errorIcon} aria-hidden="true">
        ✕
      </span>
      {code !== undefined ? <span className={styles.errorCode}>{code}</span> : null}
      <p className={styles.emptyTitle}>{title}</p>
      {description !== undefined ? (
        <p className={styles.emptyDescription}>{description}</p>
      ) : null}
      {details !== undefined ? <code className={styles.errorDetails}>{details}</code> : null}
      {actions !== undefined ? <div className={styles.emptyActions}>{actions}</div> : null}
    </div>
  );
}

/** Accessible loading placeholder for a section or a page. */
export function Loading({ label = 'Loading…', rows = 3 }: { label?: string; rows?: number }) {
  return (
    <div aria-busy="true" aria-label={label}>
      {Array.from({ length: rows }, (_, index) => (
        <div key={index} className={cx(styles.skeleton, styles.skeletonRow)} />
      ))}
      <span className={styles.loadingLabel}>{label}</span>
    </div>
  );
}

/** Single skeleton block. */
export function Skeleton({ height = 12, width = '100%' }: { height?: number; width?: string }) {
  return (
    <div
      className={styles.skeleton}
      style={{ height, width }}
      aria-hidden="true"
    />
  );
}

export type ToastSeverity = 'info' | 'success' | 'warning' | 'error';

export interface ToastProps {
  severity?: ToastSeverity;
  message: ReactNode;
  action?: ReactNode;
}

/** One toast; the region owns the live announcement. */
export function Toast({ severity = 'info', message, action }: ToastProps) {
  return (
    <div
      className={cx(
        styles.toast,
        severity === 'success' && styles.toastSuccess,
        severity === 'warning' && styles.toastWarning,
        severity === 'error' && styles.toastError,
      )}
    >
      <span>{message}</span>
      {action !== undefined ? <span className={styles.toastAction}>{action}</span> : null}
    </div>
  );
}

/** Polite live region for toasts. */
export function ToastRegion({ children }: { children: ReactNode }) {
  return (
    <div className={styles.toastRegion} aria-live="polite" aria-atomic="false">
      {children}
    </div>
  );
}

export interface LiveRegionsProps {
  /** Throttled polite text, usually from `useLiveAnnouncer`. */
  polite: string;
  /** Assertive text reserved for errors that need a decision. */
  assertive: string;
}

/**
 * Visually hidden polite and assertive regions.
 *
 * They are rendered once per surface (shell status bar, operation panel) so a
 * message is announced from a region that is already in the accessibility tree.
 */
export function LiveRegions({ polite, assertive }: LiveRegionsProps) {
  return (
    <>
      <div
        className={VISUALLY_HIDDEN_CLASS}
        aria-live="polite"
        aria-atomic="true"
      >
        {polite}
      </div>
      <div
        className={VISUALLY_HIDDEN_CLASS}
        aria-live="assertive"
        aria-atomic="true"
      >
        {assertive}
      </div>
    </>
  );
}
