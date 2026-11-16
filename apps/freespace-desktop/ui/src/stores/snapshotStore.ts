import { create } from 'zustand';

import type { VolumeSummary } from '../api/dto';

interface SnapshotState {
  generation: number | null;
  volume: VolumeSummary | null;
  setGeneration: (generation: number) => void;
  setVolume: (volume: VolumeSummary | null) => void;
  setSnapshot: (generation: number, volume: VolumeSummary | null) => void;
  reset: () => void;
}

/** Generation and volume of the latest finished scan. */
export const useSnapshotStore = create<SnapshotState>()((set) => ({
  generation: null,
  volume: null,

  setGeneration(generation) {
    set({ generation });
  },

  setVolume(volume) {
    set({ volume });
  },

  setSnapshot(generation, volume) {
    set({ generation, volume });
  },

  reset() {
    set({ generation: null, volume: null });
  },
}));
