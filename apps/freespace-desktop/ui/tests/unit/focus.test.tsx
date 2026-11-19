import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import {
  FOCUS_VISIBLE_ATTRIBUTE,
  clearFocusRing,
  focusElement,
  focusWithRing,
  focusableElements,
  moveFocusWithin,
  resolveInitialFocus,
  rovingIndex,
  useFocusTrap,
  type FocusTrapOptions,
} from '../../src/app/focus';
import { Dialog } from '../../src/components/Dialog';

function Trap({
  open,
  options,
}: {
  open: boolean;
  options?: FocusTrapOptions;
}) {
  const ref = useFocusTrap<HTMLDivElement>(open, options);

  return (
    <div>
      {open ? (
        <div ref={ref}>
          <button type="button">first</button>
          <button type="button">last</button>
        </div>
      ) : null}
    </div>
  );
}

describe('focus helpers', () => {
  it('lists only focusable elements in DOM order', () => {
    const { container } = render(
      <div data-testid="root">
        <button type="button" aria-label="one">
          One
        </button>
        <button type="button" disabled aria-label="two">
          Two
        </button>
        <span tabIndex={-1} aria-label="skipped">
          Three
        </span>
        <button type="button" hidden aria-label="hidden">
          Four
        </button>
        <input aria-label="search" />
      </div>,
    );
    const root = container.querySelector<HTMLElement>('[data-testid="root"]');

    expect(
      focusableElements(root).map((element) => element.getAttribute('aria-label')),
    ).toEqual(['one', 'search']);
  });

  it('prefers a data-autofocus marker for the initial focus', () => {
    const { container } = render(
      <div data-testid="root">
        <button type="button">close</button>
        <input aria-label="search" data-autofocus="true" />
      </div>,
    );
    const root = container.querySelector<HTMLElement>('[data-testid="root"]');
    if (root === null) throw new Error('missing root');

    const initial = resolveInitialFocus(root);
    expect(initial?.getAttribute('aria-label')).toBe('search');
    expect(focusElement(initial)).toBe(true);
    expect(screen.getByLabelText('search')).toHaveFocus();
  });

  it('wraps Tab and Shift+Tab inside a container', () => {
    const { container } = render(
      <div data-testid="root">
        <button type="button">first</button>
        <button type="button">last</button>
      </div>,
    );
    const root = container.querySelector<HTMLElement>('[data-testid="root"]');
    if (root === null) throw new Error('missing root');
    const [first, last] = focusableElements(root);
    if (first === undefined || last === undefined) throw new Error('missing buttons');

    last.focus();
    expect(moveFocusWithin(root, false)).toBe(true);
    expect(first).toHaveFocus();

    expect(moveFocusWithin(root, true)).toBe(true);
    expect(last).toHaveFocus();
  });

  it('computes roving indexes for grid navigation', () => {
    expect(rovingIndex({ key: 'ArrowDown', index: 0, count: 3 })).toBe(1);
    expect(rovingIndex({ key: 'ArrowUp', index: 0, count: 3 })).toBe(0);
    expect(rovingIndex({ key: 'End', index: 0, count: 3 })).toBe(2);
    expect(rovingIndex({ key: 'Home', index: 2, count: 3 })).toBe(0);
    expect(rovingIndex({ key: 'PageDown', index: 0, count: 30, pageSize: 10 })).toBe(10);
    expect(rovingIndex({ key: 'PageUp', index: 4, count: 30, pageSize: 10 })).toBe(0);
    expect(rovingIndex({ key: 'Tab', index: 0, count: 3 })).toBeNull();
    expect(rovingIndex({ key: 'ArrowDown', index: 0, count: 0 })).toBeNull();
  });

  it('marks programmatic focus so the ring stays visible', () => {
    const { container } = render(<button type="button">row</button>);
    const button = container.querySelector('button');
    if (button === null) throw new Error('missing button');

    expect(focusWithRing(button)).toBe(true);
    expect(button).toHaveAttribute(FOCUS_VISIBLE_ATTRIBUTE, 'true');

    clearFocusRing(button);
    expect(button).not.toHaveAttribute(FOCUS_VISIBLE_ATTRIBUTE);
  });

  it('moves initial focus, traps Tab, handles Escape and returns focus', async () => {
    const user = userEvent.setup();
    const onEscape = vi.fn();
    const { rerender } = render(
      <>
        <button type="button">opener</button>
        <Trap open={false} options={{ onEscape }} />
      </>,
    );
    const opener = screen.getByRole('button', { name: 'opener' });
    opener.focus();

    rerender(
      <>
        <button type="button">opener</button>
        <Trap open options={{ onEscape }} />
      </>,
    );

    const first = screen.getByRole('button', { name: 'first' });
    const last = screen.getByRole('button', { name: 'last' });
    expect(first).toHaveFocus();

    await user.tab();
    expect(last).toHaveFocus();
    await user.tab();
    expect(first).toHaveFocus();
    await user.tab({ shift: true });
    expect(last).toHaveFocus();

    await user.keyboard('{Escape}');
    expect(onEscape).toHaveBeenCalledTimes(1);

    rerender(
      <>
        <button type="button">opener</button>
        <Trap open={false} options={{ onEscape }} />
      </>,
    );
    expect(opener).toHaveFocus();
  });

  it('ignores Escape while the trap is disabled', async () => {
    const user = userEvent.setup();
    const onEscape = vi.fn();
    render(<Trap open options={{ onEscape, escapeDisabled: true }} />);

    expect(screen.getByRole('button', { name: 'first' })).toHaveFocus();
    await user.keyboard('{Escape}');
    expect(onEscape).not.toHaveBeenCalled();
  });

  it('focuses the dialog input and restores the trigger on close', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    const { rerender } = render(
      <>
        <button type="button">open dialog</button>
        <Dialog open={false} title="Test" onClose={onClose}>
          <input aria-label="search" data-autofocus="true" />
        </Dialog>
      </>,
    );
    const trigger = screen.getByRole('button', { name: 'open dialog' });
    trigger.focus();

    rerender(
      <>
        <button type="button">open dialog</button>
        <Dialog open title="Test" onClose={onClose}>
          <input aria-label="search" data-autofocus="true" />
        </Dialog>
      </>,
    );

    expect(screen.getByRole('dialog', { name: 'Test' })).toBeInTheDocument();
    expect(screen.getByLabelText('search')).toHaveFocus();

    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(1);

    rerender(
      <>
        <button type="button">open dialog</button>
        <Dialog open={false} title="Test" onClose={onClose}>
          <input aria-label="search" data-autofocus="true" />
        </Dialog>
      </>,
    );
    expect(trigger).toHaveFocus();
  });
});
