/**
 * 桌面窄脊折叠偏好：localStorage 键 + 纯读写，供 AppShell 与测试 mock。
 */
export const RAIL_COLLAPSED_STORAGE_KEY = 'lanjing.shell.rail-collapsed';

/** 读取脊是否折叠；缺省或非法值为展开。 */
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
