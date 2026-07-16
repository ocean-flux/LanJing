/** Cold-start launch visual gate: skip when fast enough or already shown this session. */

/** Show AppLaunch only when cold start has already waited at least this long. */
export const COLD_LAUNCH_THRESHOLD_MS = 280;

/** sessionStorage key — set once launch is shown (or completed) in this tab session. */
export const COLD_LAUNCH_SESSION_KEY = 'lanjing.cold-launch.shown';

export type ColdLaunchDecisionInput = {
  /** Elapsed ms since navigation start (`performance.now()` at AppShell init). */
  now: number;
  /** True when this tab session already recorded a launch show. */
  sessionShown: boolean;
  /** Override threshold (tests). */
  thresholdMs?: number;
};

/**
 * Decide whether cold-start brand launch overlay should appear.
 * Pure: no I/O. Hot path / fast cold start / same-session → false.
 */
export function shouldShowColdLaunch({
  now,
  sessionShown,
  thresholdMs = COLD_LAUNCH_THRESHOLD_MS,
}: ColdLaunchDecisionInput): boolean {
  if (sessionShown) return false;
  return now >= thresholdMs;
}

/** Read session flag; missing / blocked storage → not shown. */
export function readColdLaunchSessionShown(
  storage: Pick<Storage, 'getItem'> | null | undefined,
  key: string = COLD_LAUNCH_SESSION_KEY,
): boolean {
  if (!storage) return false;
  try {
    return storage.getItem(key) === '1';
  } catch {
    return false;
  }
}

/** Persist session flag so SPA remounts / later navigations skip launch. */
export function markColdLaunchSessionShown(
  storage: Pick<Storage, 'setItem'> | null | undefined,
  key: string = COLD_LAUNCH_SESSION_KEY,
): void {
  if (!storage) return;
  try {
    storage.setItem(key, '1');
  } catch {
    // private mode / quota — ignore; worst case launch may replay once
  }
}
