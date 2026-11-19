import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { CommandPalette } from '../../src/app/CommandPalette';
import { DataTable, type Column, type DataTableTree } from '../../src/components/DataTable';

interface Row {
  id: number;
  name: string;
  kind: 'directory' | 'file';
  children: number;
}

const ROWS: Row[] = [
  { id: 1, name: 'Windows', kind: 'directory', children: 3 },
  { id: 2, name: 'pagefile.sys', kind: 'file', children: 0 },
  { id: 3, name: 'Users', kind: 'directory', children: 0 },
];

const COLUMNS: Column<Row>[] = [
  { id: 'name', header: 'Name', sortable: true, render: (row) => row.name },
  { id: 'count', header: 'Items', align: 'end', render: (row) => row.children },
];

describe('DataTable keyboard model', () => {
  it('uses a roving tabindex between rows', async () => {
    const user = userEvent.setup();
    render(
      <DataTable
        label="Tree"
        rows={ROWS}
        columns={COLUMNS}
        rowKey={(row) => row.id}
      />,
    );

    const windows = screen.getByRole('row', { name: /Windows/ });
    const pagefile = screen.getByRole('row', { name: /pagefile/ });
    expect(windows).toHaveAttribute('tabindex', '0');
    expect(pagefile).toHaveAttribute('tabindex', '-1');

    windows.focus();
    await user.keyboard('{ArrowDown}');
    expect(pagefile).toHaveFocus();
    expect(pagefile).toHaveAttribute('tabindex', '0');
    expect(windows).toHaveAttribute('tabindex', '-1');

    await user.keyboard('{End}');
    expect(screen.getByRole('row', { name: /Users/ })).toHaveFocus();

    await user.keyboard('{Home}');
    expect(windows).toHaveFocus();
  });

  it('selects with Space and activates with Enter', async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    const onActivate = vi.fn();
    render(
      <DataTable
        label="Tree"
        rows={ROWS}
        columns={COLUMNS}
        rowKey={(row) => row.id}
        onSelect={onSelect}
        onActivate={onActivate}
      />,
    );

    const windows = screen.getByRole('row', { name: /Windows/ });
    windows.focus();
    await user.keyboard(' ');
    expect(onSelect).toHaveBeenCalledWith(ROWS[0]);

    await user.keyboard('{Enter}');
    expect(onActivate).toHaveBeenCalledWith(ROWS[0]);
  });

  it('expands and collapses tree rows with the arrow keys', async () => {
    const user = userEvent.setup();
    const onExpand = vi.fn();
    const onCollapse = vi.fn();
    const tree: DataTableTree<Row> = {
      level: (row) => (row.kind === 'directory' ? 1 : 2),
      hasChildren: (row) => row.kind === 'directory' && row.children > 0,
      isExpanded: () => false,
      onExpand,
      onCollapse,
    };

    render(
      <DataTable
        label="Tree"
        rows={ROWS}
        columns={COLUMNS}
        rowKey={(row) => row.id}
        tree={tree}
      />,
    );

    const windows = screen.getByRole('row', { name: /Windows/ });
    expect(windows).toHaveAttribute('aria-level', '1');
    expect(windows).toHaveAttribute('aria-expanded', 'false');

    const file = screen.getByRole('row', { name: /pagefile/ });
    expect(file).toHaveAttribute('aria-level', '2');
    expect(file).not.toHaveAttribute('aria-expanded');

    windows.focus();
    await user.keyboard('{ArrowRight}');
    expect(onExpand).toHaveBeenCalledWith(ROWS[0]);

    await user.keyboard('{ArrowLeft}');
    expect(onCollapse).toHaveBeenCalledWith(ROWS[0]);
  });

  it('renders error, partial and empty slots inside the grid', () => {
    render(
      <DataTable
        label="Tree"
        rows={[]}
        columns={COLUMNS}
        rowKey={(row) => row.id}
        error="Scan failed"
        partial="Partial results"
        empty="Nothing here"
      />,
    );

    expect(screen.getByText('Scan failed')).toBeInTheDocument();
    expect(screen.getByText('Partial results')).toBeInTheDocument();
    expect(screen.getByText('Nothing here')).toBeInTheDocument();
    expect(screen.getByRole('grid', { name: 'Tree' })).toBeInTheDocument();
  });
});

describe('CommandPalette keyboard model', () => {
  it('focuses the search input and tracks aria-activedescendant', async () => {
    const user = userEvent.setup();
    render(
      <CommandPalette
        open
        onClose={() => undefined}
        commands={[
          { id: 'tree', label: 'Tree', run: () => undefined },
          { id: 'map', label: 'Map', run: () => undefined },
        ]}
      />,
    );

    const input = screen.getByRole('combobox');
    expect(input).toHaveFocus();
    expect(input).toHaveAttribute('aria-activedescendant', 'command-tree');

    await user.keyboard('{ArrowDown}');
    expect(input).toHaveAttribute('aria-activedescendant', 'command-map');

    const options = screen.getAllByRole('option');
    expect(options[1]).toHaveAttribute('id', 'command-map');
    expect(options[1]).toHaveAttribute('aria-selected', 'true');
  });
});
