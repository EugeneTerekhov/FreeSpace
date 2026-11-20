import { create } from 'zustand';

import type { ApiClient } from '../api/client';
import type { CleanupCandidateView, FilterSet, UiError } from '../api/dto';
import { normalizeError } from '../app/errors';
import { emptyFilter } from '../lib/filters';

/**
 * State of the recommendations tab that outlives the screen component.
 *
 * Switching screens unmounts `Cleanup`, so keeping the result in component
 * state would refetch every recommendation on every visit. The UI contract
 * (§4.2) requires a return to a screen to reuse data while `generation` is
 * unchanged; the cache below also keeps the query, the selection and the
 * position of the virtualized table.
 */
interface CleanupState {
  /** Generation the cached result belongs to; `null` before the first load. */
  generation: number | null;
  /** Parameters of the last (or pending) analysis. */
  olderThan: number;
  filter: FilterSet;
  /** Filter the cached result was loaded with; avoids a reload on remount. */
  appliedFilter: FilterSet | null;
  candidates: CleanupCandidateView[];
  selected: ReadonlySet<number>;
  /** Scroll offset of the virtualized recommendations table. */
  scrollTop: number;
  /** A successful response has been received for `generation`. */
  loaded: boolean;
  loading: boolean;
  error: UiError | null;
  setOlderThan: (value: number) => void;
  setFilter: (filter: FilterSet) => void;
  setScrollTop: (value: number) => void;
  load: (api: ApiClient, generation: number, filterOverride?: FilterSet) => Promise<void>;
  toggle: (id: number, checked: boolean) => void;
  selectIds: (ids: readonly number[], checked: boolean) => void;
  clearSelection: () => void;
  /** Drops cached data, optionally binding the empty cache to a new generation. */
  reset: (generation?: number | null) => void;
}

const NO_SELECTION: ReadonlySet<number> = new Set();

export const useCleanupStore = create<CleanupState>()((set, get) => ({
  generation: null,
  olderThan: 180,
  filter: emptyFilter(),
  appliedFilter: null,
  candidates: [],
  selected: NO_SELECTION,
  scrollTop: 0,
  loaded: false,
  loading: false,
  error: null,

  setOlderThan(value) {
    set({ olderThan: value });
  },

  setFilter(filter) {
    set({ filter });
  },

  setScrollTop(value) {
    set({ scrollTop: value });
  },

  async load(api, generation, filterOverride) {
    const { olderThan, filter } = get();
    const activeFilter = filterOverride ?? filter;
    set({ generation, appliedFilter: activeFilter, loading: true, error: null });
    try {
      const candidates = await api.cleanupCandidates(generation, olderThan, activeFilter);
      set({
        candidates,
        selected: NO_SELECTION,
        scrollTop: 0,
        loaded: true,
        loading: false,
      });
    } catch (cause) {
      set({ loading: false, error: normalizeError(cause) });
    }
  },

  toggle(id, checked) {
    set((state) => {
      const next = new Set(state.selected);
      if (checked) next.add(id);
      else next.delete(id);
      return { selected: next };
    });
  },

  selectIds(ids, checked) {
    set((state) => {
      const next = new Set(state.selected);
      for (const id of ids) {
        if (checked) next.add(id);
        else next.delete(id);
      }
      return { selected: next };
    });
  },

  clearSelection() {
    set({ selected: NO_SELECTION });
  },

  reset(generation = null) {
    set({
      generation,
      olderThan: 180,
      filter: emptyFilter(),
      appliedFilter: null,
      candidates: [],
      selected: NO_SELECTION,
      scrollTop: 0,
      loaded: false,
      loading: false,
      error: null,
    });
  },
}));
