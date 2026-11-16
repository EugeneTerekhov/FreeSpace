import { render, type RenderOptions } from '@testing-library/react';
import type { ReactElement } from 'react';

import { setLanguage } from '../../src/i18n';

/** Renders a component with the default test language reset. */
export function renderWithI18n(
  ui: ReactElement,
  options?: RenderOptions,
) {
  setLanguage('ru');

  return render(ui, options);
}
