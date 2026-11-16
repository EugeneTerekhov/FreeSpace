import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import { Tree } from '../../../../src/screens/Tree';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { renderWithI18n } from '../../../helpers/render';

afterEach(() => {
  useSnapshotStore.getState().reset();
});

describe('Tree screen', () => {
  it('asks for a scan without a generation', () => {
    renderWithI18n(<Tree />);

    expect(screen.getByText('Сканирование дало пустое дерево.')).toBeInTheDocument();
  });

  it('drills into a directory and shows details', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    await user.dblClick(root);
    const windows = await screen.findByRole('row', { name: /Windows/ });
    await user.click(windows);

    expect(screen.getByText('Логический размер')).toBeInTheDocument();
    expect(screen.getAllByText('Каталог')[0]).toBeInTheDocument();
  });

  it('finds nested rows through search', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup({ delay: null });
    renderWithI18n(<Tree />);

    await screen.findByRole('row', { name: /C:\\/ });
    await user.type(screen.getByRole('searchbox', { name: 'Найти' }), 'explorer');
    await user.click(screen.getByRole('button', { name: 'Найти' }));

    expect(await screen.findByRole('row', { name: /explorer.exe/ })).toBeInTheDocument();
  });
});
