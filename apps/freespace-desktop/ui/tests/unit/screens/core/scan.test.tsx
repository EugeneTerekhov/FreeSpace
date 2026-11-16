import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Scan } from '../../../../src/screens/Scan';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { useTasksStore } from '../../../../src/stores/tasksStore';
import { renderWithI18n } from '../../../helpers/render';

afterEach(() => {
  useTasksStore.getState().setTasks([]);
  useSnapshotStore.getState().reset();
  vi.clearAllTimers();
  vi.useRealTimers();
});

describe('Scan screen', () => {
  it('shows the empty prompt before a scan starts', () => {
    renderWithI18n(<Scan />);

    expect(screen.getByText('Сначала просканируйте том или каталог.')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Начать сканирование' }),
    ).toBeInTheDocument();
  });

  it('runs a simulated scan to completion', async () => {
    const user = userEvent.setup();
    renderWithI18n(<Scan />);

    await user.click(screen.getByRole('button', { name: 'Начать сканирование' }));

    expect(
      await screen.findByText(/Сканирование завершено/, {}, { timeout: 4_000 }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Открыть дерево' }),
    ).toBeInTheDocument();
    expect(useSnapshotStore.getState().generation).toBe(2);
  });
});
