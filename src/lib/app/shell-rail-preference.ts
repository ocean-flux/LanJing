/**
 * 桌面脊偏好：折叠态 + 固定/悬停模式。
 * pure 读写，供 AppShell 与测试 mock。
 */

export const RAIL_COLLAPSED_STORAGE_KEY = 'lanjing.shell.rail-collapsed';
export const RAIL_BEHAVIOR_STORAGE_KEY = 'lanjing.shell.rail-behavior';

/** 固定：手动展开/收起；悬停：指针停留后展开、离开后收起 */
export type RailBehavior = 'fixed' | 'hover';

/** 悬停展开延迟（毫秒），避免秒出 */
export const RAIL_HOVER_OPEN_MS = 320;
/** 悬停收起延迟（毫秒） */
export const RAIL_HOVER_CLOSE_MS = 240;

/** 读取脊是否折叠；缺省或非法值为展开。仅 fixed 模式使用。 */
export function readRailCollapsed(storage: Pick<Storage, 'getItem'> | null | undefined): boolean {
  if (!storage) return false;
  try {
    return storage.getItem(RAIL_COLLAPSED_STORAGE_KEY) === '1';
  } catch {
    return false;
  }
}

/** 持久化脊折叠态。 */
export function writeRailCollapsed(
  storage: Pick<Storage, 'setItem'> | null | undefined,
  collapsed: boolean,
): void {
  if (!storage) return;
  try {
    storage.setItem(RAIL_COLLAPSED_STORAGE_KEY, collapsed ? '1' : '0');
  } catch {
    /* 隐私模式等写失败忽略 */
  }
}

/** 读取侧栏行为；缺省 fixed。 */
export function readRailBehavior(
  storage: Pick<Storage, 'getItem'> | null | undefined,
): RailBehavior {
  if (!storage) return 'fixed';
  try {
    const raw = storage.getItem(RAIL_BEHAVIOR_STORAGE_KEY);
    return raw === 'hover' ? 'hover' : 'fixed';
  } catch {
    return 'fixed';
  }
}

/** 持久化侧栏行为。 */
export function writeRailBehavior(
  storage: Pick<Storage, 'setItem'> | null | undefined,
  behavior: RailBehavior,
): void {
  if (!storage) return;
  try {
    storage.setItem(RAIL_BEHAVIOR_STORAGE_KEY, behavior);
  } catch {
    /* ignore */
  }
}
