import { create } from 'zustand';

import type { MonitorEvent, MonitorStatus, UiError } from '../api/dto';

/** Default form values shown when the monitor has never been configured. */
export const MONITOR_DEFAULT_THRESHOLD = String(1024 * 1024 * 1024);
export const MONITOR_DEFAULT_INTERVAL = '2';
export const MONITOR_MAX_INTERVAL = 86_400;

const NO_EVENTS: MonitorEvent[] = [];

/**
 * State of the live write monitor.
 *
 * The monitor session lives here instead of in the `Monitor` component: the
 * router unmounts a screen as soon as another one is selected, so component
 * local state would lose the task id, counters and the event history on every
 * navigation (US20, contract §4.2). The native service keeps the session
 * running; returning to the screen simply reattaches to the stored task.
 */
interface MonitorState {
  /** Threshold input in bytes, kept as text for the number field. */
  threshold: string;
  /** Poll interval input in seconds, kept as text for the number field. */
  interval: string;
  /** Exclusion masks as entered by the user. */
  exclusions: string;

  processFilter: string;
  pathFilter: string;
  thresholdOnly: boolean;

  taskId: string | null;
  running: boolean;
  status: MonitorStatus | null;
  events: MonitorEvent[];
  /** Highest event sequence already announced into the live region. */
  lastSequence: number;

  loading: boolean;
  validation: string | null;
  error: UiError | null;
  notice: string | null;

  setThreshold: (value: string) => void;
  setInterval: (value: string) => void;
  setExclusions: (value: string) => void;
  setProcessFilter: (value: string) => void;
  setPathFilter: (value: string) => void;
  setThresholdOnly: (value: boolean) => void;
  setLoading: (value: boolean) => void;
  setValidation: (value: string | null) => void;
  setError: (value: UiError | null) => void;
  setNotice: (value: string | null) => void;

  /** Records a freshly started session and clears the previous history. */
  beginSession: (taskId: string) => void;
  setRunning: (value: boolean) => void;
  setStatus: (value: MonitorStatus | null) => void;
  setEvents: (value: MonitorEvent[]) => void;
  /** Adds tick events without dropping events fetched by the cursor page. */
  mergeEvents: (incoming: MonitorEvent[]) => void;
  /** Events newer than the announced cursor; advances the cursor. */
  takeFresh: (incoming: MonitorEvent[]) => MonitorEvent[];
  /** Full reset, used by tests. */
  reset: () => void;
}

const initialState = () => ({
  threshold: MONITOR_DEFAULT_THRESHOLD,
  interval: MONITOR_DEFAULT_INTERVAL,
  exclusions: '',
  processFilter: '',
  pathFilter: '',
  thresholdOnly: false,
  taskId: null,
  running: false,
  status: null,
  events: NO_EVENTS,
  lastSequence: 0,
  loading: false,
  validation: null,
  error: null,
  notice: null,
});

/** Write-monitor session and form state shared by every screen mount. */
export const useMonitorStore = create<MonitorState>()((set, get) => ({
  ...initialState(),

  setThreshold: (threshold) => set({ threshold }),
  setInterval: (interval) => set({ interval }),
  setExclusions: (exclusions) => set({ exclusions }),
  setProcessFilter: (processFilter) => set({ processFilter }),
  setPathFilter: (pathFilter) => set({ pathFilter }),
  setThresholdOnly: (thresholdOnly) => set({ thresholdOnly }),
  setLoading: (loading) => set({ loading }),
  setValidation: (validation) => set({ validation }),
  setError: (error) => set({ error }),
  setNotice: (notice) => set({ notice }),

  beginSession: (taskId) =>
    set({
      taskId,
      running: true,
      status: null,
      events: NO_EVENTS,
      lastSequence: 0,
      validation: null,
      error: null,
      notice: null,
    }),
  setRunning: (running) => set({ running }),
  setStatus: (status) => set({ status }),
  setEvents: (events) => set({ events }),

  mergeEvents: (incoming) =>
    set((state) => {
      if (incoming.length === 0) return state;
      const bySequence = new Map(state.events.map((event) => [event.sequence, event]));
      for (const event of incoming) {
        bySequence.set(event.sequence, event);
      }

      return {
        events: [...bySequence.values()].sort((left, right) => right.sequence - left.sequence),
      };
    }),

  takeFresh: (incoming) => {
    if (incoming.length === 0) return NO_EVENTS;
    const last = get().lastSequence;
    const fresh = incoming.filter((event) => event.sequence > last);
    const highest = incoming.reduce((max, event) => Math.max(max, event.sequence), last);
    if (highest !== last) set({ lastSequence: highest });

    return fresh;
  },

  reset: () => set(initialState()),
}));
