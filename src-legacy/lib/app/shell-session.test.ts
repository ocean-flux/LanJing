import { afterEach, describe, expect, it } from 'vitest';
import {
  clearActivityOverride,
  clearAmbientAudio,
  getActivityOverride,
  getAmbientAudio,
  notifyPathnameChanged,
  resetShellSession,
  setActivityOverride,
  setAmbientAudio,
} from './shell-session.svelte';

afterEach(() => {
  resetShellSession();
});

describe('shell-session', () => {
  it('sets and keeps ambient audio identity across repeated reads', () => {
    setAmbientAudio({
      id: 'ambient-1',
      state: 'playing',
      focus: 'ambient',
      label: '夜航',
    });

    const first = getAmbientAudio();
    const second = getAmbientAudio();

    expect(first).toEqual({
      id: 'ambient-1',
      state: 'playing',
      focus: 'ambient',
      label: '夜航',
    });
    expect(second).toEqual(first);
    expect(second?.id).toBe('ambient-1');
    expect(second?.label).toBe('夜航');
  });

  it('clears ambient audio without touching activity override', () => {
    setAmbientAudio({
      id: 'ambient-1',
      state: 'paused',
      focus: 'none',
      label: '夜航',
    });
    setActivityOverride({ kind: 'reader', id: 'chapter-7' });

    clearAmbientAudio();

    expect(getAmbientAudio()).toBeNull();
    expect(getActivityOverride()).toEqual({ kind: 'reader', id: 'chapter-7' });
  });

  it('clears activity override on pathname change but keeps ambient audio', () => {
    setAmbientAudio({
      id: 'ambient-1',
      state: 'playing',
      focus: 'ambient',
      label: '夜航',
    });
    setActivityOverride({ kind: 'player', id: 'track-9' });

    notifyPathnameChanged();

    expect(getActivityOverride()).toBeNull();
    expect(getAmbientAudio()).toEqual({
      id: 'ambient-1',
      state: 'playing',
      focus: 'ambient',
      label: '夜航',
    });
  });

  it('clearActivityOverride only drops activity override', () => {
    setAmbientAudio({
      id: 'ambient-1',
      state: 'paused',
      focus: 'none',
      label: '夜航',
    });
    setActivityOverride({ kind: 'reader' });

    clearActivityOverride();

    expect(getActivityOverride()).toBeNull();
    expect(getAmbientAudio()?.id).toBe('ambient-1');
  });
});
