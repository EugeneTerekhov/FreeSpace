import { useCallback, useRef, useState } from 'react';

/** Contract minimum: progress is announced at most once every two seconds. */
export const DEFAULT_THROTTLE_MS = 2000;
/** Rolling rate limit that keeps rapid status changes from flooding the narrator. */
export const DEFAULT_MAX_PER_WINDOW = 8;
export const DEFAULT_WINDOW_MS = 10_000;

export type AnnouncementUrgency = 'polite' | 'assertive';

export interface AnnounceOptions {
  /** `assertive` interrupts the narrator; reserved for errors that need a decision. */
  urgency?: AnnouncementUrgency;
  /**
   * `progress` announcements are throttled; `status` ones (completion, failure,
   * selection) are not, because they happen once and must not be dropped.
   */
  priority?: 'progress' | 'status';
}

export interface AnnouncerOptions {
  /** Minimum gap between two accepted progress announcements, in ms. */
  throttleMs?: number;
  /** Maximum announcements inside the rolling window. */
  maxPerWindow?: number;
  /** Rolling window length, in ms. */
  windowMs?: number;
  /** Clock injection for deterministic tests. */
  now?: () => number;
}

function defaultNow(): number {
  return Date.now();
}

/**
 * Decides whether a message should reach an `aria-live` region.
 *
 * Rules, in order: empty messages are dropped, an exact repeat of the last
 * accepted message is dropped, progress messages respect the throttle,
 * assertive messages bypass both the throttle and the rate limit, everything
 * else is capped by the rolling window.
 */
export class LiveAnnouncer {
  private readonly throttleMs: number;
  private readonly maxPerWindow: number;
  private readonly windowMs: number;
  private readonly now: () => number;
  private accepted: number[] = [];
  private lastAt = Number.NEGATIVE_INFINITY;
  private lastMessage = '';

  constructor(options: AnnouncerOptions = {}) {
    this.throttleMs = options.throttleMs ?? DEFAULT_THROTTLE_MS;
    this.maxPerWindow = options.maxPerWindow ?? DEFAULT_MAX_PER_WINDOW;
    this.windowMs = options.windowMs ?? DEFAULT_WINDOW_MS;
    this.now = options.now ?? defaultNow;
  }

  /** Returns `true` when the message may be written into a live region. */
  accept(message: string, options: AnnounceOptions = {}): boolean {
    const text = message.trim();
    if (text === '') return false;
    if (text === this.lastMessage) return false;

    const urgency = options.urgency ?? 'polite';
    const priority = options.priority ?? 'status';
    const at = this.now();

    if (priority === 'progress' && at - this.lastAt < this.throttleMs) {
      return false;
    }

    this.prune(at);
    if (urgency !== 'assertive' && this.accepted.length >= this.maxPerWindow) {
      return false;
    }

    this.accepted.push(at);
    this.lastAt = at;
    this.lastMessage = text;

    return true;
  }

  /** Forgets the throttle, rate limit and last message. */
  reset(): void {
    this.accepted = [];
    this.lastAt = Number.NEGATIVE_INFINITY;
    this.lastMessage = '';
  }

  private prune(at: number): void {
    for (;;) {
      const oldest = this.accepted[0];
      if (oldest === undefined || at - oldest < this.windowMs) return;
      this.accepted.shift();
    }
  }
}

export interface LiveAnnouncerHandle {
  /** Text currently mounted in the polite region. */
  polite: string;
  /** Text currently mounted in the assertive region. */
  assertive: string;
  /** Attempts an announcement; returns whether it reached a region. */
  announce: (message: string, options?: AnnounceOptions) => boolean;
  /** Clears the regions and the throttle history. */
  reset: () => void;
}

/**
 * React binding for `LiveAnnouncer`. The returned strings are rendered into
 * one polite and one assertive region by `LiveRegions`.
 */
export function useLiveAnnouncer(
  options: AnnouncerOptions = {},
): LiveAnnouncerHandle {
  const engine = useRef<LiveAnnouncer | null>(null);
  engine.current ??= new LiveAnnouncer(options);

  const [regions, setRegions] = useState({ polite: '', assertive: '' });

  const announce = useCallback(
    (message: string, announceOptions: AnnounceOptions = {}): boolean => {
      const announcer = engine.current;
      if (announcer === null) return false;
      if (!announcer.accept(message, announceOptions)) return false;

      const urgency = announceOptions.urgency ?? 'polite';
      setRegions((current) =>
        urgency === 'assertive'
          ? { ...current, assertive: message }
          : { ...current, polite: message },
      );

      return true;
    },
    [],
  );

  const reset = useCallback(() => {
    engine.current?.reset();
    setRegions({ polite: '', assertive: '' });
  }, []);

  return { ...regions, announce, reset };
}
