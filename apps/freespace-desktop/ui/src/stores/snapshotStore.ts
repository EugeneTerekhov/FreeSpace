import { create } from 'zustand';

import type { VolumeSummary } from '../api/dto';

interface SnapshotState {
  generation: number | null;
  volume: VolumeSummary | null;
  /**
   * Paths removed from the snapshot by a successful operation. The scan tree is
   * immutable, so a deleted object would otherwise reappear after leaving and
   * re-entering the tree; a new generation clears the list.
   */
  hiddenPaths: ReadonlySet<string>;
  setGeneration: (generation: number) => void;
  setVolume: (volume: VolumeSummary | null) => void;
  setSnapshot: (generation: number, volume: VolumeSummary | null) => void;
  hidePath: (path: string) => void;
  reset: () => void;
}

const NO_HIDDEN_PATHS: ReadonlySet<string> = new Set();

/** Generation and volume of the latest finished scan. */
export const useSnapshotStore = create<SnapshotState>()((set) => ({
  generation: null,
  volume: null,
  hiddenPaths: NO_HIDDEN_PATHS,

  setGeneration(generation) {
    set((state) =>
      state.generation === generation
        ? state
        : { generation, hiddenPaths: NO_HIDDEN_PATHS },
    );
  },

  setVolume(volume) {
    set({ volume });
  },

  setSnapshot(generation, volume) {
    set({ generation, volume, hiddenPaths: NO_HIDDEN_PATHS });
  },

  hidePath(path) {
    const normalized = path.toLowerCase();
    set((state) => {
      if (state.hiddenPaths.has(normalized)) return state;
      const next = new Set(state.hiddenPaths);
      next.add(normalized);

      return { hiddenPaths: next };
    });
  },

  reset() {
    set({ generation: null, volume: null, hiddenPaths: NO_HIDDEN_PATHS });
  },
}));
