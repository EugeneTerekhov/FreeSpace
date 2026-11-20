import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { DataTable, type Column } from '../../src/components/DataTable';
import type { NodeRow } from '../../src/api/dto';

const ROWS: NodeRow[] = [
  {
    id: 1,
    parent_id: 0,
    name: 'Windows',
    path: 'C:\\Windows',
    kind: 'directory',
    logical_size: 100,
    on_disk_size: 100,
    physical_size: 100,
    ads_size: 0,
    child_count: 3,
    file_count: 5,
    modified_at: null,
    accessed_at: null,
    attributes: {
      read_only: false,
      hidden: false,
      system: false,
      compressed: false,
      sparse: false,
      reparse: false,
    },
    accessible: true,
    link_target: null,
  },
  {
    id: 2,
    parent_id: 0,
    name: 'pagefile.sys',
    path: 'C:\\pagefile.sys',
    kind: 'file',
    logical_size: 50,
    on_disk_size: 50,
    physical_size: 50,
    ads_size: 0,
    child_count: 0,
    file_count: 1,
    modified_at: null,
    accessed_at: null,
    attributes: {
      read_only: false,
      hidden: false,
      system: false,
      compressed: false,
      sparse: false,
      reparse: false,
    },
    accessible: true,
    link_target: null,
  },
];

const COLUMNS: Column<NodeRow>[] = [
  { id: 'name', header: 'Name', sortable: true, render: (row) => row.name },
  { id: 'size', header: 'Size', align: 'end', render: (row) => row.on_disk_size },
];

describe('DataTable', () => {
  it('renders accessible grid rows and selects a row', async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <DataTable
        label="Tree"
        rows={ROWS}
        columns={COLUMNS}
        rowKey={(row) => row.id}
        onSelect={onSelect}
      />,
    );

    expect(screen.getByRole('grid', { name: 'Tree' })).toHaveAttribute(
      'aria-rowcount',
      '2',
    );
    expect(screen.getByRole('row', { name: /Windows/ })).toBeInTheDocument();

    await user.click(screen.getByRole('row', { name: /Windows/ }));
    expect(onSelect).toHaveBeenCalledWith(ROWS[0]);
  });

  it('sorts from a column header and activates with Enter', async () => {
    const user = userEvent.setup();
    const onSortChange = vi.fn();
    const onActivate = vi.fn();
    render(
      <DataTable
        label="Tree"
        rows={ROWS}
        columns={COLUMNS}
        rowKey={(row) => row.id}
        sort={{ key: 'size', descending: true }}
        onSortChange={onSortChange}
        onActivate={onActivate}
      />,
    );

    await user.click(screen.getByRole('button', { name: /Name/ }));
    expect(onSortChange).toHaveBeenCalledWith({ key: 'name', descending: true });

    const row = screen.getByRole('row', { name: /pagefile/ });
    row.focus();
    await user.keyboard('{Enter}');
    expect(onActivate).toHaveBeenCalledWith(ROWS[1]);
  });

  it('pins the header group so column names stay visible while rows scroll', () => {
    render(
      <DataTable
        label="Tree"
        rows={ROWS}
        columns={COLUMNS}
        rowKey={(row) => row.id}
      />,
    );

    const header = screen.getByRole('columnheader', { name: /Name/ });
    const group = header.closest('[role="rowgroup"]');
    expect(group?.className).toContain('headerGroup');
  });

  it('renders the empty slot', () => {
    render(
      <DataTable
        label="Tree"
        rows={[]}
        columns={COLUMNS}
        rowKey={(row) => row.id}
        empty="Nothing here"
      />,
    );

    expect(screen.getByText('Nothing here')).toBeInTheDocument();
  });
});
