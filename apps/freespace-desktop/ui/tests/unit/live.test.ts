import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import {
  DEFAULT_THROTTLE_MS,
  LiveAnnouncer,
  useLiveAnnouncer,
} from '../../src/app/live';

function clock(start = 0) {
  const time = { value: start };

  return { time, now: () => time.value };
}

describe('LiveAnnouncer', () => {
  it('throttles progress announcements to the contract interval', () => {
    const { time, now } = clock();
    const announcer = new LiveAnnouncer({
      throttleMs: DEFAULT_THROTTLE_MS,
      now,
    });

    expect(announcer.accept('Scan 10%', { priority: 'progress' })).toBe(true);
    time.value = 1000;
    expect(announcer.accept('Scan 20%', { priority: 'progress' })).toBe(false);
    time.value = 2000;
    expect(announcer.accept('Scan 20%', { priority: 'progress' })).toBe(true);
  });

  it('deduplicates an unchanged message', () => {
    const { time, now } = clock();
    const announcer = new LiveAnnouncer({ now });

    expect(announcer.accept('Ready')).toBe(true);
    expect(announcer.accept('Ready')).toBe(false);
    time.value = 5000;
    expect(announcer.accept('Ready')).toBe(false);
  });

  it('rejects empty messages', () => {
    const announcer = new LiveAnnouncer();

    expect(announcer.accept('')).toBe(false);
    expect(announcer.accept('   ')).toBe(false);
  });

  it('caps status messages in the rolling window and lets errors through', () => {
    const { time, now } = clock();
    const announcer = new LiveAnnouncer({
      maxPerWindow: 2,
      windowMs: 10_000,
      throttleMs: 0,
      now,
    });

    expect(announcer.accept('one')).toBe(true);
    expect(announcer.accept('two')).toBe(true);
    expect(announcer.accept('three')).toBe(false);
    expect(
      announcer.accept('delete failed', { urgency: 'assertive' }),
    ).toBe(true);

    time.value = 10_001;
    expect(announcer.accept('after window')).toBe(true);
  });

  it('resets the throttle and dedupe history', () => {
    const announcer = new LiveAnnouncer({ throttleMs: 2000 });

    expect(announcer.accept('Scan 10%', { priority: 'progress' })).toBe(true);
    expect(announcer.accept('Scan 10%', { priority: 'progress' })).toBe(false);
    announcer.reset();
    expect(announcer.accept('Scan 10%', { priority: 'progress' })).toBe(true);
  });
});

describe('useLiveAnnouncer', () => {
  it('renders polite and assertive messages into separate regions', () => {
    const { time, now } = clock();
    const { result } = renderHook(() => useLiveAnnouncer({ now }));

    expect(result.current.polite).toBe('');

    act(() => {
      expect(
        result.current.announce('Scan 40%', { priority: 'progress' }),
      ).toBe(true);
    });
    expect(result.current.polite).toBe('Scan 40%');
    expect(result.current.assertive).toBe('');

    time.value = 100;
    act(() => {
      expect(
        result.current.announce('Scan 50%', { priority: 'progress' }),
      ).toBe(false);
    });
    expect(result.current.polite).toBe('Scan 40%');

    act(() => {
      result.current.announce('Delete failed', {
        urgency: 'assertive',
        priority: 'status',
      });
    });
    expect(result.current.assertive).toBe('Delete failed');

    act(() => {
      result.current.reset();
    });
    expect(result.current.polite).toBe('');
    expect(result.current.assertive).toBe('');
  });
});
