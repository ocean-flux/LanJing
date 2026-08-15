// 壳层会话归属：环境音频 + 可选前台活动覆盖。
// 由 ModeShell 编排持有；跨路由/平台抖动存活，直至显式清空或 pathname 变化。

import type { AmbientAudioSession, ForegroundActivity } from './shell-types';

export type AmbientAudioSessionValue = NonNullable<AmbientAudioSession>;

let ambientAudio = $state<AmbientAudioSession>(null);
let activityOverride = $state<ForegroundActivity | null>(null);

/** 当前环境音频会话；无会话时为 null。 */
export function getAmbientAudio(): AmbientAudioSession {
  return ambientAudio;
}

/** 替换环境音频会话；跨 pathname/视口变化保留，直至 clear。 */
export function setAmbientAudio(session: AmbientAudioSessionValue): void {
  ambientAudio = {
    id: session.id,
    state: session.state,
    focus: session.focus,
    label: session.label,
  };
}

/** 清空环境音频会话槽位。 */
export function clearAmbientAudio(): void {
  ambientAudio = null;
}

/** 显式前台活动覆盖（阅读器/播放器会话保活）。 */
export function getActivityOverride(): ForegroundActivity | null {
  return activityOverride;
}

/** 设置显式前台活动；跨纯平台变化保留。 */
export function setActivityOverride(activity: ForegroundActivity): void {
  activityOverride = {
    kind: activity.kind,
    ...(activity.id !== undefined ? { id: activity.id } : {}),
  };
}

/** 清除显式前台活动覆盖。 */
export function clearActivityOverride(): void {
  activityOverride = null;
}

/**
 * pathname 已变——单一前台活动须回落到路由默认。
 * 环境音频刻意不在此清空。
 */
export function notifyPathnameChanged(): void {
  activityOverride = null;
}

/** 测试辅助：重置全部会话字段。 */
export function resetShellSession(): void {
  ambientAudio = null;
  activityOverride = null;
}
