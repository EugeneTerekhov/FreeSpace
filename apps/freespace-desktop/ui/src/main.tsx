import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import './styles/global.css';

const container = document.getElementById('root');

if (container === null) {
  throw new Error('the #root container is missing from index.html');
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

// The splash only covers the gap between the first paint and React's first
// commit; keeping it in the DOM would make the shell look stuck on a slow boot.
document.getElementById('splash')?.remove();
