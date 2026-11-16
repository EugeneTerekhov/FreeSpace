import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TopFiles } from '../../../../src/screens/TopFiles';
import { Types } from '../../../../src/screens/Types';
import { Duplicates } from '../../../../src/screens/Duplicates';
import { Cleanup } from '../../../../src/screens/Cleanup';
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

beforeEach(() => {
  useSnapshotStore.getState().setSnapshot(2, volume);
});

afterEach(() => {
  useSnapshotStore.getState().reset();
  vi.restoreAllMocks();
});

describe('analysis screens', () => {
  it('lists top files, filters them and opens the preview', async () => {
    const user = userEvent.setup();
    renderWithI18n(<TopFiles />);

    expect(await screen.findByText('pagefile.sys')).toBeInTheDocument();
    await user.type(screen.getByRole('searchbox', { name: 'Текст' }), 'crash');
    expect(await screen.findByText('crash.dmp')).toBeInTheDocument();

    await user.click(screen.getByRole('checkbox', { name: /crash\.dmp/ }));
    await user.click(await screen.findByRole('button', { name: 'Удалить выбранные (1)' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText(/Объектов: 1/)).toBeInTheDocument();
  });

  it('shows type totals and drills into files', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Types />);

    const zip = await screen.findByRole('button', { name: '.zip' });
    await user.click(zip);
    expect(await screen.findByText('archive.zip')).toBeInTheDocument();

    await user.click(screen.getByRole('radio', { name: 'По категориям' }));
    expect(
      (await screen.findAllByRole('button', { name: 'Архивы' })).length,
    ).toBeGreaterThan(0);  });

  it('finds duplicates, keeps one copy and opens the guarded preview', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Duplicates />);

    await user.click(screen.getByRole('button', { name: 'Найти дубликаты' }));
    expect(
      await screen.findByText(/backup-1\.bin/, {}, { timeout: 5000 }),
    ).toBeInTheDocument();
    expect(screen.getByText(/backup-2\.bin/)).toBeInTheDocument();

    const group = screen.getByRole('table', { name: 'Дубликаты' });
    const checkboxes = within(group).getAllByRole('checkbox');
    await user.click(checkboxes[0]!);
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Хотя бы одна копия в каждой группе должна остаться.',
    );
  });

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
