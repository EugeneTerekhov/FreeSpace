import { create } from 'zustand';

import type { ApiClient } from '../api/client';
import type { AppBootstrap, PlatformCapabilities, ScreenId, UiError } from '../api/dto';
import { normalizeError, logLocal } from '../app/errors';
import { resolveInitialScreen } from '../app/routing';
import { applySettingsToDocument } from './settingsStore';

interface ShellState {
  ready: boolean;
  bootstrap: AppBootstrap | null;
  error: UiError | null;
  screen: ScreenId;
  sidebarCollapsed: boolean;
  pendingNodeId: number | null;
  load: (api: ApiClient) => Promise<void>;
  setScreen: (screen: ScreenId) => void;
  setPendingNode: (nodeId: number | null) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;
}

/** Shell state: bootstrap, active screen and sidebar. */
export const useShellStore = create<ShellState>()((set, get) => ({
  ready: false,
  bootstrap: null,
  error: null,
  screen: 'overview',
  sidebarCollapsed: false,
  pendingNodeId: null,

  async load(api) {
    try {
      const bootstrap = await api.appBootstrap();
      applySettingsToDocument(bootstrap.settings);
      const screen = resolveInitialScreen();
      set({
        ready: true,
        bootstrap,
        error: null,
        screen,
        sidebarCollapsed: bootstrap.settings.sidebar_collapsed,
      });
    } catch (error) {
      const normalized = normalizeError(error);
      logLocal('error', 'bootstrap failed', normalized);
      set({ ready: true, error: normalized });
    }
  },

  setScreen(screen) {
    set({ screen });
  },

  setPendingNode(nodeId) {
    set({ pendingNodeId: nodeId });
  },

  setSidebarCollapsed(collapsed) {
    set({ sidebarCollapsed: collapsed });
  },

  toggleSidebar() {
    set({ sidebarCollapsed: !get().sidebarCollapsed });
  },
}));

/**
 * Platform capabilities advertised by the running backend.
 *
 * `null` until the bootstrap payload arrives; callers fall back to the full
 * capability set in that window (see `lib/capabilities.ts`).
 */
export function usePlatformCapabilities(): PlatformCapabilities | null {
  return useShellStore(
    (state) => state.bootstrap?.capabilities.platform_capabilities ?? null,
  );
}
