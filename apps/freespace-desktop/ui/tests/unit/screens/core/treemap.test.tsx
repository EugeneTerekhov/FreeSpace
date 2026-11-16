import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import { Treemap } from '../../../../src/screens/Treemap';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { renderWithI18n } from '../../../helpers/render';

afterEach(() => {
  useSnapshotStore.getState().reset();
});

describe('Treemap screen', () => {
  it('asks for a scan without a generation', () => {
    renderWithI18n(<Treemap />);

    expect(screen.getByText('Просканируйте том или каталог, чтобы увидеть карту.')).toBeInTheDocument();
  });

  it('renders tiles and selects one with the keyboard', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Treemap />);

    const canvas = await screen.findByRole('img', { name: /Щёлкните плитку/ });
    expect(screen.getByText('Видео')).toBeInTheDocument();

    canvas.focus();
    await user.keyboard('{ArrowRight}{Enter}');

    expect(
      await screen.findByText('Логический размер'),
    ).toBeInTheDocument();
  });
});
