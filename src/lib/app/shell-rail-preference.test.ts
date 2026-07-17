import { describe, expect, it } from 'vitest';
import {
  RAIL_COLLAPSED_STORAGE_KEY,
  readRailCollapsed,
  writeRailCollapsed,
} from './shell-rail-preference';

function memoryStorage(seed: Record<string, string> = {}): Storage {
  const map = new Map(Object.entries(seed));
  return {
    get length() {
      return map.size;
    },
    clear() {
      map.clear();
    },
    getItem(key: string) {
      return map.has(key) ? (map.get(key) ?? null) : null;
    },
    key(index: number) {
      return [...map.keys()][index] ?? null;
    },
    removeItem(key: string) {
      map.delete(key);
    },
    setItem(key: string, value: string) {
      map.set(key, value);
    },
  } as Storage;
}

describe('shell-rail-preference', () => {
  it('defaults to expanded when storage missing or invalid', () => {
    expect(readRailCollapsed(null)).toBe(false);
    expect(readRailCollapsed(memoryStorage())).toBe(false);
    expect(readRailCollapsed(memoryStorage({ [RAIL_COLLAPSED_STORAGE_KEY]: '0' }))).toBe(false);
    expect(readRailCollapsed(memoryStorage({ [RAIL_COLLAPSED_STORAGE_KEY]: 'yes' }))).toBe(false);
  });

  it('reads collapsed only when value is 1', () => {
    expect(readRailCollapsed(memoryStorage({ [RAIL_COLLAPSED_STORAGE_KEY]: '1' }))).toBe(true);
  });

  it('persists collapse preference', () => {
    const storage = memoryStorage();
    writeRailCollapsed(storage, true);
    expect(storage.getItem(RAIL_COLLAPSED_STORAGE_KEY)).toBe('1');
    expect(readRailCollapsed(storage)).toBe(true);

    writeRailCollapsed(storage, false);
    expect(storage.getItem(RAIL_COLLAPSED_STORAGE_KEY)).toBe('0');
    expect(readRailCollapsed(storage)).toBe(false);
  });
});
