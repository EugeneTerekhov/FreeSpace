import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import {
  clearFocusRing,
  FOCUS_VISIBLE_ATTRIBUTE,
  focusWithRing,
  rovingIndex,
} from '../app/focus';
import { cx } from '../lib/cx';
import styles from './DataTable.module.css';

/** Optional select-all control rendered in a column header. */
export interface HeaderCheckbox {
  checked: boolean;
  indeterminate?: boolean;
  disabled?: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}

export interface Column<T> {
  id: string;
  header: string;
  width?: string;
  align?: 'start' | 'end';
  sortable?: boolean;
  /** Renders a tri-state checkbox in the header, for row selection columns. */
  headerCheckbox?: HeaderCheckbox;
  render: (row: T) => ReactNode;
}

export interface SortState {
  key: string;
  descending: boolean;
}

/** Viewport position of a pointer or a row, used to place a hover card. */
export interface RowAnchor {
  x: number;
  y: number;
}

/** Optional hierarchy semantics: `aria-level`, `aria-expanded` and arrow keys. */
export interface DataTableTree<T> {
  level: (row: T) => number;
  hasChildren: (row: T) => boolean;
  isExpanded: (row: T) => boolean;
  onExpand: (row: T) => void;
  onCollapse: (row: T) => void;
}

/** Shared row height for the virtualizer, the static body and paging. */
const ROW_HEIGHT = 30;

export interface DataTableProps<T> {
  label: string;
  rows: T[];
  columns: Column<T>[];
  rowKey: (row: T) => string | number;
  selectedKey?: string | number | null;
  onSelect?: (row: T) => void;
  onActivate?: (row: T) => void;
  sort?: SortState | null;
  onSortChange?: (sort: SortState) => void;
  empty?: ReactNode;
  loading?: boolean;
  height?: number;
  /** Error slot rendered inside the grid, without replacing the shell. */
  error?: ReactNode;
  /** Partial-result marker rendered inside the grid. */
  partial?: ReactNode;
  /**
   * Decorative layer painted behind the cells, for example the inline share bar
   * of a tree row. It must position itself absolutely; the cells stay above it.
   */
  rowBackground?: (row: T) => ReactNode;
  /** Shows the check mark in the first cell of the selected row. Defaults to true. */
  showSelectionMark?: boolean;
  /** Hierarchy semantics for tree-like tables; enables arrow expand/collapse. */
  tree?: DataTableTree<T>;
  /**
   * Hover card hooks: the anchor is the pointer position for a mouse and the
   * row itself for a keyboard focus.
   */
  onRowHoverStart?: (row: T, anchor: RowAnchor) => void;
  onRowHoverEnd?: (row: T) => void;
  /**
   * Context menu of a row: the right mouse button, Shift+F10 or the
   * context-menu key. The anchor is the pointer or the row itself.
   */
  onRowContextMenu?: (row: T, anchor: RowAnchor) => void;
  /** Restores the vertical offset when the table remounts on a screen return. */
  initialScrollTop?: number;
  /** Reports the vertical offset so the caller can persist the scroll anchor. */
  onScrollTopChange?: (top: number) => void;
}

/** Focus that came from the keyboard, not from a pointer click. */
function isKeyboardFocus(element: HTMLElement): boolean {
  if (element.getAttribute(FOCUS_VISIBLE_ATTRIBUTE) === 'true') return true;
  try {
    return element.matches(':focus-visible');
  } catch {
    return false;
  }
}

/** Tri-state header checkbox; the DOM property carries the partial state. */
function HeaderCheckboxControl({
  checked,
  indeterminate = false,
  disabled = false,
  label,
  onChange,
}: HeaderCheckbox) {
  const reference = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (reference.current !== null) reference.current.indeterminate = indeterminate;
  }, [indeterminate]);

  return (
    <input
      ref={reference}
      type="checkbox"
      className={styles.headerCheckbox}
      aria-label={label}
      checked={checked}
      disabled={disabled}
      onChange={(event) => onChange(event.target.checked)}
    />
  );
}

/** Virtualised grid with roving tabindex, keyboard activation and sortable headers. */
export function DataTable<T>({
  label,
  rows,
  columns,
  rowKey,
  selectedKey = null,
  onSelect,
  onActivate,
  sort = null,
  onSortChange,
  empty,
  loading = false,
  height = 520,
  error,
  partial,
  tree,
  rowBackground,
  showSelectionMark = true,
  onRowHoverStart,
  onRowHoverEnd,
  onRowContextMenu,
  initialScrollTop = 0,
  onScrollTopChange,
}: DataTableProps<T>) {
  const parent = useRef<HTMLDivElement>(null);
  const scrollTopRef = useRef(initialScrollTop);
  const virtual = rows.length > 50;
  const virtualizer = useVirtualizer({
    count: virtual ? rows.length : 0,
    getScrollElement: () => parent.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });
  const template = columns
    .map((column) => column.width ?? 'minmax(80px, 1fr)')
    .join(' ');
  const pageSize = Math.max(1, Math.floor(height / ROW_HEIGHT) - 1);

  // Restore the anchor once the grid remounts. At this point rows are already
  // in the DOM (the caller reuses a cached result), so the scroll range exists.
  useEffect(() => {
    if (parent.current !== null) parent.current.scrollTop = scrollTopRef.current;
  }, []);

  const rowKeyRef = useRef(rowKey);
  rowKeyRef.current = rowKey;
  const [activeIndex, setActiveIndex] = useState(() => {
    if (selectedKey === null) return 0;
    const index = rows.findIndex((row) => rowKey(row) === selectedKey);

    return index < 0 ? 0 : index;
  });
  const lastSelected = useRef(selectedKey);
  const hadFocus = useRef(false);
  const activeIndexRef = useRef(activeIndex);
  activeIndexRef.current = activeIndex;

  useEffect(() => {
    setActiveIndex((current) =>
      rows.length === 0 ? 0 : Math.min(current, rows.length - 1),
    );
  }, [rows.length]);

  useEffect(() => {
    if (selectedKey === lastSelected.current) return;
    lastSelected.current = selectedKey;
    if (selectedKey === null) return;
    const index = rows.findIndex((row) => rowKeyRef.current(row) === selectedKey);
    if (index >= 0) setActiveIndex(index);
  }, [rows, selectedKey]);

  const focusRow = (index: number) => {
    const root = parent.current;
    if (root === null) return;

    const find = () =>
      root.querySelector<HTMLElement>(`[data-row-index="${index}"]`);
    const immediate = find();
    if (immediate !== null) {
      focusWithRing(immediate);
      return;
    }
    if (typeof requestAnimationFrame !== 'function') return;
    requestAnimationFrame(() => {
      const later = find();
      if (later !== null) focusWithRing(later);
    });
  };

  const moveTo = (next: number | null) => {
    if (next === null || rows.length === 0) return;
    if (virtual) virtualizer.scrollToIndex(next);
    setActiveIndex(next);
    focusRow(next);
  };

  // When a refresh replaces the row under the cursor (for example after
  // drilling into a directory), the old DOM node disappears with focus. Move
  // the roving tab stop back onto the new active row so keyboard users do not
  // fall back to the document body.
  useEffect(() => {
    if (!hadFocus.current) return;
    const root = parent.current;
    const element = root?.querySelector<HTMLElement>(
      `[data-row-index="${activeIndexRef.current}"]`,
    );
    if (element !== undefined && element !== null) focusWithRing(element);
  }, [rows]);

  const renderRow = (
    row: T,
    index: number,
    style?: { transform: string; height: number },
    virtualized = true,
  ) => {
    const key = rowKey(row);
    const selected = selectedKey === key;
    const expandable = tree?.hasChildren(row) ?? false;
    const level = tree?.level(row) ?? 1;

    return (
      <div
        role="row"
        key={key}
        data-row-index={index}
        aria-selected={selected}
        aria-level={tree === undefined ? undefined : level}
        aria-expanded={expandable ? tree?.isExpanded(row) : undefined}
        tabIndex={index === activeIndex ? 0 : -1}
        className={cx(styles.row, !virtualized && styles.staticRow, selected && styles.selected)}
        style={{
          gridTemplateColumns: template,
          ...style,
        }}
        onBlur={(event) => {
          clearFocusRing(event.currentTarget);
          onRowHoverEnd?.(row);
        }}
        onFocus={(event) => {
          if (!isKeyboardFocus(event.currentTarget)) return;
          const rect = event.currentTarget.getBoundingClientRect();
          onRowHoverStart?.(row, {
            x: rect.left + Math.min(240, rect.width / 2),
            y: rect.bottom,
          });
        }}
        onPointerEnter={(event) => {
          if (event.pointerType !== 'mouse') return;
          onRowHoverStart?.(row, { x: event.clientX, y: event.clientY });
        }}
        onPointerLeave={() => {
          onRowHoverEnd?.(row);
        }}
        onClick={() => {
          setActiveIndex(index);
          onSelect?.(row);
        }}
        onDoubleClick={() => {
          onActivate?.(row);
        }}
        onContextMenu={(event) => {
          if (onRowContextMenu === undefined) return;
          event.preventDefault();
          setActiveIndex(index);
          onSelect?.(row);
          onRowContextMenu(row, { x: event.clientX, y: event.clientY });
        }}
        onKeyDown={(event) => {
          if (
            onRowContextMenu !== undefined &&
            (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10'))
          ) {
            event.preventDefault();
            const rect = event.currentTarget.getBoundingClientRect();
            setActiveIndex(index);
            onSelect?.(row);
            onRowContextMenu(row, {
              x: rect.left + Math.min(240, rect.width / 2),
              y: rect.bottom,
            });
            return;
          }
          const next = rovingIndex({
            key: event.key,
            index,
            count: rows.length,
            pageSize,
          });
          if (next !== null) {
            event.preventDefault();
            moveTo(next);
            return;
          }
          if (event.key === 'ArrowRight') {
            if (tree === undefined) return;
            event.preventDefault();
            if (tree.hasChildren(row)) tree.onExpand(row);
            return;
          }
          if (event.key === 'ArrowLeft') {
            if (tree === undefined) return;
            event.preventDefault();
            tree.onCollapse(row);
            return;
          }
          if (event.key === 'Enter') {
            event.preventDefault();
            onActivate?.(row);
          } else if (event.key === ' ') {
            event.preventDefault();
            onSelect?.(row);
          }
        }}
      >
        {rowBackground !== undefined ? rowBackground(row) : null}
        {columns.map((column, columnIndex) => (
          <div
            key={column.id}
            role="gridcell"
            className={cx(styles.cell, column.align === 'end' && styles.alignEnd)}
            style={
              tree !== undefined && columnIndex === 0
                ? {
                    paddingLeft: `calc(var(--fs-table-gap) / 2 + ${(level - 1) * 16}px)`,
                  }
                : undefined
            }
          >
            {showSelectionMark && columnIndex === 0 && selected ? (
              <span className={styles.selectedMark} aria-hidden="true">
                ✓
              </span>
            ) : null}
            {column.render(row)}
          </div>
        ))}
      </div>
    );
  };

  const announceRow = (message: ReactNode) => (
    <div role="row">
      <div
        role="gridcell"
        aria-colspan={columns.length}
        className={styles.statusRow}
      >
        {message}
      </div>
    </div>
  );

  return (
    <div
      ref={parent}
      className={styles.table}
      role={tree === undefined ? 'grid' : 'treegrid'}
      aria-label={label}
      aria-rowcount={rows.length}
      aria-colcount={columns.length}
      aria-busy={loading || undefined}
      style={{ height }}
      onFocusCapture={() => {
        hadFocus.current = true;
      }}
      onBlurCapture={(event) => {
        const next = event.relatedTarget;
        if (next !== null && !event.currentTarget.contains(next)) {
          hadFocus.current = false;
        }
      }}
      onScroll={(event) => {
        onScrollTopChange?.(event.currentTarget.scrollTop);
        if (!virtual) return;
        const items = virtualizer.getVirtualItems();
        const first = items[0];
        const last = items[items.length - 1];
        if (first === undefined || last === undefined) return;
        if (activeIndex < first.index || activeIndex > last.index) {
          // Keep the roving tab stop inside the rendered window.
          setActiveIndex(first.index);
        }
      }}
    >
      <div role="rowgroup" className={styles.headerGroup}>
        <div
          role="row"
          className={styles.header}
          style={{ gridTemplateColumns: template }}
        >
          {columns.map((column) => {
            const active = sort?.key === column.id;
            const ariaSort =
              active && sort !== null
                ? sort.descending
                  ? 'descending'
                  : 'ascending'
                : 'none';

            return (
              <div
                key={column.id}
                role="columnheader"
                aria-sort={column.sortable === true ? ariaSort : undefined}
                className={cx(
                  styles.headerCell,
                  column.align === 'end' && styles.alignEnd,
                )}
              >
                {column.headerCheckbox !== undefined ? (
                  <HeaderCheckboxControl {...column.headerCheckbox} />
                ) : column.sortable === true && onSortChange !== undefined ? (
                  <button
                    type="button"
                    className={cx(
                      styles.sortButton,
                      column.align === 'end' && styles.sortButtonEnd,
                    )}
                    onClick={() => {
                      onSortChange({
                        key: column.id,
                        descending: active && sort !== null ? !sort.descending : true,
                      });
                    }}
                  >
                    <span className={styles.sortLabel}>{column.header}</span>
                    {active ? (
                      <span className={styles.sortArrow} aria-hidden="true">
                        {sort.descending ? '↓' : '↑'}
                      </span>
                    ) : null}
                  </button>
                ) : (
                  column.header
                )}
              </div>
            );
          })}
        </div>
      </div>
      {error != null ? announceRow(error) : null}
      {partial != null ? announceRow(partial) : null}
      {rows.length === 0 ? (
        <div role="rowgroup">
          <div role="row">
            <div
              role="gridcell"
              aria-colspan={columns.length}
              className={styles.empty}
            >
              {empty}
            </div>
          </div>
        </div>
      ) : virtual ? (
        <div
          role="rowgroup"
          className={styles.body}
          style={{ height: virtualizer.getTotalSize() }}
        >
          {virtualizer.getVirtualItems().map((item) => {
            const row = rows[item.index];
            if (row === undefined) return null;

            return renderRow(row, item.index, {
              transform: `translateY(${item.start}px)`,
              height: ROW_HEIGHT,
            });
          })}
        </div>
      ) : (
        <div role="rowgroup" className={styles.staticBody}>
          {rows.map((row, index) =>
            renderRow(row, index, { transform: 'none', height: ROW_HEIGHT }, false),
          )}
        </div>
      )}
    </div>
  );
}
