/** Candidate preview pure helpers — hash truncation + expires delta. */

/** Truncate a hash for denselist display: first head…last tail. */
export function truncateHash(hash: string, head = 8, tail = 4): string {
  const value = hash.trim();
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

export type ExpiresHint =
  | { kind: 'expired' }
  | { kind: 'minutes'; minutes: number }
  | { kind: 'hours'; hours: number }
  | { kind: 'absolute'; isoDate: string };

/** Classify candidate expiry for short readable UI text. */
export function classifyExpiresAt(expiresAtMs: number, nowMs = Date.now()): ExpiresHint {
  if (!Number.isFinite(expiresAtMs)) {
    return { kind: 'absolute', isoDate: String(expiresAtMs) };
  }

  const remaining = expiresAtMs - nowMs;
  if (remaining <= 0) return { kind: 'expired' };

  const minutes = Math.ceil(remaining / 60_000);
  if (minutes < 60) return { kind: 'minutes', minutes: Math.max(1, minutes) };

  const hours = Math.ceil(remaining / 3_600_000);
  if (hours < 48) return { kind: 'hours', hours };

  try {
    return {
      kind: 'absolute',
      isoDate: new Date(expiresAtMs).toISOString().slice(0, 16).replace('T', ' '),
    };
  } catch {
    return { kind: 'absolute', isoDate: String(expiresAtMs) };
  }
}
