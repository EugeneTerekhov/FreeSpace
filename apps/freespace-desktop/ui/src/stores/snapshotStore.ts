import { create } from 'zustand';

import type { VolumeSummary } from '../api/dto';

interface SnapshotState {
  generation: number | null;
  /** Primary volume of the snapshot, kept for single-volume callers. */
  volume: VolumeSummary | null;
  /** Every volume of the snapshot, in root order. */
  volumes: VolumeSummary[];
  /**
   * Paths removed from the snapshot by a successful operation. The scan tree is
   * immutable, so a deleted object would otherwise reappear after leaving and
   * re-entering the tree; a new generation clears the list.
   */
  hiddenPaths: ReadonlySet<string>;
  setGeneration: (generation: number) => void;
  setVolume: (volume: VolumeSummary | null) => void;
  setVolumes: (volumes: VolumeSummary[]) => void;
  setSnapshot: (generation: number, volume: VolumeSummary | null) => void;
  hidePath: (path: string) => void;
  reset: () => void;
}

const NO_HIDDEN_PATHS: ReadonlySet<string> = new Set();
const NO_VOLUMES: VolumeSummary[] = [];

/** Generation and volumes of the latest finished scan. */
export const useSnapshotStore = create<SnapshotState>()((set) => ({
  generation: null,
  volume: null,
  volumes: NO_VOLUMES,
  hiddenPaths: NO_HIDDEN_PATHS,

  setGeneration(generation) {
    set((state) =>
      state.generation === generation
        ? state
        : {
            generation,
            volume: null,
            volumes: NO_VOLUMES,
            hiddenPaths: NO_HIDDEN_PATHS,
          },
    );
  },

  setVolume(volume) {
    set({ volume, volumes: volume === null ? NO_VOLUMES : [volume] });
  },

  setVolumes(volumes) {
    set({ volumes, volume: volumes[0] ?? null });
  },

  setSnapshot(generation, volume) {
    set({
      generation,
      volume,
      volumes: volume === null ? NO_VOLUMES : [volume],
      hiddenPaths: NO_HIDDEN_PATHS,
    });
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
    set({
      generation: null,
      volume: null,
      volumes: NO_VOLUMES,
      hiddenPaths: NO_HIDDEN_PATHS,
    });
  },
}));
