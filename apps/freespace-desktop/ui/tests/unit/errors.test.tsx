import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ErrorBoundary } from '../../src/app/ErrorBoundary';
import { isUiError, normalizeError, validationError } from '../../src/app/errors';

function Boom(): never {
  throw new Error('boom');
}

describe('error handling', () => {
  it('keeps a UiError envelope unchanged', () => {
    const error = validationError('bad path');
    expect(isUiError(error)).toBe(true);
    expect(normalizeError(error)).toEqual(error);
  });

  it('normalises unknown failures into E_INTERNAL', () => {
    const error = normalizeError(new Error('disk gone'));
    expect(error.code).toBe('E_INTERNAL');
    expect(error.category).toBe('internal');
    expect(error.retryable).toBe(true);
    expect(error.details).toBe('disk gone');
  });

  it('renders the fallback screen and logs locally', () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );

    expect(screen.getByRole('alert')).toHaveTextContent('Something went wrong');
    expect(screen.getByText('E_INTERNAL')).toBeInTheDocument();
    consoleError.mockRestore();
  });
});
