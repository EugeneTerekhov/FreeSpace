import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Dashboard } from '../../../../src/screens/Dashboard';
import { useShellStore } from '../../../../src/stores/shellStore';
import { renderWithI18n } from '../../../helpers/render';
import { sharedTransport } from '../../../helpers/mockTransport';

afterEach(() => {
  useShellStore.getState().setScreen('dashboard');
  vi.clearAllTimers();
  vi.restoreAllMocks();
});

describe('Dashboard screen', () => {
  it('renders volumes from the transport', async () => {
    renderWithI18n(<Dashboard />);

    const mounts = await screen.findAllByText('C:\\', { exact: true });
    expect(mounts.length).toBeGreaterThan(0);
    expect(screen.getByText('Data')).toBeInTheDocument();
    expect(
      screen.getAllByRole('button', { name: 'Анализировать этот том' }).length,
    ).toBe(2);
  });

  it('starts a scan and navigates to the scan screen', async () => {
    const transport = sharedTransport();
    const invoke = vi.spyOn(transport, 'invoke');
    const user = userEvent.setup();
    renderWithI18n(<Dashboard />);

    const buttons = await screen.findAllByRole('button', {
      name: 'Анализировать этот том',
    });
    await user.click(buttons[0]!);

    const scanCall = invoke.mock.calls.find(([command]) => command === 'scan_start');
    expect(scanCall).toBeDefined();
    const args = scanCall?.[1] as { request: { path: string } } | undefined;
    expect(args?.request.path).toBe('C:\\');
    expect(useShellStore.getState().screen).toBe('scan');
  });
});
