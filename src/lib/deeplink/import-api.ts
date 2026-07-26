import { invoke } from '@tauri-apps/api/core';

export interface FetchImportSrcRequest {
  url: string;
}

/** 通过 Rust 安全 HTTP seam 读取深链 src，不在 WebView 直接发起跨域请求。 */
export function fetchImportSrc(url: string): Promise<string> {
  return invoke<string>('fetch_import_src', { request: { url } });
}
