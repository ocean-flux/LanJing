import { describe, expect, it, vi } from 'vitest';
import {
  COLD_LAUNCH_SESSION_KEY,
  COLD_LAUNCH_THRESHOLD_MS,
  markColdLaunchSessionShown,
  readColdLaunchSessionShown,
  shouldShowColdLaunch,
} from './cold-launch';

describe('shouldShowColdLaunch', () => {
  it('exports 280ms cold-start threshold', () => {
    expect(COLD_LAUNCH_THRESHOLD_MS).toBe(280);
  });

  it('skips when session already showed launch', () => {
    expect(
      shouldShowColdLaunch({
        now: 5_000,
        sessionShown: true,
      }),
    ).toBe(false);
  });

  it('skips when cold start is faster than threshold', () => {
    expect(
      shouldShowColdLaunch({
        now: COLD_LAUNCH_THRESHOLD_MS - 1,
        sessionShown: false,
      }),
    ).toBe(false);
  });

  it('shows when cold start meets or exceeds threshold and session is clean', () => {
    expect(
      shouldShowColdLaunch({
        now: COLD_LAUNCH_THRESHOLD_MS,
        sessionShown: false,
      }),
    ).toBe(true);
    expect(
      shouldShowColdLaunch({
        now: COLD_LAUNCH_THRESHOLD_MS + 120,
        sessionShown: false,
      }),
    ).toBe(true);
  });

  it('honors custom threshold override', () => {
    expect(shouldShowColdLaunch({ now: 100, sessionShown: false, thresholdMs: 50 })).toBe(true);
    expect(shouldShowColdLaunch({ now: 100, sessionShown: false, thresholdMs: 200 })).toBe(false);
  });
});

describe('cold-launch session storage helpers', () => {
  it('reads and marks session flag', () => {
    const store = new Map<string, string>();
    const storage = {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
    };

    expect(readColdLaunchSessionShown(storage)).toBe(false);
    markColdLaunchSessionShown(storage);
    expect(store.get(COLD_LAUNCH_SESSION_KEY)).toBe('1');
    expect(readColdLaunchSessionShown(storage)).toBe(true);
  });

  it('treats null storage and storage errors as not shown / no-op', () => {
    expect(readColdLaunchSessionShown(null)).toBe(false);
    expect(readColdLaunchSessionShown(undefined)).toBe(false);

    const brokenGet = {
      getItem: () => {
        throw new Error('blocked');
      },
    };
    expect(readColdLaunchSessionShown(brokenGet)).toBe(false);

    const brokenSet = {
      setItem: () => {
        throw new Error('quota');
      },
    };
    expect(() => markColdLaunchSessionShown(brokenSet)).not.toThrow();
    markColdLaunchSessionShown(null);
  });

  it('ignores unrelated session values', () => {
    const storage = {
      getItem: vi.fn(() => 'true'),
    };
    expect(readColdLaunchSessionShown(storage)).toBe(false);
  });
});
