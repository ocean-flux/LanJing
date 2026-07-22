import { describe, expect, it } from 'vitest';
import {
  RAIL_BEHAVIOR_STORAGE_KEY,
  RAIL_COLLAPSED_STORAGE_KEY,
  readRailBehavior,
  readRailCollapsed,
  writeRailBehavior,
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

  it('defaults rail behavior to fixed', () => {
    expect(readRailBehavior(null)).toBe('fixed');
    expect(readRailBehavior(memoryStorage())).toBe('fixed');
    expect(readRailBehavior(memoryStorage({ [RAIL_BEHAVIOR_STORAGE_KEY]: 'nope' }))).toBe('fixed');
  });

  it('persists hover rail behavior', () => {
    const storage = memoryStorage();
    writeRailBehavior(storage, 'hover');
    expect(storage.getItem(RAIL_BEHAVIOR_STORAGE_KEY)).toBe('hover');
    expect(readRailBehavior(storage)).toBe('hover');
    writeRailBehavior(storage, 'fixed');
    expect(readRailBehavior(storage)).toBe('fixed');
  });
});
