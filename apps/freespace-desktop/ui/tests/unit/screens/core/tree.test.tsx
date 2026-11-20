import { screen, within } from '@testing-library/react';
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

    expect(screen.getByText('Сначала просканируйте том или каталог.')).toBeInTheDocument();
  });

  it('drills into a directory and shows details on hover', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    await user.dblClick(root);
    const windows = await screen.findByRole('row', { name: /Windows(?!\.)/ });

    await user.hover(windows);

    // The card waits for the pointer to settle before it opens.
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    const card = await screen.findByRole('tooltip', { name: 'Детали размера' });
    expect(within(card).getByText('Логический размер')).toBeInTheDocument();
    expect(within(card).getByText('Каталог')).toBeInTheDocument();

    await user.unhover(windows);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('expands and collapses a directory in place', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    await user.click(
      within(root).getByRole('button', { name: 'Развернуть: C:\\' }),
    );

    const windows = await screen.findByRole('row', { name: /Windows(?!\.)/ });
    expect(windows).toHaveAttribute('aria-level', '2');

    await user.click(
      within(root).getByRole('button', { name: 'Свернуть: C:\\' }),
    );
    expect(
      screen.queryByRole('row', { name: /Windows(?!\.)/ }),
    ).not.toBeInTheDocument();
  });
});
