import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';

import { focusElement } from '../app/focus';
import { cx } from '../lib/cx';
import styles from './ContextMenu.module.css';

/** One command of a context menu. */
export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: ReactNode;
  /** Destructive commands use the danger tone and are never first. */
  danger?: boolean;
  disabled?: boolean;
  onSelect: () => void;
}

export interface ContextMenuProps {
  open: boolean;
  /** Viewport position of the pointer or the row that opened the menu. */
  x: number;
  y: number;
  /** Accessible name; usually the object the menu belongs to. */
  label: string;
  items: ContextMenuItem[];
  onClose: () => void;
}

const VIEWPORT_MARGIN = 8;

/**
 * Context menu anchored to a pointer or a row (`role="menu"`, contract §3).
 *
 * The menu is rendered in a portal so a virtualised row cannot clip it. It keeps
 * the focus inside while open, moves with the arrow keys, closes on Escape, Tab,
 * an outside press, resize or scroll, and returns focus to the element that
 * opened it unless another dialog already took focus.
 */
export function ContextMenu({
  open,
  x,
  y,
  label,
  items,
  onClose,
}: ContextMenuProps) {
  const menu = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ left: x, top: y });
  const close = useRef(onClose);
  close.current = onClose;

  useLayoutEffect(() => {
    if (!open) return;
    const element = menu.current;
    const width = element?.offsetWidth ?? 0;
    const height = element?.offsetHeight ?? 0;
    const viewportWidth = typeof window === 'undefined' ? 1280 : window.innerWidth;
    const viewportHeight = typeof window === 'undefined' ? 800 : window.innerHeight;
    setPosition({
      left: Math.max(
        VIEWPORT_MARGIN,
        Math.min(x, viewportWidth - width - VIEWPORT_MARGIN),
      ),
      top: Math.max(
        VIEWPORT_MARGIN,
        Math.min(y, viewportHeight - height - VIEWPORT_MARGIN),
      ),
    });
  }, [items.length, open, x, y]);

  useEffect(() => {
    if (!open) return undefined;
    const element = menu.current;
    const previous =
      document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const first = element?.querySelector<HTMLElement>(
      '[role="menuitem"]:not(:disabled)',
    );
    focusElement(first ?? null);

    const onPointerDown = (event: PointerEvent) => {
      if (
        element !== null &&
        event.target instanceof Node &&
        element.contains(event.target)
      ) {
        return;
      }
      close.current();
    };
    const dismiss = () => {
      close.current();
    };
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        close.current();
      }
    };

    document.addEventListener('pointerdown', onPointerDown, true);
    document.addEventListener('keydown', onKeyDown);
    document.addEventListener('scroll', dismiss, true);
    window.addEventListener('resize', dismiss);
    window.addEventListener('blur', dismiss);

    return () => {
      document.removeEventListener('pointerdown', onPointerDown, true);
      document.removeEventListener('keydown', onKeyDown);
      document.removeEventListener('scroll', dismiss, true);
      window.removeEventListener('resize', dismiss);
      window.removeEventListener('blur', dismiss);
      // A command that opens a dialog already moved focus; only restore it when
      // the menu itself (or nothing) owned the focus.
      const active = document.activeElement;
      if (
        active === null ||
        active === document.body ||
        element?.contains(active) === true
      ) {
        focusElement(previous);
      }
    };
  }, [open]);

  if (!open) return null;

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Tab') {
      event.preventDefault();
      onClose();
      return;
    }
    if (
      event.key !== 'ArrowDown' &&
      event.key !== 'ArrowUp' &&
      event.key !== 'Home' &&
      event.key !== 'End'
    ) {
      return;
    }

    const buttons = Array.from(
      menu.current?.querySelectorAll<HTMLButtonElement>(
        '[role="menuitem"]:not(:disabled)',
      ) ?? [],
    );
    if (buttons.length === 0) return;
    event.preventDefault();
    const current = buttons.findIndex((button) => button === document.activeElement);
    const last = buttons.length - 1;
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? last
          : event.key === 'ArrowDown'
            ? current < 0 || current === last
              ? 0
              : current + 1
            : current <= 0
              ? last
              : current - 1;
    buttons[next]?.focus();
  };

  return createPortal(
    <div
      ref={menu}
      role="menu"
      aria-label={label}
      className={styles.menu}
      style={{ left: position.left, top: position.top }}
      onKeyDown={onKeyDown}
      onContextMenu={(event) => {
        event.preventDefault();
      }}
    >
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          role="menuitem"
          tabIndex={-1}
          disabled={item.disabled}
          className={cx(
            styles.item,
            item.danger === true && styles.danger,
            item.disabled === true && styles.disabled,
          )}
          onClick={() => {
            if (item.disabled === true) return;
            onClose();
            item.onSelect();
          }}
        >
          {item.icon === undefined ? null : (
            <span className={styles.icon} aria-hidden="true">
              {item.icon}
            </span>
          )}
          <span className={styles.label}>{item.label}</span>
        </button>
      ))}
    </div>,
    document.body,
  );
}
