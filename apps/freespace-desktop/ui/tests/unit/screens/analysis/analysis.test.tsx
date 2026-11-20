import { useState } from 'react';

import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../../../../src/api/runtime';
import { Analysis } from '../../../../src/screens/Analysis';
import { Cleanup } from '../../../../src/screens/Cleanup';
import { Duplicates } from '../../../../src/screens/Duplicates';
import { Snapshots } from '../../../../src/screens/Snapshots';
import { FilterBar } from '../../../../src/components/FilterBar';
import { emptyFilter } from '../../../../src/lib/filters';
import { useCleanupStore } from '../../../../src/stores/cleanupStore';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { renderWithI18n } from '../../../helpers/render';

const volume = {
  volume_id: '11111111-1111-1111-1111-111111111111',
  mount_point: 'C:\\',
  label: 'System',
  file_system: 'ntfs',
  total_bytes: 1024,
  free_bytes: 512,
  used_bytes: 512,
  is_removable: false,
  status: 'available',
  scan_method: 'mft',
  is_elevated: true,
};

/** Mounted tab panel; every panel stays in the DOM (contract §4.2). */
function panel(id: string): HTMLElement {
  const element = document.getElementById(`analysis-panel-${id}`);
  if (element === null) throw new Error(`missing analysis panel: ${id}`);

  return element;
}

beforeEach(() => {
  useSnapshotStore.getState().setSnapshot(2, volume);
});

afterEach(() => {
  useSnapshotStore.getState().reset();
  useCleanupStore.getState().reset();
  vi.restoreAllMocks();
});

describe('analysis screen', () => {
  it('renders the three tabs and switches the visible panel', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Analysis />);

    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([
      'Крупнейшие файлы',
      'Типы файлов',
      'Владельцы',
    ]);
    expect(screen.getByRole('tab', { name: 'Крупнейшие файлы' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    expect(panel('top')).not.toHaveAttribute('hidden');
    expect(panel('types')).toHaveAttribute('hidden');
    expect(panel('owners')).toHaveAttribute('hidden');

    expect(await within(panel('top')).findByText('pagefile.sys')).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: 'Владельцы' }));

    expect(screen.getByRole('tab', { name: 'Владельцы' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    expect(panel('owners')).not.toHaveAttribute('hidden');
    expect(panel('top')).toHaveAttribute('hidden');
    expect(
      within(panel('owners')).getByText(
        'Нажмите «Построить отчёт», чтобы прочитать владельцев файлов.',
      ),
    ).toBeInTheDocument();
  });

  it('filters each tab and keeps the filter after a tab switch', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Analysis />);

    const top = panel('top');
    expect(await within(top).findByText('pagefile.sys')).toBeInTheDocument();

    await user.type(within(top).getByRole('searchbox', { name: 'Текст' }), 'crash');
    await waitFor(() => {
      expect(within(top).getByText('crash.dmp')).toBeInTheDocument();
      expect(within(top).queryByText('pagefile.sys')).not.toBeInTheDocument();
    });

    await user.click(screen.getByRole('tab', { name: 'Типы файлов' }));
    const types = panel('types');
    await user.type(within(types).getByRole('searchbox', { name: 'Текст' }), 'zip');

    await waitFor(() => {
      expect(within(types).queryByRole('button', { name: '.sys' })).not.toBeInTheDocument();
    });
    expect(within(types).getByRole('button', { name: '.zip' })).toBeInTheDocument();

    await user.click(screen.getByRole('tab', { name: 'Крупнейшие файлы' }));

    expect(within(top).getByText('crash.dmp')).toBeInTheDocument();
    expect(within(top).queryByText('pagefile.sys')).not.toBeInTheDocument();
  });

  it('selects another extension when its whole row is clicked', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Analysis />);

    await user.click(screen.getByRole('tab', { name: 'Типы файлов' }));
    const types = panel('types');
    const table = within(types).getByRole('table', { name: 'Типы файлов' });

    await waitFor(() => {
      expect(within(table).getByRole('button', { name: '.zip' })).toBeInTheDocument();
    });

    const zipButton = within(table).getByRole('button', { name: '.zip' });
    const zipRow = zipButton.closest('tr');
    expect(zipRow).not.toBeNull();
    expect(zipRow).toHaveAttribute('aria-selected', 'false');

    // Click a numeric cell, not the tiny label button: the whole row selects.
    const sizeCell = zipRow!.querySelectorAll('td')[2];
    expect(sizeCell).not.toBeUndefined();
    await user.click(sizeCell!);

    expect(zipRow).toHaveAttribute('aria-selected', 'true');
    expect(zipRow!.className).toContain('rowSelected');

    const grid = within(types).getByRole('grid', { name: 'Типы файлов' });
    await waitFor(() => {
      expect(within(grid).getByText('archive.zip', { exact: true })).toBeInTheDocument();
    });
  });

  it('puts directories in the folder category and keeps volume roots out', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Analysis />);

    await user.click(screen.getByRole('tab', { name: 'Типы файлов' }));
    const types = panel('types');
    await user.click(within(types).getByRole('radio', { name: 'По категориям' }));

    const table = within(types).getByRole('table', { name: 'Типы файлов' });
    const folder = await within(table).findByRole('button', { name: 'Каталоги' });
    await user.click(folder);

    const grid = within(types).getByRole('grid', { name: 'Типы файлов' });
    await waitFor(() => {
      expect(within(grid).getByText('Windows', { exact: true })).toBeInTheDocument();
    });
    expect(within(grid).queryByText('C:\\', { exact: true })).not.toBeInTheDocument();
    expect(within(grid).queryByText('D:\\', { exact: true })).not.toBeInTheDocument();
  });

  it('shows the per-tab counts and totals', async () => {
    renderWithI18n(<Analysis />);

    await waitFor(() => {
      expect(screen.getByTestId('analysis-top-shown')).toHaveTextContent('13');
    });
    expect(screen.getByTestId('analysis-top-total')).toHaveTextContent('13');
    expect(screen.getByTestId('analysis-top-size')).toHaveTextContent('77.3 GiB');

    await waitFor(() => {
      expect(screen.getByTestId('analysis-types-groups')).toHaveTextContent('8');
    });
    expect(screen.getByTestId('analysis-types-files')).toHaveTextContent('13');
    expect(screen.getByTestId('analysis-types-size')).toHaveTextContent('77.3 GiB');

    // The report only counts once the owner totals have been read.
    expect(screen.queryByTestId('analysis-owners-count')).not.toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole('tab', { name: 'Владельцы' }));
    await user.click(
      within(panel('owners')).getAllByRole('button', {
        name: 'Построить отчёт по владельцам',
      })[0]!,
    );

    await waitFor(() => {
      expect(screen.getByTestId('analysis-owners-count')).toHaveTextContent('7');
    });
    expect(screen.getByTestId('analysis-owners-size')).toHaveTextContent('83 GiB');
    expect(screen.getByTestId('analysis-owners-unknown')).toHaveTextContent('0 B');
  });

  it('sorts the largest files by full path from the Объект column', async () => {
    const user = userEvent.setup();
    const invoke = vi.spyOn(api.transport, 'invoke');
    renderWithI18n(<Analysis />);
    await within(panel('top')).findByText('pagefile.sys');

    const header = within(panel('top')).getByRole('button', { name: 'Объект' });
    await user.click(header);
    expect(invoke).toHaveBeenCalledWith(
      'top_files_page',
      expect.objectContaining({ sort: { key: 'path', descending: true } }),
    );

    await user.click(header);
    expect(invoke).toHaveBeenLastCalledWith(
      'top_files_page',
      expect.objectContaining({ sort: { key: 'path', descending: false } }),
    );
    expect(header.closest('[role="columnheader"]')).toHaveAttribute('aria-sort', 'ascending');
    invoke.mockRestore();
  });

  it('uses an accessible icon-only open-in-tree action', async () => {
    renderWithI18n(<Analysis />);
    const row = await within(panel('top')).findByRole('row', { name: /pagefile\.sys/ });
    const button = within(row).getByRole('button', { name: 'Открыть в дереве' });

    expect(button.querySelector('svg')).not.toBeNull();
    expect(button).toHaveAttribute('title', 'Открыть в дереве');
    expect(button).toHaveTextContent('');
  });

  it('prints the modified date as DD.MM.YYYY', async () => {
    renderWithI18n(<Analysis />);
    const row = await within(panel('top')).findByRole('row', { name: /pagefile\.sys/ });
    const cells = within(row).getAllByRole('gridcell');

    expect(cells[4]).toHaveTextContent(/^\d{2}\.\d{2}\.\d{4}$/);
  });

  it('shows the full object information on hover', async () => {
    renderWithI18n(<Analysis />);
    const row = await within(panel('top')).findByRole('row', { name: /pagefile\.sys/ });

    row.setAttribute('data-focus-visible', 'true');
    row.focus();

    const card = await screen.findByRole('tooltip', { name: 'Детали размера' }, { timeout: 2000 });
    expect(within(card).getByText('pagefile.sys')).toBeInTheDocument();
    expect(within(card).getByText('C:\\pagefile.sys')).toBeInTheDocument();
    expect(within(card).getByText('Изменён')).toBeInTheDocument();
  });
});

describe('duplicates screen', () => {
  it('preselects copies from the keep rule and opens the guarded preview', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Duplicates />);

    await user.click(screen.getByRole('button', { name: 'Найти дубликаты' }));
    const oldest = await screen.findByRole(
      'checkbox',
      { name: 'удалить: backup-1.bin' },
      { timeout: 5000 },
    );
    const newest = screen.getByRole('checkbox', { name: 'удалить: backup-2.bin' });

    // backup-2 is the older copy, so it is kept and everything else is preselected.
    expect(oldest).toBeChecked();
    expect(newest).not.toBeChecked();
    expect(screen.getByTestId('duplicates-groups')).toHaveTextContent('1');
    expect(screen.getByTestId('duplicates-copies')).toHaveTextContent('2');
    expect(screen.getByTestId('duplicates-reclaimable')).toHaveTextContent('512 MiB');
    expect(screen.getByTestId('duplicates-selected')).toHaveTextContent('1 · 512 MiB');
    expect(screen.getByTestId('duplicates-group-1-size')).toHaveTextContent('512 MiB');

    // Changing the rule re-applies the preselection without a new search.
    await user.selectOptions(screen.getByLabelText('Правило выбора копии'), 'newest');

    expect(oldest).not.toBeChecked();
    expect(newest).toBeChecked();
    expect(screen.getByTestId('duplicates-selected')).toHaveTextContent('1 · 512 MiB');

    // The kept copy cannot join the deletion list: at least one must stay.
    await user.click(oldest);
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Хотя бы одна копия в каждой группе должна остаться.',
    );

    await user.click(screen.getByRole('button', { name: 'Удалить выбранные (1)' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText(/Объектов: 1/)).toBeInTheDocument();
  });

  it('filters duplicate groups by path', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Duplicates />);

    await user.click(screen.getByRole('button', { name: 'Найти дубликаты' }));
    await screen.findByTestId('duplicates-group-1-size', {}, { timeout: 5000 });

    await user.type(screen.getByRole('searchbox', { name: 'Текст' }), 'nope');

    expect(await screen.findByText('По фильтру ничего не найдено.')).toBeInTheDocument();
    expect(
      screen.queryByRole('checkbox', { name: 'удалить: backup-1.bin' }),
    ).not.toBeInTheDocument();
  });
});

describe('absorbed analysis screens', () => {
  it('recommends cleanup candidates and opens the preview', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Cleanup />);

    expect(await screen.findByText(/crash\.dmp/)).toBeInTheDocument();
    const grid = screen.getByRole('grid', { name: 'Рекомендации' });
    const row = within(grid).getByRole('row', { name: /crash\.dmp/ });
    await user.click(within(row).getByRole('checkbox'));
    await user.click(await screen.findByRole('button', { name: 'Удалить выбранные (1)' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
  });

  it('creates two snapshots and compares them', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Snapshots />);

    const create = screen.getByRole('button', { name: 'Создать снимок' });
    await user.click(create);
    await waitFor(() => {
      expect(screen.getAllByRole('option').length).toBeGreaterThan(1);
    });
    await user.click(screen.getByRole('button', { name: 'Создать снимок' }));
    await user.click(screen.getByRole('button', { name: 'Сравнить' }));
    expect(await screen.findByText('Users\\Downloads\\archive.zip')).toBeInTheDocument();
  });

  it('renders shared filters with active conditions', async () => {
    const user = userEvent.setup();
    const changed: string[] = [];
    renderWithI18n(
      <FilterBar
        filter={emptyFilter()}
        onChange={(filter) => changed.push(filter.text)}
      />,
    );

    expect(screen.getByRole('region', { name: 'Фильтры' })).toBeInTheDocument();
    await user.type(screen.getByRole('searchbox', { name: 'Текст' }), 'a');
    expect(changed).toContain('a');
    await user.click(screen.getByRole('button', { name: 'Видео' }));
    expect(changed.length).toBeGreaterThan(1);
  });

  it('does not offer saving or reusing named filters', () => {
    renderWithI18n(<FilterBar filter={emptyFilter()} onChange={vi.fn()} />);

    expect(screen.queryByRole('button', { name: 'Сохранить фильтр' })).not.toBeInTheDocument();
    expect(screen.queryByRole('combobox', { name: 'Сохранённые фильтры' })).not.toBeInTheDocument();
  });

  it('keeps the filter search focused while the listing reloads', async () => {
    const user = userEvent.setup();

    function Harness() {
      const [filter, setFilter] = useState(emptyFilter());

      return (
        <FilterBar
          filter={filter}
          onChange={setFilter}
          disabled
        />
      );
    }

    renderWithI18n(<Harness />);
    const search = screen.getByRole('searchbox', { name: 'Текст' });

    await user.click(search);
    await user.keyboard('logo');

    expect(search).toHaveValue('logo');
    expect(search).toHaveFocus();
  });
});
