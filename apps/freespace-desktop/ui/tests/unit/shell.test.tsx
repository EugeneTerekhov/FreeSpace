import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import {
  AppShell,
  Breadcrumbs,
  Header,
  Sidebar,
  SidebarDrive,
  SplitPane,
  StatusBar,
} from '../../src/app/AppShell';

const GROUPS = [
  {
    id: 'overview',
    label: 'Обзор',
    items: [{ id: 'overview', label: 'Обзор' }],
  },
  {
    id: 'analysis',
    label: 'Анализ',
    items: [
      { id: 'tree', label: 'Дерево' },
      { id: 'analysis', label: 'Анализ' },
    ],
  },
];

describe('shell', () => {
  it('renders the documented landmarks', () => {
    render(
      <AppShell
        sidebar={<Sidebar groups={GROUPS} active="overview" onSelect={() => undefined} />}
        header={<Header title="Обзор" />}
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

  it('renders group headings and the application icon', () => {
    const { container } = render(
      <Sidebar groups={GROUPS} active="overview" onSelect={() => undefined} />,
    );

    expect(screen.getAllByText('Обзор').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Анализ').length).toBeGreaterThan(0);
    // The brand icon is decorative: rendered, but hidden from narrators.
    expect(container.querySelector('svg')).not.toBeNull();
  });

  it('marks the active screen and selects another one', async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(<Sidebar groups={GROUPS} active="overview" onSelect={onSelect} />);

    expect(screen.getByRole('button', { name: 'Обзор' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(screen.getByRole('button', { name: 'Дерево' })).not.toHaveAttribute(
      'aria-current',
    );

    await user.click(screen.getByRole('button', { name: 'Дерево' }));
    expect(onSelect).toHaveBeenCalledWith('tree');
  });

  it('shows the active drive in the sidebar footer', () => {
    render(
      <Sidebar
        groups={GROUPS}
        active="overview"
        onSelect={() => undefined}
        footer={<SidebarDrive mount="C:\\" free="288 ГБ" total="1 ТБ" />}
      />,
    );

    expect(screen.getByText(/288 ГБ/)).toBeInTheDocument();
    expect(screen.getByText('1 ТБ')).toBeInTheDocument();
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
