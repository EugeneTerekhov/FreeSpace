import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { Button, IconButton } from '../../src/components/Button';
import {
  Badge,
  Checkbox,
  Chip,
  Radio,
  Segmented,
  Tabs,
  Toggle,
} from '../../src/components/Choice';
import { Combobox, Field, SearchField, Select } from '../../src/components/Fields';
import { Kbd, Tooltip } from '../../src/components/Tooltip';

describe('UI kit, part 1', () => {
  it('renders buttons with busy state and an accessible icon button', () => {
    render(
      <>
        <Button variant="primary" loading>
          Scan
        </Button>
        <IconButton label="Close window">×</IconButton>
      </>,
    );

    expect(screen.getByRole('button', { name: 'Scan' })).toHaveAttribute(
      'aria-busy',
      'true',
    );
    expect(screen.getByRole('button', { name: 'Close window' })).toBeEnabled();
  });

  it('wires labels, hints and errors to the input', () => {
    render(
      <Field id="path" label="Path" hint="Absolute path" required>
        <input id="path" aria-describedby="path-hint" />
      </Field>,
    );

    expect(screen.getByLabelText(/Path/)).toBeInTheDocument();
    expect(screen.getByText('Absolute path')).toBeInTheDocument();
  });

  it('clears the search field with the button and Escape', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    const { rerender } = render(
      <SearchField label="Search" value="report" onValueChange={onChange} />,
    );

    await user.click(screen.getByRole('button', { name: 'Очистить' }));
    expect(onChange).toHaveBeenCalledWith('');

    rerender(
      <SearchField label="Search" value="report" onValueChange={onChange} />,
    );
    await user.type(screen.getByRole('searchbox'), '{Escape}');
    expect(onChange).toHaveBeenCalledWith('');
  });

  it('selects a combobox option by typing and clicking', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <Combobox
        label="Volume"
        value=""
        onValueChange={onChange}
        options={[
          { value: 'c', label: 'C:\\' },
          { value: 'd', label: 'D:\\' },
        ]}
      />,
    );

    await user.click(screen.getByRole('combobox', { name: 'Volume' }));
    expect(screen.getByRole('listbox', { name: 'Volume' })).toBeInTheDocument();
    await user.click(screen.getByRole('option', { name: 'D:\\' }));
    expect(onChange).toHaveBeenCalledWith('d');
  });

  it('renders a native select', () => {
    render(
      <Select
        aria-label="Method"
        defaultValue="auto"
        options={[
          { value: 'auto', label: 'Auto' },
          { value: 'walk', label: 'Walk' },
        ]}
      />,
    );

    expect(screen.getByRole('combobox', { name: 'Method' })).toHaveValue('auto');
  });

  it('toggles a switch and checks native inputs', async () => {
    const user = userEvent.setup();
    const onCheckedChange = vi.fn();
    render(
      <>
        <Toggle label="Compact" checked={false} onCheckedChange={onCheckedChange} />
        <Checkbox id="hidden" label="Hidden" />
        <Radio id="binary" name="units" label="Binary" />
      </>,
    );

    await user.click(screen.getByRole('switch', { name: 'Compact' }));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
    await user.click(screen.getByLabelText('Hidden'));
    expect(screen.getByLabelText('Hidden')).toBeChecked();
    await user.click(screen.getByLabelText('Binary'));
    expect(screen.getByLabelText('Binary')).toBeChecked();
  });

  it('moves between segmented options with the arrow keys', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <Segmented
        label="Density"
        value="comfortable"
        onChange={onChange}
        options={[
          { value: 'comfortable', label: 'Comfortable' },
          { value: 'compact', label: 'Compact' },
        ]}
      />,
    );

    await user.click(screen.getByRole('radio', { name: 'Comfortable' }));
    await user.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenCalledWith('compact');
  });

  it('navigates tabs and chips with ARIA state', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(
      <>
        <Tabs
          label="Views"
          value="extensions"
          onChange={onChange}
          items={[
            { value: 'extensions', label: 'Extensions' },
            { value: 'categories', label: 'Categories' },
          ]}
        />
        <Chip selected onClick={() => undefined} onRemove={() => undefined}>
          PDF
        </Chip>
        <Badge tone="danger">Missing</Badge>
      </>,
    );

    await user.click(screen.getByRole('tab', { name: 'Extensions' }));
    await user.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenCalledWith('categories');
    expect(screen.getByRole('button', { pressed: true })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Remove' })).toBeInTheDocument();
  });

  it('shows a tooltip on focus', async () => {
    const user = userEvent.setup();
    render(
      <Tooltip content="Ctrl+K">
        <button type="button">Commands</button>
      </Tooltip>,
    );

    await user.tab();
    expect(screen.getByRole('tooltip')).toHaveTextContent('Ctrl+K');
    expect(screen.getByText('Ctrl+K')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Commands' })).toBeInTheDocument();
    expect(screen.getByText('Ctrl+K')).toBeInTheDocument();
  });

  it('renders a keyboard hint', () => {
    render(<Kbd>Ctrl+K</Kbd>);
    expect(screen.getByText('Ctrl+K')).toBeInTheDocument();
  });
});
