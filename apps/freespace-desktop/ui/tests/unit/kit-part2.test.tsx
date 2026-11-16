import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { Card, StatCard, SummaryCard } from '../../src/components/Card';
import { ConfirmDialog, Dialog } from '../../src/components/Dialog';
import {
  Banner,
  EmptyState,
  ErrorState,
  Loading,
  Meter,
  Progress,
  Skeleton,
  Toast,
  ToastRegion,
} from '../../src/components/Feedback';

describe('UI kit, part 2', () => {
  it('exposes progress and meter semantics', () => {
    render(
      <>
        <Progress value={42} label="Scanning" />
        <Meter label="Disk usage" value={91} />
      </>,
    );

    expect(screen.getByRole('progressbar', { name: 'Scanning' })).toHaveAttribute(
      'aria-valuenow',
      '42',
    );
    expect(screen.getByRole('meter', { name: 'Disk usage' })).toHaveAttribute(
      'aria-valuenow',
      '91',
    );
  });

  it('renders cards without a heading when no title is provided', () => {
    render(
      <>
        <Card title="Volumes">Body</Card>
        <StatCard label="Free" value="120 GiB" trend="+2%" />
        <SummaryCard title="Summary" selected>
          Content
        </SummaryCard>
      </>,
    );

    expect(screen.getByRole('heading', { name: 'Volumes' })).toBeInTheDocument();
    expect(screen.getByText('120 GiB')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Summary' })).toBeInTheDocument();
  });

  it('closes a dialog with Escape and traps the initial focus', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Dialog open title="Confirm" onClose={onClose} footer={<button type="button">OK</button>}>
        Body
      </Dialog>,
    );

    expect(screen.getByRole('dialog', { name: 'Confirm' })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
  });

  it('runs the confirm action from the dialog', async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(
      <ConfirmDialog
        open
        title="Delete files"
        description="The files will be moved to the Recycle Bin."
        confirmLabel="Delete"
        cancelLabel="Cancel"
        onConfirm={onConfirm}
        onCancel={() => undefined}
        destructive
      />,
    );

    await user.click(screen.getByRole('button', { name: 'Delete' }));
    expect(onConfirm).toHaveBeenCalled();
  });

  it('announces toasts, banners and empty states', () => {
    render(
      <>
        <ToastRegion>
          <Toast severity="error" message="Scan failed" />
        </ToastRegion>
        <Banner tone="error">Disk offline</Banner>
        <EmptyState title="No data" description="Scan a volume" />
        <ErrorState title="Failed" code="E_IO" details="disk gone" />
      </>,
    );

    expect(screen.getByText('Scan failed')).toBeInTheDocument();
    expect(screen.getByText('Disk offline')).toBeInTheDocument();
    expect(screen.getByText('No data')).toBeInTheDocument();
    expect(screen.getByText('E_IO')).toBeInTheDocument();
    expect(screen.getByText('disk gone')).toBeInTheDocument();
  });

  it('marks loading regions and hides skeletons from assistive tech', () => {
    render(
      <>
        <Loading label="Loading volumes" rows={2} />
        <Skeleton />
      </>,
    );

    expect(screen.getByLabelText('Loading volumes')).toHaveAttribute(
      'aria-busy',
      'true',
    );
    expect(screen.getByLabelText('Loading volumes')).toBeInTheDocument();
  });
});
