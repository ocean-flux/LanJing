/** 冷启动启动视觉门槛：够快或本会话已展示则跳过。 */

/** 冷启动已等待至少该毫秒数时才展示 AppLaunch。 */
export const COLD_LAUNCH_THRESHOLD_MS = 280;

/** sessionStorage 键：本标签页会话内已展示（或完成）启动视觉。 */
export const COLD_LAUNCH_SESSION_KEY = 'lanjing.cold-launch.shown';

export type ColdLaunchDecisionInput = {
  /** 自导航起经过的毫秒数（AppShell 初始化时的 `performance.now()`）。 */
  now: number;
  /** 本会话是否已记录过启动视觉展示。 */
  sessionShown: boolean;
  /** 覆盖阈值（测试用）。 */
  thresholdMs?: number;
};

/**
 * 是否应展示冷启动品牌启动层。
 * 纯函数、无 I/O。热路径 / 快冷启动 / 同会话已展示 → false。
 */
export function shouldShowColdLaunch({
  now,
  sessionShown,
  thresholdMs = COLD_LAUNCH_THRESHOLD_MS,
}: ColdLaunchDecisionInput): boolean {
  if (sessionShown) return false;
  return now >= thresholdMs;
}

/** 读取会话标志；无 storage 或读写失败 → 视为未展示。 */
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

/** 写入会话标志，避免 SPA 重挂载或后续导航再次播放启动视觉。 */
export function markColdLaunchSessionShown(
  storage: Pick<Storage, 'setItem'> | null | undefined,
  key: string = COLD_LAUNCH_SESSION_KEY,
): void {
  if (!storage) return;
  try {
    storage.setItem(key, '1');
  } catch {
    // 隐私模式 / 配额：忽略；最坏情况启动视觉可能再播一次
  }
}
