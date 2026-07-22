import type { NativeWindowControlMode } from './shell-types';

/** 仅无边框浏览器预览与 Windows/Linux overlay 渲染 HTML 三键。 */
export function shouldRenderHtmlWindowControls(mode: NativeWindowControlMode): boolean {
  return mode === 'browser-preview' || mode === 'windows-overlay';
}
