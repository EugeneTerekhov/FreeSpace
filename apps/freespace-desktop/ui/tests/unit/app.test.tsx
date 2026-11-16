import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { App } from '../../src/App';

describe('App', () => {
  it('renders the shell heading', async () => {
    render(<App />);

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Диски' }),
    ).toBeInTheDocument();
  });
});
