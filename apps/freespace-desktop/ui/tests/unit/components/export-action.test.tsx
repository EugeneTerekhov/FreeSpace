import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { UiError } from '../../../src/api/dto';
import type { CommandName } from '../../../src/api/transport';
import { ExportAction } from '../../../src/components/ExportAction';
import { t } from '../../../src/i18n';
import { sharedTransport } from '../../helpers/mockTransport';
import { renderWithI18n } from '../../helpers/render';

function uiError(code: UiError['code'], messageKey: string): UiError {
  return {
    code,
    category: 'io',
    message_key: messageKey,
    details: null,
    retryable: true,
    task_id: null,
    journal_entry_id: null,
  };
}

/** Wraps the shared transport so one command can be observed or replaced. */
function spyTransport(
  override: (
    command: CommandName,
    args: Record<string, unknown> | undefined,
  ) => Promise<unknown> | undefined,
): void {
  const transport = sharedTransport();
  const original = transport.invoke.bind(transport);

  vi.spyOn(transport, 'invoke').mockImplementation(
    ((
      command: CommandName,
      args?: Record<string, unknown>,
    ) => override(command, args) ?? original(command, args)),
  );
}

describe('export action', () => {
  it('offers every report format and exports the previewed scope', async () => {
    const user = userEvent.setup();
    let scope: unknown = null;

    spyTransport((command, args) => {
      if (command === 'export_preview') {
        scope = args?.scope;
      }
      if (command === 'export_status') {
        return Promise.resolve({
          task_id: 'export-task-1',
          state: 'completed',
          processed: 3,
          total: 3,
          phase_key: 'export.phase.done',
          out: 'C:\\Temp\\report.xml',
          bytes_written: 8192,
          rows_written: 18,
          cancelable: false,
          started_at: 0,
          finished_at: 1,
          error: null,
        });
      }

      return undefined;
    });

    renderWithI18n(
      <ExportAction generation={7} defaultOutput={'C:\\Temp\\report.csv'} />,
    );

    await user.click(screen.getByRole('button', { name: t('export.export') }));
    const dialog = await screen.findByRole('dialog');

    const format = within(dialog).getByLabelText(t('export.format'));
    expect(format).toHaveValue('csv');
    expect(
      within(format)
        .getAllByRole('option')
        .map((option) => option.textContent),
    ).toEqual(['CSV', 'XML', 'HTML', 'Excel']);

    await user.selectOptions(format, 'xml');
    expect(within(dialog).getByLabelText(t('export.output'))).toHaveValue(
      'C:\\Temp\\report.csv',
    );
    expect(within(dialog).getByRole('checkbox', { name: t('export.section.tree') })).toBeChecked();

    await user.click(within(dialog).getByRole('button', { name: t('rename.preview') }));
    expect(await within(dialog).findByText('C:\\Temp\\report.csv')).toBeInTheDocument();
    expect(scope).toEqual({ generation: 7, out: 'C:\\Temp\\report.csv' });

    await user.click(within(dialog).getByRole('button', { name: t('export.export') }));
    expect(
      await screen.findByText('Отчёт записан: C:\\Temp\\report.xml'),
    ).toBeInTheDocument();
  });

  it('keeps the preview and reports the failure when the export cannot start', async () => {
    const user = userEvent.setup();

    spyTransport((command) => {
      if (command === 'export_start') {
        return Promise.reject(
          Object.assign(new Error('disk full'), uiError('E_IO', 'error.io')),
        );
      }

      return undefined;
    });

    renderWithI18n(
      <ExportAction generation={2} defaultOutput={'C:\\Temp\\report.csv'} />,
    );

    await user.click(screen.getByRole('button', { name: t('export.export') }));
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: t('rename.preview') }));
    await within(dialog).findByText('C:\\Temp\\report.csv');

    await user.click(within(dialog).getByRole('button', { name: t('export.export') }));

    expect(await within(dialog).findByText('Ошибка файловой системы.')).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: t('error.retry') }),
    ).toBeInTheDocument();
    // The preview is not discarded, so the user keeps the chosen scope.
    expect(within(dialog).getByText('C:\\Temp\\report.csv')).toBeInTheDocument();
  });

  it('validates the destination before previewing', async () => {
    const user = userEvent.setup();

    renderWithI18n(
      <ExportAction generation={2} defaultOutput={'C:\\Temp\\report.csv'} />,
    );

    await user.click(screen.getByRole('button', { name: t('export.export') }));
    const dialog = await screen.findByRole('dialog');

    const output = within(dialog).getByLabelText(t('export.output'));
    await user.clear(output);
    await user.click(within(dialog).getByRole('button', { name: t('rename.preview') }));

    expect(await within(dialog).findByText('Проверьте введённые значения.')).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: t('export.export') }),
    ).toBeDisabled();
  });

  it('explains that a scan is required when there is no generation', async () => {
    const user = userEvent.setup();

    renderWithI18n(<ExportAction generation={null} />);

    await user.click(screen.getByRole('button', { name: t('export.export') }));
    const dialog = await screen.findByRole('dialog');

    expect(
      within(dialog).getAllByText('Сначала просканируйте том или каталог.').length,
    ).toBeGreaterThan(0);
    expect(
      within(dialog).queryByRole('button', { name: t('rename.preview') }),
    ).not.toBeInTheDocument();
  });
});
