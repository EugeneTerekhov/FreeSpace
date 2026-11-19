import { useEffect, useRef, type RefObject } from 'react';

/**
 * Elements that participate in the document tab order.
 *
 * Groups are written out explicitly instead of relying on `:is()` because the
 * selector is also queried in the jsdom tests.
 */
export const FOCUSABLE_SELECTOR = [
  'a[href]',
  'area[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'audio[controls]',
  'video[controls]',
  'details > summary',
  'iframe',
  '[contenteditable]:not([contenteditable="false"])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

/** Attribute set when focus was moved programmatically from the keyboard. */
export const FOCUS_VISIBLE_ATTRIBUTE = 'data-focus-visible';

/** Global class for content that is hidden visually but readable by a screen reader. */
export const VISUALLY_HIDDEN_CLASS = 'fs-visually-hidden';

/** Id of the main landmark, referenced by the shell skip link. */
export const MAIN_CONTENT_ID = 'main-content';

function isAvailable(element: HTMLElement): boolean {
  if (element.hasAttribute('hidden')) return false;
  if (element.getAttribute('aria-hidden') === 'true') return false;
  if (element.closest('[hidden]') !== null) return false;
  if (element.closest('[inert]') !== null) return false;

  return true;
}

/** Focusable elements of a container in DOM (and therefore tab) order. */
export function focusableElements(container: HTMLElement | null): HTMLElement[] {
  if (container === null) return [];

  return Array.from(
    container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
  ).filter(isAvailable);
}

/** Focuses an element, tolerating null and non-HTML targets. */
export function focusElement(
  target: Element | null | undefined,
  options: { preventScroll?: boolean } = {},
): boolean {
  if (!(target instanceof HTMLElement)) return false;

  target.focus({ preventScroll: options.preventScroll === true });

  return document.activeElement === target;
}

/**
 * Picks the element that should receive focus when a dialog opens:
 * an explicit `[data-autofocus]` marker, then the first focusable control.
 */
export function resolveInitialFocus(
  container: HTMLElement,
  preferredSelector?: string,
): HTMLElement | null {
  if (preferredSelector !== undefined) {
    const preferred = container.querySelector<HTMLElement>(preferredSelector);
    if (preferred !== null && isAvailable(preferred)) return preferred;
  }

  const marked = container.querySelector<HTMLElement>('[data-autofocus]');
  if (marked !== null && isAvailable(marked)) return marked;

  return focusableElements(container)[0] ?? null;
}

/**
 * Moves focus inside a closed container when Tab or Shift+Tab would leave it.
 * Returns `true` when the caller must call `preventDefault()`.
 */
export function moveFocusWithin(
  container: HTMLElement | null,
  backwards: boolean,
): boolean {
  if (container === null) return false;

  const list = focusableElements(container);
  const first = list[0];
  const last = list[list.length - 1];
  const active = document.activeElement;
  if (first === undefined || last === undefined) {
    // Nothing to move to: keep focus inside the container instead of leaving it.
    return active !== null && container.contains(active);
  }

  if (active === null || !container.contains(active)) {
    (backwards ? last : first).focus();
    return true;
  }
  if (backwards && active === first) {
    last.focus();
    return true;
  }
  if (!backwards && active === last) {
    first.focus();
    return true;
  }

  return false;
}

export interface RovingMove {
  key: string;
  index: number;
  count: number;
  /** Rows moved by PageUp/PageDown; defaults to 10. */
  pageSize?: number;
}

/**
 * Next row/cell index for a roving-tabindex grid.
 * Returns `null` for keys the grid does not handle.
 */
export function rovingIndex({
  key,
  index,
  count,
  pageSize = 10,
}: RovingMove): number | null {
  if (count <= 0) return null;

  const last = count - 1;
  switch (key) {
    case 'ArrowDown':
      return Math.min(last, index + 1);
    case 'ArrowUp':
      return Math.max(0, index - 1);
    case 'Home':
      return 0;
    case 'End':
      return last;
    case 'PageDown':
      return Math.min(last, index + pageSize);
    case 'PageUp':
      return Math.max(0, index - pageSize);
    default:
      return null;
  }
}

/** Focuses an element and keeps the visible focus ring on it. */
export function focusWithRing(target: HTMLElement | null): boolean {
  if (target === null) return false;

  target.setAttribute(FOCUS_VISIBLE_ATTRIBUTE, 'true');
  const focused = focusElement(target);
  if (!focused) target.removeAttribute(FOCUS_VISIBLE_ATTRIBUTE);

  return focused;
}

/** Removes the extra focus ring marker, e.g. on blur. */
export function clearFocusRing(target: HTMLElement | null): void {
  target?.removeAttribute(FOCUS_VISIBLE_ATTRIBUTE);
}

export interface FocusTrapOptions {
  /** Selector of the control that should receive focus first. */
  initialFocusSelector?: string;
  /** Return focus to the previously focused element when the trap closes. */
  restoreFocus?: boolean;
  /** Escape handler; when omitted, Escape is ignored by the trap. */
  onEscape?: () => void;
  /** When true the dialog cannot be dismissed with Escape, e.g. while busy. */
  escapeDisabled?: boolean;
}

/**
 * Modal focus trap used by `Dialog`: focuses the first control, keeps Tab
 * inside the container, restores focus on close and routes Escape.
 */
export function useFocusTrap<T extends HTMLElement>(
  open: boolean,
  options: FocusTrapOptions = {},
): RefObject<T | null> {
  const container = useRef<T>(null);
  const latest = useRef(options);
  latest.current = options;

  useEffect(() => {
    if (!open) return undefined;

    const previous =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const element = container.current;
    if (element !== null) {
      const initial = resolveInitialFocus(
        element,
        latest.current.initialFocusSelector,
      );
      focusElement(initial ?? element);
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        const handler = latest.current.onEscape;
        if (handler !== undefined && latest.current.escapeDisabled !== true) {
          event.preventDefault();
          handler();
        }
        return;
      }
      if (event.key !== 'Tab') return;
      if (moveFocusWithin(container.current, event.shiftKey)) {
        event.preventDefault();
      }
    };

    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('keydown', onKeyDown);
      if (latest.current.restoreFocus !== false) focusElement(previous);
    };
  }, [open]);

  return container;
}
