import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { Analysis } from '../../../../src/screens/Analysis';
import { Cleanup } from '../../../../src/screens/Cleanup';
import { Duplicates } from '../../../../src/screens/Duplicates';
import { Snapshots } from '../../../../src/screens/Snapshots';
import { FilterBar } from '../../../../src/components/FilterBar';
import { emptyFilter } from '../../../../src/lib/filters';
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
    const table = screen.getByRole('table', { name: 'Рекомендации' });
    const checkbox = within(table).getAllByRole('checkbox')[0]!;
    await user.click(checkbox);
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
        screen="top_files"
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
});
