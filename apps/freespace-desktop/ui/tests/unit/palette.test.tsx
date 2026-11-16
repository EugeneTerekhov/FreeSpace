import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { CommandPalette } from '../../src/app/CommandPalette';

describe('CommandPalette', () => {
  it('filters commands and runs the active one', async () => {
    const user = userEvent.setup();
    const runTree = vi.fn();
    const onClose = vi.fn();
    render(
      <CommandPalette
        open
        onClose={onClose}
        commands={[
          { id: 'dashboard', label: 'Disks', run: vi.fn() },
          { id: 'tree', label: 'Tree', run: runTree },
        ]}
      />,
    );

    const input = screen.getByRole('combobox', { name: 'Type a command…' });
    await user.type(input, 'tree');
    expect(screen.getByRole('option', { name: 'Tree' })).toBeInTheDocument();
    await user.keyboard('{Enter}');

    expect(runTree).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it('supports arrow navigation and an empty state', async () => {
    const user = userEvent.setup();
    const runSecond = vi.fn();
    render(
      <CommandPalette
        open
        onClose={() => undefined}
        commands={[
          { id: 'one', label: 'One', run: vi.fn() },
          { id: 'two', label: 'Two', run: runSecond },
        ]}
      />,
    );

    const input = screen.getByRole('combobox');
    await user.type(input, '{ArrowDown}{Enter}');
    expect(runSecond).toHaveBeenCalled();

    await user.clear(input);
    await user.type(input, 'nothing');
    expect(screen.getByText('No matching commands')).toBeInTheDocument();
  });
});
