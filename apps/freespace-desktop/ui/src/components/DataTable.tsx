import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

import {
  clearFocusRing,
  focusWithRing,
  rovingIndex,
} from '../app/focus';
import { cx } from '../lib/cx';
import styles from './DataTable.module.css';

export interface Column<T> {
  id: string;
  header: string;
  width?: string;
  align?: 'start' | 'end';
  sortable?: boolean;
  render: (row: T) => ReactNode;
}

export interface SortState {
  key: string;
  descending: boolean;
}

/** Optional hierarchy semantics: `aria-level`, `aria-expanded` and arrow keys. */
export interface DataTableTree<T> {
  level: (row: T) => number;
  hasChildren: (row: T) => boolean;
  isExpanded: (row: T) => boolean;
  onExpand: (row: T) => void;
  onCollapse: (row: T) => void;
}

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
  /** Hierarchy semantics for tree-like tables; enables arrow expand/collapse. */
  tree?: DataTableTree<T>;
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
}: DataTableProps<T>) {
  const parent = useRef<HTMLDivElement>(null);
  const virtual = rows.length > 50;
  const virtualizer = useVirtualizer({
    count: virtual ? rows.length : 0,
    getScrollElement: () => parent.current,
    estimateSize: () => 36,
    overscan: 12,
  });
  const template = columns
    .map((column) => column.width ?? 'minmax(80px, 1fr)')
    .join(' ');
  const pageSize = Math.max(1, Math.floor(height / 36) - 1);

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

    return (
      <div
        role="row"
        key={key}
        data-row-index={index}
        aria-selected={selected}
        aria-level={tree?.level(row)}
        aria-expanded={expandable ? tree?.isExpanded(row) : undefined}
        tabIndex={index === activeIndex ? 0 : -1}
        className={cx(styles.row, !virtualized && styles.staticRow, selected && styles.selected)}
        style={{
          gridTemplateColumns: template,
          ...style,
        }}
        onBlur={(event) => {
          clearFocusRing(event.currentTarget);
        }}
        onClick={() => {
          setActiveIndex(index);
          onSelect?.(row);
        }}
        onDoubleClick={() => {
          onActivate?.(row);
        }}
        onKeyDown={(event) => {
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
        {columns.map((column, columnIndex) => (
          <div
            key={column.id}
            role="gridcell"
            className={cx(styles.cell, column.align === 'end' && styles.alignEnd)}
          >
            {columnIndex === 0 && selected ? (
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
      onScroll={() => {
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
      <div role="rowgroup">
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
                {column.sortable === true && onSortChange !== undefined ? (
                  <button
                    type="button"
                    className={styles.sortButton}
                    onClick={() => {
                      onSortChange({
                        key: column.id,
                        descending: active && sort !== null ? !sort.descending : true,
                      });
                    }}
                  >
                    {column.header}
                    {active ? (
                      <span aria-hidden="true">{sort.descending ? ' ↓' : ' ↑'}</span>
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
              height: 36,
            });
          })}
        </div>
      ) : (
        <div role="rowgroup" className={styles.staticBody}>
          {rows.map((row, index) =>
            renderRow(row, index, { transform: 'none', height: 36 }, false),
          )}
        </div>
      )}
    </div>
  );
}
