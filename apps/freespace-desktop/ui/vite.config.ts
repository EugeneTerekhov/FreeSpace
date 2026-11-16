import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The Tauri shell loads the built assets from `dist`; the dev server is bound to
// localhost only and uses the fixed port expected by `tauri.conf.json`.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**', '**/dist/**'],
    },
  },
  build: {
    target: 'chrome105',
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
  },
});
