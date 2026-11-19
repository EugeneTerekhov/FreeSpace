import { useRef, type KeyboardEvent, type ReactNode } from 'react';

import { t } from '../i18n';
import styles from './screen.module.css';

export interface ScreenProps {
  /** One line of context under the header: scope, counts, freshness. */
  context?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}

/**
 * Common screen body: context line, contextual actions and the main sections.
 *
 * The screen title itself is the shell `h1`, so every screen keeps exactly one
 * top-level heading (contract §5.2).
 */
export function Screen({ context, actions, children }: ScreenProps) {
  return (
    <div className={styles.screen}>
      {context !== undefined || actions !== undefined ? (
        <div className={styles.screenHead}>
          {context !== undefined ? (
            <p className={styles.screenContext}>{context}</p>
          ) : null}
          {actions !== undefined ? (
            <div className={styles.screenActions}>{actions}</div>
          ) : null}
        </div>
      ) : null}
      {children}
    </div>
  );
}

export interface PanelProps {
  title?: ReactNode;
  actions?: ReactNode;
  hint?: ReactNode;
  children: ReactNode;
  className?: string;
}

/** Raised section: a title row, optional actions and the content. */
export function Panel({ title, actions, hint, children, className }: PanelProps) {
  return (
    <section className={className === undefined ? styles.panel : `${styles.panel} ${className}`}>
      {title !== undefined || actions !== undefined ? (
        <div className={styles.panelHead}>
          {title !== undefined ? <h2 className={styles.panelTitle}>{title}</h2> : null}
          {actions !== undefined ? (
            <span className={styles.screenActions}>{actions}</span>
          ) : null}
        </div>
      ) : null}
      {children}
      {hint !== undefined ? <p className={styles.panelHint}>{hint}</p> : null}
    </section>
  );
}

export interface StatProps {
  label: ReactNode;
  value: ReactNode;
  className?: string;
}

/** One headline number with its caption. */
export function Stat({ label, value, className }: StatProps) {
  return (
    <div className={className === undefined ? styles.stat : `${styles.stat} ${className}`}>
      <span className={styles.statLabel}>{label}</span>
      <span className={styles.statValue}>{value}</span>
    </div>
  );
}

export function StatRow({ children }: { children: ReactNode }) {
  return <div className={styles.statRow}>{children}</div>;
}

export interface TabItem {
  id: string;
  label: string;
}

export interface TabsProps {
  items: readonly TabItem[];
  active: string;
  onChange: (id: string) => void;
  /** Stable prefix for the tab/panel ids; must match `TabPanel`. */
  idBase: string;
  label?: string;
}

/**
 * Horizontal tabs with roving focus: arrows move between tabs, `Home`/`End`
 * jump to the edges, and both `aria-selected` and `tabIndex` follow the active
 * tab.
 */
export function Tabs({
  items,
  active,
  onChange,
  idBase,
  label = t('tabs.label'),
}: TabsProps) {
  const refs = useRef(new Map<string, HTMLButtonElement>());

  const select = (index: number) => {
    const item = items[(index + items.length) % items.length];
    if (item === undefined) return;
    onChange(item.id);
    refs.current.get(item.id)?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const index = items.findIndex((item) => item.id === active);
    if (index === -1) return;

    switch (event.key) {
      case 'ArrowRight':
        event.preventDefault();
        select(index + 1);
        break;
      case 'ArrowLeft':
        event.preventDefault();
        select(index - 1);
        break;
      case 'Home':
        event.preventDefault();
        select(0);
        break;
      case 'End':
        event.preventDefault();
        select(items.length - 1);
        break;
      default:
        break;
    }
  };

  return (
    <div className={styles.tabs} role="tablist" aria-label={label} onKeyDown={onKeyDown}>
      {items.map((item) => {
        const selected = item.id === active;

        return (
          <button
            key={item.id}
            ref={(node) => {
              if (node === null) {
                refs.current.delete(item.id);
              } else {
                refs.current.set(item.id, node);
              }
            }}
            type="button"
            role="tab"
            id={`${idBase}-tab-${item.id}`}
            aria-selected={selected}
            aria-controls={`${idBase}-panel-${item.id}`}
            tabIndex={selected ? 0 : -1}
            className={styles.tab}
            onClick={() => {
              onChange(item.id);
            }}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}

/** Panel matching the active tab; hidden panels stay mounted by the caller. */
export function TabPanel({
  id,
  active,
  idBase,
  children,
}: {
  id: string;
  active: boolean;
  idBase: string;
  children: ReactNode;
}) {
  return (
    <div
      role="tabpanel"
      id={`${idBase}-panel-${id}`}
      aria-labelledby={`${idBase}-tab-${id}`}
      hidden={!active}
      className={styles.tabPanel}
    >
      {children}
    </div>
  );
}
