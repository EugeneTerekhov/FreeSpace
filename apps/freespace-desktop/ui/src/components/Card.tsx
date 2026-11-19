import type { HTMLAttributes, ReactNode } from 'react';

import { cx } from '../lib/cx';
import styles from './kit.module.css';

export interface CardProps extends Omit<HTMLAttributes<HTMLElement>, 'title'> {
  title?: ReactNode;
  actions?: ReactNode;
  interactive?: boolean;
  selected?: boolean;
  children?: ReactNode;
}

/** Raised surface with an optional title and actions. */
export function Card({
  title,
  actions,
  interactive = false,
  selected = false,
  className,
  children,
  ...rest
}: CardProps) {
  return (
    <article
      {...rest}
      className={cx(
        styles.card,
        interactive && styles.interactive,
        selected && styles.selected,
        className,
      )}
    >
      {title !== undefined || actions !== undefined ? (
        <div className={styles.cardHeader}>
          {title !== undefined ? <h2 className={styles.cardTitle}>{title}</h2> : null}
          {actions}
        </div>
      ) : null}
      {children !== undefined ? <div className={styles.cardBody}>{children}</div> : null}
    </article>
  );
}

export interface StatCardProps {
  label: string;
  value: ReactNode;
  trend?: string;
  hint?: string;
}

/** Large number with a label and an optional trend. */
export function StatCard({ label, value, trend, hint }: StatCardProps) {
  return (
    <div className={cx(styles.card, styles.statCard)}>
      <span className={styles.statLabel}>{label}</span>
      <span className={styles.statValue}>{value}</span>
      {trend !== undefined ? <span className={styles.statTrend}>{trend}</span> : null}
      {hint !== undefined ? <span className={styles.statTrend}>{hint}</span> : null}
    </div>
  );
}

export interface SummaryCardProps {
  title: ReactNode;
  children: ReactNode;
  actions?: ReactNode;
  selected?: boolean;
}

/** Card variant used by the dashboard bento grid. */
export function SummaryCard({ title, children, actions, selected = false }: SummaryCardProps) {
  return (
    <Card title={title} actions={actions} selected={selected}>
      {children}
    </Card>
  );
}
