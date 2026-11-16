import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from '../../../src/api/runtime';
import type { OperationPreview, OperationProgress } from '../../../src/api/dto';
import { OperationPreviewDialog } from '../../../src/components/OperationPreviewDialog';
import { OperationProgressView } from '../../../src/components/OperationProgress';
import { Journal } from '../../../src/screens/Journal';
import { needsElevation, elevationError } from '../../../src/operations/elevation';
import { renderWithI18n } from '../../helpers/render';

function preview(overrides: Partial<OperationPreview> = {}): OperationPreview {
  return {
    preview_id: 'preview-1',
    kind: 'delete',
    targets: [
      {
        path: 'C:\\Temp\\victim.txt',
        kind: 'file',
        size: 12,
        accessible: true,
        is_system: false,
      },
    ],
    total_size: 12,
    delete_mode: 'recycle_bin',
    reversible: true,
    requires_elevation: false,
    warnings: [],
    expires_at: Date.now() + 600_000,
    confirmation_token: 'token-1',
    ...overrides,
  };
}

describe('operation preview', () => {
  it('requires an acknowledgement and an extra confirmation for permanent deletion', async () => {
    const user = userEvent.setup();
    const confirm = vi.fn();
    renderWithI18n(
      <OperationPreviewDialog
        open
        preview={preview()}
        error={null}
        onCancel={vi.fn()}
        onConfirm={confirm}
      />,
    );

    const apply = screen.getByRole('button', { name: 'Удалить' });
    expect(apply).toBeDisabled();

    await user.click(screen.getByRole('radio', { name: /Безвозвратное удаление/ }));
    await user.click(screen.getByRole('checkbox', { name: 'Я просмотрел объекты и подтверждаю операцию' }));
    expect(apply).toBeDisabled();

    await user.click(
      screen.getByRole('checkbox', {
        name: 'Я понимаю, что путь защищён, и подтверждаю действие с повышенными правами',
      }),
    );
    expect(apply).toBeEnabled();
    await user.click(apply);
    expect(confirm).toHaveBeenCalledWith('permanent', true);
  });

  it('shows targets and warnings from the preview', () => {
    renderWithI18n(
      <OperationPreviewDialog
        open
        preview={preview({
          requires_elevation: true,
          warnings: ['confirm.warning.protected'],
        })}
        error={null}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );

    expect(screen.getByText('C:\\Temp\\victim.txt')).toBeInTheDocument();
    expect(screen.getByText(/защищённые системные пути/)).toBeInTheDocument();
  });

  it('reports elevation requirements from the preview', () => {
    expect(needsElevation(preview())).toBe(false);
    expect(elevationError(preview())).toBeNull();
    const elevated = preview({ requires_elevation: true });
    expect(needsElevation(elevated)).toBe(true);
    expect(elevationError(elevated)?.code).toBe('E_ELEVATION_REQUIRED');
  });
});

describe('operation progress', () => {
  it('offers cancel while running and undo when available', async () => {
    const user = userEvent.setup();
    const cancel = vi.fn();
    const undo = vi.fn();
    const running: OperationProgress = {
      operation_id: 'operation-1',
      state: 'running',
      processed: 1,
      total: 3,
      current_target: 'C:\\Temp\\victim.txt',
      bytes_per_second: null,
      errors: [],
      undo_available: false,
      journal_entry_id: null,
    };
    const { rerender } = renderWithI18n(
      <OperationProgressView progress={running} onCancel={cancel} onUndo={undo} />,
    );
    await user.click(screen.getByRole('button', { name: 'Отменить операцию' }));
    expect(cancel).toHaveBeenCalled();

    rerender(
      <OperationProgressView
        progress={{ ...running, state: 'completed', undo_available: true }}
        onCancel={cancel}
        onUndo={undo}
      />,
    );
    await user.click(screen.getByRole('button', { name: 'Отменить изменения' }));
    expect(undo).toHaveBeenCalled();
  });
});

describe('journal', () => {
  it('records a confirmed operation and restores it', async () => {
    const created = await api.operationPreview({
      kind: 'delete',
      targets: ['C:\\Temp\\victim.txt'],
      delete_mode: 'quarantine',
    });
    await api.operationConfirm(
      created.preview_id,
      created.confirmation_token,
      'quarantine',
      false,
    );

    const user = userEvent.setup();
    renderWithI18n(<Journal />);
    expect(await screen.findByText('mock-user')).toBeInTheDocument();
    const restore = screen.getByRole('button', { name: 'Восстановить' });
    await user.click(restore);
    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveTextContent('Восстановлено');
    });
  });
});
