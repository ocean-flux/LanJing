import type { NativeWindowControlMode } from './shell-types';

/** 仅 Windows/Linux 无边框 Tauri 窗口渲染 HTML 三键。 */
export function shouldRenderHtmlWindowControls(mode: NativeWindowControlMode): boolean {
  return mode === 'windows-overlay';
}
