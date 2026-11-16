import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import {
  AppShell,
  Breadcrumbs,
  Header,
  Sidebar,
  SplitPane,
  StatusBar,
} from '../../src/app/AppShell';

describe('shell', () => {
  it('renders the documented landmarks', () => {
    render(
      <AppShell
        sidebar={
          <Sidebar
            items={[{ id: 'dashboard', label: 'Disks' }]}
            active="dashboard"
            onSelect={() => undefined}
          />
        }
        header={<Header title="Disks" />}
        status={<StatusBar message="Ready" />}
      >
        Body
      </AppShell>,
    );

    expect(screen.getByRole('navigation', { name: 'Экраны' })).toBeInTheDocument();
    expect(screen.getByRole('banner')).toBeInTheDocument();
    expect(screen.getByRole('main')).toHaveTextContent('Body');
    expect(screen.getByRole('status')).toHaveTextContent('Ready');
  });

  it('marks the active screen and selects another one', async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <Sidebar
        items={[
          { id: 'dashboard', label: 'Disks' },
          { id: 'tree', label: 'Tree' },
        ]}
        active="dashboard"
        onSelect={onSelect}
      />,
    );

    expect(screen.getByRole('button', { name: 'Disks' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    await user.click(screen.getByRole('button', { name: 'Tree' }));
    expect(onSelect).toHaveBeenCalledWith('tree');
  });

  it('renders one h1 and breadcrumbs with a current page', () => {
    render(
      <>
        <Header title="Tree" subtitle="C:\\" />
        <Breadcrumbs items={[{ label: 'C:\\' }, { label: 'Users' }]} />
      </>,

    );

    expect(screen.getByRole('heading', { level: 1, name: 'Tree' })).toBeInTheDocument();
    expect(
      screen.getByRole('navigation', { name: 'Навигационная цепочка' }),
    ).toBeInTheDocument();
    expect(screen.getByText('Users')).toHaveAttribute('aria-current', 'page');
  });

  it('resizes the split pane with the keyboard', async () => {
    const user = userEvent.setup();
    render(<SplitPane left={<div>Left</div>} right={<div>Right</div>} />);

    const separator = screen.getByRole('separator', {
      name: 'Изменить ширину панелей',
    });
    expect(separator).toHaveAttribute('aria-valuenow', '60');
    separator.focus();
    await user.keyboard('{ArrowLeft}');
    expect(separator).toHaveAttribute('aria-valuenow', '55');
    await user.keyboard('{ArrowRight}');
    expect(separator).toHaveAttribute('aria-valuenow', '60');
    expect(screen.getByText('Left')).toBeInTheDocument();
  });
});
