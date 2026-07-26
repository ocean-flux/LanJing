import { describe, expect, it } from 'vitest';
import { classifyExpiresAt, truncateHash } from './candidate-preview';

describe('truncateHash', () => {
  it('returns short hashes unchanged', () => {
    expect(truncateHash('abcdef')).toBe('abcdef');
  });

  it('truncates long hashes as head…tail', () => {
    expect(truncateHash('0123456789abcdef0123')).toBe('01234567…0123');
  });
});

describe('classifyExpiresAt', () => {
  const now = 1_700_000_000_000;

  it('marks past timestamps as expired', () => {
    expect(classifyExpiresAt(now - 1, now)).toEqual({ kind: 'expired' });
  });

  it('uses minutes under one hour', () => {
    expect(classifyExpiresAt(now + 25 * 60_000, now)).toEqual({ kind: 'minutes', minutes: 25 });
  });

  it('uses hours under 48h', () => {
    expect(classifyExpiresAt(now + 5 * 3_600_000, now)).toEqual({ kind: 'hours', hours: 5 });
  });

  it('uses absolute text for longer windows', () => {
    const hint = classifyExpiresAt(now + 72 * 3_600_000, now);
    expect(hint.kind).toBe('absolute');
  });
});
