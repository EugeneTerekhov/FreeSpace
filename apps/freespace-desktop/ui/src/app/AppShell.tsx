import { useEffect, useState, type ReactNode } from 'react';

import { MAIN_CONTENT_ID } from './focus';
import { useLiveAnnouncer } from './live';
import { cx } from '../lib/cx';
import { LiveRegions } from '../components/Feedback';
import { t } from '../i18n';
import styles from './app.module.css';

export interface AppShellProps {
  sidebar: ReactNode;
  header: ReactNode;
  status: ReactNode;
  children: ReactNode;
  /** Skip link text; localized by the caller. */
  skipLabel?: string;
}

/**
 * Landmark skeleton: a skip link first, then navigation, banner, main and
 * contentinfo. Tab order follows the DOM, so it matches the visual order.
 */
export function AppShell({
  sidebar,
  header,
  status,
  children,
  skipLabel = t('a11y.skip_to_main'),
}: AppShellProps) {
  return (
    <div className={styles.shell}>
      <a
        className={styles.skipLink}
        href={`#${MAIN_CONTENT_ID}`}
        onClick={(event) => {
          const main = document.getElementById(MAIN_CONTENT_ID);
          if (main === null) return;
          event.preventDefault();
          main.focus();
        }}
      >
        {skipLabel}
      </a>
      {sidebar}
      <div className={styles.main}>
        {header}
        <main className={styles.content} id={MAIN_CONTENT_ID} tabIndex={-1}>
          {children}
        </main>
      </div>
      {status}
    </div>
  );
}

export interface SidebarItem {
  id: string;
  label: string;
  icon?: ReactNode;
  badge?: ReactNode;
}

/** Navigation group: a static heading with its screens below it. */
export interface SidebarGroup {
  id: string;
  label: string;
  items: SidebarItem[];
}

export interface SidebarProps {
  groups: SidebarGroup[];
  active: string;
  onSelect: (id: string) => void;
  collapsed?: boolean;
  /** Persistent footer, usually the active drive summary. */
  footer?: ReactNode;
}

/**
 * Primary navigation: one labelled group per section, the active screen marked
 * with both `aria-current` and a visible accent. The brand block carries the
 * application image, which is decorative for screen readers.
 */
export function Sidebar({
  groups,
  active,
  onSelect,
  collapsed = false,
  footer,
}: SidebarProps) {
  return (
    <aside className={cx(styles.sidebar, collapsed && styles.sidebarCollapsed)}>
      <div className={styles.brand}>
        <span className={styles.brandMark} aria-hidden="true">
          <img className={styles.brandIcon} src="/icon.png" alt="" />
        </span>
        <span className={styles.brandText}>{t('app.name')}</span>
      </div>
      <nav aria-label={t('a11y.screens')}>
        {groups.map((group) => (
          <div className={styles.navGroup} key={group.id} data-nav-group={group.id}>
            <span className={styles.navGroupLabel}>{group.label}</span>
            <ul className={styles.navList}>
              {group.items.map((item) => {
                const current = item.id === active;

                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      data-screen={item.id}
                      className={cx(styles.navItem, current && styles.navItemActive)}
                      aria-current={current ? 'page' : undefined}
                      aria-label={item.label}
                      title={collapsed ? item.label : undefined}
                      onClick={() => {
                        onSelect(item.id);
                      }}
                    >
                      <span className={styles.navIcon} aria-hidden="true">
                        {item.icon ?? '•'}
                      </span>
                      <span className={styles.navLabel}>{item.label}</span>
                      {item.badge !== undefined ? (
                        <span className={styles.navBadge}>{item.badge}</span>
                      ) : null}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>
      {footer !== undefined ? <div className={styles.sidebarFooter}>{footer}</div> : null}
    </aside>
  );
}

export interface SidebarDriveProps {
  mount: string;
  /** Localized free-space amount, e.g. `288 ГБ свободно`. */
  free: string;
  /** Localized total capacity, e.g. `1 ТБ`. */
  total: string;
}

/** Compact drive line at the bottom of the sidebar. */
export function SidebarDrive({ mount, free, total }: SidebarDriveProps) {
  return (
    <div
      className={styles.sidebarDrive}
      title={`${mount} · ${t('sidebar.free', { size: free })} · ${total}`}
    >
      <span className={styles.driveDot} aria-hidden="true" />
      <span className={styles.driveText}>
        {mount} · {free}
      </span>
      <span className={styles.driveTotal}>{total}</span>
    </div>
  );
}

export interface HeaderProps {
  title: string;
  subtitle?: string | undefined;
  back?: ReactNode;
  actions?: ReactNode;
}

/** Screen header with a single `h1`. */
export function Header({ title, subtitle, back, actions }: HeaderProps) {
  return (
    <header className={styles.header}>
      {back}
      <div className={styles.headerText}>
        <h1 className={styles.title}>{title}</h1>
        {subtitle !== undefined ? <span className={styles.subtitle}>{subtitle}</span> : null}
      </div>
      {actions !== undefined ? <div className={styles.headerActions}>{actions}</div> : null}
    </header>
  );
}

export interface StatusBarProps {
  message?: ReactNode;
  task?: ReactNode;
  right?: ReactNode;
  offline?: boolean;
}

function textOf(value: ReactNode): string | null {
  if (typeof value === 'string') return value;
  if (typeof value === 'number') return String(value);

  return null;
}

/**
 * Persistent status line. The visible footer is a `contentinfo` landmark; its
 * `role="status"` child is muted (`aria-live="off"`) so a task that updates
 * every second does not flood the narrator. A separate throttled live region
 * announces the same text at most once every two seconds.
 */
export function StatusBar({ message, task, right, offline = false }: StatusBarProps) {
  const { polite, assertive, announce } = useLiveAnnouncer();
  const text = [offline ? t('status.offline') : null, textOf(message), textOf(task)]
    .filter((part): part is string => part !== null && part !== '')
    .join(' · ');

  useEffect(() => {
    if (text === '') return;
    announce(text, { priority: 'progress' });
  }, [announce, text]);

  return (
    <footer className={styles.statusBar} data-status-bar tabIndex={-1}>
      <div className={styles.statusContent} role="status" aria-live="off">
        {offline ? <span>{t('status.offline')}</span> : null}
        {message !== undefined ? <span>{message}</span> : null}
        {task !== undefined ? <span>{task}</span> : null}
      </div>
      <LiveRegions polite={polite} assertive={assertive} />
      {right !== undefined ? <span className={styles.statusSpacer}>{right}</span> : null}
    </footer>
  );
}

export interface Crumb {
  label: string;
  onClick?: (() => void) | undefined;
}

/** Hierarchy trail; the final item is the current page. */
export function Breadcrumbs({ items }: { items: Crumb[] }) {
  return (
    <nav aria-label={t('a11y.breadcrumbs')}>
      <ol className={styles.breadcrumbs}>
        {items.map((item, index) => {
          const last = index === items.length - 1;

          return (
            <li key={`${item.label}-${index}`}>
              {last ? (
                <span aria-current="page">{item.label}</span>
              ) : (
                <>
                  {item.onClick !== undefined ? (
                    <button
                      type="button"
                      className={styles.crumbButton}
                      onClick={item.onClick}
                    >
                      {item.label}
                    </button>
                  ) : (
                    <span>{item.label}</span>
                  )}
                  <span className={styles.crumbSeparator} aria-hidden="true">
                    /
                  </span>
                </>
              )}
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

export interface SplitPaneProps {
  left: ReactNode;
  right: ReactNode;
  initialLeftPercent?: number;
  label?: string;
}

/** Two panes with a keyboard-resizable separator. */
export function SplitPane({
  left,
  right,
  initialLeftPercent = 60,
  label = t('a11y.resize_panes'),
}: SplitPaneProps) {
  const [percent, setPercent] = useState(
    Math.max(20, Math.min(80, initialLeftPercent)),
  );

  return (
    <div className={styles.split}>
      <div className={styles.splitPane} style={{ width: `${percent}%` }}>
        {left}
      </div>
      <button
        type="button"
        role="separator"
        aria-orientation="vertical"
        aria-label={label}
        aria-valuemin={20}
        aria-valuemax={80}
        aria-valuenow={Math.round(percent)}
        className={styles.splitHandle}
        onKeyDown={(event) => {
          if (event.key === 'ArrowLeft') {
            event.preventDefault();
            setPercent((value) => Math.max(20, value - 5));
          } else if (event.key === 'ArrowRight') {
            event.preventDefault();
            setPercent((value) => Math.min(80, value + 5));
          }
        }}
      />
      <div className={styles.splitPane} style={{ flex: 1 }}>
        {right}
      </div>
    </div>
  );
}
