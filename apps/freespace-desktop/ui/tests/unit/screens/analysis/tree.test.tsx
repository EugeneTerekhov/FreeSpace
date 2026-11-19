import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { NodeRow } from '../../../../src/api/dto';
import { api } from '../../../../src/api/runtime';
import {
  isVolumeRow,
  sharePercent,
  treeIconKind,
} from '../../../../src/components/TreeRow';
import { Tree } from '../../../../src/screens/Tree';
import { useSnapshotStore } from '../../../../src/stores/snapshotStore';
import { renderWithI18n } from '../../../helpers/render';

afterEach(() => {
  useSnapshotStore.getState().reset();
});

function node(overrides: Partial<NodeRow>): NodeRow {
  return {
    id: 1,
    parent_id: 0,
    name: 'file.bin',
    path: 'C:\\file.bin',
    kind: 'file',
    logical_size: 1024,
    on_disk_size: 1024,
    physical_size: 1024,
    ads_size: 0,
    child_count: 0,
    file_count: 1,
    modified_at: null,
    accessed_at: null,
    attributes: {
      read_only: false,
      hidden: false,
      system: false,
      compressed: false,
      sparse: false,
      reparse: false,
    },
    accessible: true,
    link_target: null,
    ...overrides,
  };
}

function iconOf(row: HTMLElement): string | null {
  return row.querySelector('[data-tree-icon]')?.getAttribute('data-tree-icon') ?? null;
}

function shareOf(row: HTMLElement): string | null {
  return row.querySelector('[data-tree-fill]')?.getAttribute('data-share') ?? null;
}

describe('tree row shares', () => {
  it('reports the whole-percent share of the parent', () => {
    expect(sharePercent(180, 760)).toBe(24);
    expect(sharePercent(760, 760)).toBe(100);
    expect(sharePercent(1, 3)).toBe(33);
    expect(sharePercent(900, 760)).toBe(100);
  });

  it('never invents a share for empty or unknown sizes', () => {
    expect(sharePercent(0, 760)).toBe(0);
    expect(sharePercent(5, 0)).toBe(0);
    expect(sharePercent(Number.NaN, 10)).toBe(0);
    expect(sharePercent(-5, 10)).toBe(0);
  });

  it('paints the root in full and its children by their parent size', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    expect(shareOf(root)).toBe('100');

    await user.click(within(root).getByRole('button', { name: 'Развернуть: C:\\' }));

    const windows = await screen.findByRole('row', { name: /Windows(?!\.)/ });
    // 180 GiB of the 760 GiB volume, rounded to whole percent.
    expect(shareOf(windows)).toBe('24');

    // The bar must clear the indent and the chevron.
    const fill = windows.querySelector<HTMLElement>('[data-tree-fill]');
    expect(fill?.style.left).toContain('var(--fs-space-3)');
  });
});

describe('tree row icons', () => {
  it('treats only drive roots as volumes', () => {
    expect(isVolumeRow(node({ kind: 'directory', path: 'E:\\', name: 'E:\\' }))).toBe(
      true,
    );
    expect(isVolumeRow(node({ kind: 'directory', path: 'C:', name: 'C:' }))).toBe(true);
    expect(
      isVolumeRow(node({ kind: 'directory', path: 'C:\\Windows', name: 'Windows' })),
    ).toBe(false);
    expect(isVolumeRow(node({ kind: 'file', path: 'E:\\', name: 'E:\\' }))).toBe(false);
  });

  it('chooses an icon per type', () => {
    expect(treeIconKind(node({ kind: 'directory', name: 'Windows' }))).toBe('folder');
    expect(treeIconKind(node({ kind: 'directory', name: 'Windows' }), true)).toBe(
      'folder-open',
    );
    expect(treeIconKind(node({ kind: 'directory', name: 'C:\\', path: 'C:\\' }))).toBe(
      'volume',
    );
    expect(treeIconKind(node({ name: 'report.pdf' }))).toBe('text');
    expect(treeIconKind(node({ name: 'photo.JPG' }))).toBe('image');
    expect(treeIconKind(node({ name: 'vacation.mkv' }))).toBe('video');
    expect(treeIconKind(node({ name: 'backup.7z' }))).toBe('archive');
    expect(treeIconKind(node({ name: 'disk.vdi' }))).toBe('volume');
    expect(treeIconKind(node({ name: 'explorer.exe' }))).toBe('file');
    expect(treeIconKind(node({ name: 'README' }))).toBe('file');
  });

  it('starts on the folder and flips to the open folder', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    expect(iconOf(root)).toBe('volume');

    await user.click(within(root).getByRole('button', { name: 'Развернуть: C:\\' }));

    const windows = await screen.findByRole('row', { name: /Windows(?!\.)/ });
    expect(iconOf(windows)).toBe('folder');

    await user.click(within(windows).getByRole('button', { name: 'Развернуть: Windows' }));

    const open = await screen.findByRole('row', { name: /Windows(?!\.)/ });
    expect(iconOf(open)).toBe('folder-open');
  });
});

describe('tree keyboard', () => {
  it('expands with ArrowRight and collapses with ArrowLeft', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    expect(root).toHaveAttribute('aria-expanded', 'false');

    root.focus();
    await user.keyboard('{ArrowRight}');
    expect(root).toHaveAttribute('aria-expanded', 'true');
    expect(await screen.findByRole('row', { name: /Windows(?!\.)/ })).toBeInTheDocument();

    await user.keyboard('{ArrowLeft}');
    expect(root).toHaveAttribute('aria-expanded', 'false');
    expect(
      screen.queryByRole('row', { name: /Windows(?!\.)/ }),
    ).not.toBeInTheDocument();
  });
});

describe('tree size details', () => {
  it('opens a dense card on hover and follows the row under the pointer', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    await user.dblClick(root);

    const windows = await screen.findByRole('row', { name: /Windows(?!\.)/ });
    await user.hover(windows);

    const card = await screen.findByRole('tooltip', { name: 'Детали размера' });
    expect(within(card).getByText('C:\\Windows')).toBeInTheDocument();
    expect(within(card).getByText('Логический размер')).toBeInTheDocument();
    expect(within(card).getByText('Каталог')).toBeInTheDocument();

    // The card belongs to the hovered row, not to the selection: moving the
    // pointer to another row replaces its content.
    const pagefile = await screen.findByRole('row', { name: /pagefile\.sys/ });
    await user.hover(pagefile);
    expect(
      within(await screen.findByRole('tooltip', { name: 'Детали размера' })).getByText(
        'C:\\pagefile.sys',
      ),
    ).toBeInTheDocument();
  });

  it('hides the card when the pointer leaves the table', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    await user.hover(root);
    await screen.findByRole('tooltip', { name: 'Детали размера' });

    await user.unhover(root);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('labels a drive root as a volume and hides its hidden/system badges', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    await user.hover(root);

    const card = await screen.findByRole('tooltip', { name: 'Детали размера' });
    expect(within(card).getByText('Том')).toBeInTheDocument();
    expect(within(card).queryByText('Скрытый')).not.toBeInTheDocument();
    expect(within(card).queryByText('Системный')).not.toBeInTheDocument();
  });
});

describe('tree context menu', () => {
  async function openMenuFor(name: RegExp): Promise<HTMLElement> {
    const root = await screen.findByRole('row', { name: /C:\\/ });
    const toggle = within(root).queryByRole('button', {
      name: 'Развернуть: C:\\',
    });
    if (toggle !== null) {
      fireEvent.click(toggle);
      await screen.findByRole('row', { name: /^Temp\b/ });
    }

    const row = await screen.findByRole('row', { name });
    fireEvent.contextMenu(row, { clientX: 40, clientY: 60 });

    return screen.findByRole('menu', { name: /Действия/ });
  }

  it('opens on a right click with the row commands', async () => {
    useSnapshotStore.getState().setGeneration(1);
    renderWithI18n(<Tree />);

    const menu = await openMenuFor(/^Temp\b/);

    expect(
      within(menu).getByRole('menuitem', { name: 'Открыть в Проводнике' }),
    ).toBeInTheDocument();
    expect(
      within(menu).getByRole('menuitem', { name: 'Скопировать путь' }),
    ).toBeEnabled();
    expect(
      within(menu).getByRole('menuitem', { name: 'Переименовать' }),
    ).toBeEnabled();
    expect(
      within(menu).getByRole('menuitem', { name: 'Удалить в Корзину' }),
    ).toBeEnabled();
  });

  it('does not rename or recycle a volume root', async () => {
    useSnapshotStore.getState().setGeneration(1);
    renderWithI18n(<Tree />);

    const menu = await openMenuFor(/C:\\/);

    expect(
      within(menu).getByRole('menuitem', { name: 'Переименовать' }),
    ).toBeDisabled();
    expect(
      within(menu).getByRole('menuitem', { name: 'Удалить в Корзину' }),
    ).toBeDisabled();
  });

  it('opens from the keyboard with Shift+F10', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const root = await screen.findByRole('row', { name: /C:\\/ });
    fireEvent.click(
      within(root).getByRole('button', { name: 'Развернуть: C:\\' }),
    );
    const row = await screen.findByRole('row', { name: /^Temp\b/ });
    row.focus();
    await user.keyboard('{Shift>}{F10}{/Shift}');

    expect(await screen.findByRole('menu', { name: /Действия/ })).toBeInTheDocument();
  });

  it('reveals the object in Explorer through the native command', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    const invoke = vi.spyOn(api.transport, 'invoke');
    renderWithI18n(<Tree />);

    const menu = await openMenuFor(/^Temp\b/);
    await user.click(
      within(menu).getByRole('menuitem', { name: 'Открыть в Проводнике' }),
    );

    expect(invoke).toHaveBeenCalledWith('reveal_in_explorer', {
      path: 'C:\\Temp',
    });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    invoke.mockRestore();
  });

  it('keeps the deleted row hidden after leaving and re-entering the tree', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    const first = renderWithI18n(<Tree />);

    const menu = await openMenuFor(/^Temp\b/);
    await user.click(
      within(menu).getByRole('menuitem', { name: 'Удалить в Корзину' }),
    );
    const dialog = await screen.findByRole('dialog', {
      name: 'Удаление в Корзину',
    });
    await user.click(
      within(dialog).getByRole('button', { name: 'Удалить в Корзину' }),
    );
    await vi.waitFor(() => {
      expect(screen.queryByRole('row', { name: /^Temp\b/ })).not.toBeInTheDocument();
    });

    first.unmount();
    renderWithI18n(<Tree />);
    const root = await screen.findByRole('row', { name: /C:\\/ });
    fireEvent.click(
      within(root).getByRole('button', { name: 'Развернуть: C:\\' }),
    );
    await screen.findByRole('row', { name: /Windows(?!\.)/ });
    expect(screen.queryByRole('row', { name: /^Temp\b/ })).not.toBeInTheDocument();
  });

  it('uses the file wording for a file target', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const menu = await openMenuFor(/pagefile\.sys/);
    await user.click(
      within(menu).getByRole('menuitem', { name: 'Удалить в Корзину' }),
    );

    const dialog = await screen.findByRole('dialog', {
      name: 'Удаление в Корзину',
    });
    expect(
      within(dialog).getByText('Вы действительно хотите удалить этот файл?'),
    ).toBeInTheDocument();
    expect(within(dialog).getByText('Файл')).toBeInTheDocument();
    expect(within(dialog).getByText('16 GiB')).toBeInTheDocument();
  });

  it('copies the full path to the clipboard', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const menu = await openMenuFor(/^Temp\b/);
    await user.click(
      within(menu).getByRole('menuitem', { name: 'Скопировать путь' }),
    );

    await expect(navigator.clipboard.readText()).resolves.toBe('C:\\Temp');
    expect(
      screen.getByText('Путь скопирован в буфер обмена.'),
    ).toBeInTheDocument();
  });

  it('asks a compact question with the object details and confirms recycling', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const menu = await openMenuFor(/^Temp\b/);
    await user.click(
      within(menu).getByRole('menuitem', { name: 'Удалить в Корзину' }),
    );

    const dialog = await screen.findByRole('dialog', {
      name: 'Удаление в Корзину',
    });
    expect(
      within(dialog).getByText('Вы действительно хотите удалить этот каталог?'),
    ).toBeInTheDocument();
    expect(within(dialog).getByText('Temp')).toBeInTheDocument();
    expect(within(dialog).getByText('Каталог')).toBeInTheDocument();
    expect(within(dialog).getByText('5 GiB')).toBeInTheDocument();
    expect(within(dialog).getByText('Изменён')).toBeInTheDocument();
    expect(within(dialog).queryByRole('radio')).not.toBeInTheDocument();

    await user.click(
      within(dialog).getByRole('button', { name: 'Удалить в Корзину' }),
    );

    await vi.waitFor(() => {
      expect(screen.queryByRole('row', { name: /^Temp\b/ })).not.toBeInTheDocument();
    });
    expect(screen.getByText(/перемещён в Корзину/)).toBeInTheDocument();
  });

  it('previews a new name and renames the row after confirmation', async () => {
    useSnapshotStore.getState().setGeneration(1);
    const user = userEvent.setup();
    renderWithI18n(<Tree />);

    const menu = await openMenuFor(/^Temp\b/);
    await user.click(within(menu).getByRole('menuitem', { name: 'Переименовать' }));

    const dialog = await screen.findByRole('dialog', {
      name: 'Переименование объекта',
    });
    const field = within(dialog).getByLabelText(/Новое имя/);
    await user.clear(field);
    await user.type(field, 'Temporary');
    await user.click(within(dialog).getByRole('button', { name: 'Предпросмотр' }));

    expect(await within(dialog).findByText('Temporary')).toBeInTheDocument();
    const apply = within(dialog).getByRole('button', { name: 'Переименовать' });
    expect(apply).toBeDisabled();

    await user.click(
      within(dialog).getByRole('checkbox', {
        name: 'Я просмотрел предпросмотр и подтверждаю переименование',
      }),
    );
    expect(apply).toBeEnabled();
    await user.click(apply);

    expect(await screen.findByRole('row', { name: /Temporary/ })).toBeInTheDocument();
    expect(screen.queryByRole('row', { name: /^Temp\b/ })).not.toBeInTheDocument();
  });
});
