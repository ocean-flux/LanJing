// Shell session ownership: ambient audio + optional foreground activity override.
// Owned by ModeShell orchestration; survives route/platform noise until explicit clear or pathname change.

import type { AmbientAudioSession, ForegroundActivity } from './shell-types';

export type AmbientAudioSessionValue = NonNullable<AmbientAudioSession>;

let ambientAudio = $state<AmbientAudioSession>(null);
let activityOverride = $state<ForegroundActivity | null>(null);

/** Current ambient audio session (null when none). */
export function getAmbientAudio(): AmbientAudioSession {
  return ambientAudio;
}

/** Replace ambient audio session. Survives pathname/viewport changes until clear. */
export function setAmbientAudio(session: AmbientAudioSessionValue): void {
  ambientAudio = {
    id: session.id,
    state: session.state,
    focus: session.focus,
    label: session.label,
  };
}

/** Clear ambient audio session slot. */
export function clearAmbientAudio(): void {
  ambientAudio = null;
}

/** Explicit foreground activity override (reader/player session keep). */
export function getActivityOverride(): ForegroundActivity | null {
  return activityOverride;
}

/** Set explicit foreground activity; kept across platform-only changes. */
export function setActivityOverride(activity: ForegroundActivity): void {
  activityOverride = {
    kind: activity.kind,
    ...(activity.id !== undefined ? { id: activity.id } : {}),
  };
}

/** Clear explicit foreground activity override. */
export function clearActivityOverride(): void {
  activityOverride = null;
}

/**
 * Pathname changed — single foreground activity must rebind to route default.
 * Ambient audio is intentionally not cleared here.
 */
export function notifyPathnameChanged(): void {
  activityOverride = null;
}

/** Test helper: reset all session fields. */
export function resetShellSession(): void {
  ambientAudio = null;
  activityOverride = null;
}
