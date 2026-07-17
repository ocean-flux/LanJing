/**
 * 四境主导航目的地：rail 与 bottom 共用，避免双数据源。
 * 设置 / 搜索 / 小说入口不在此清单（脊上工具或媒体内导航）。
 */
import { m } from '$lib/i18n';
import type { ShellRoute } from './shell-types';

/** 主导航可跳转 href。 */
export type PrimaryNavHref = '/' | '/apps' | '/sources' | '/library';

/** 四境导航项（无设置/搜索/媒体子路径）。 */
export type PrimaryNavigationItem = {
  key: ShellRoute;
  href: PrimaryNavHref;
  label: string;
};

/** 固定四境顺序：境场 → 应用 → 来源 → 资料库。 */
export function getPrimaryNavigationItems(): PrimaryNavigationItem[] {
  return [
    { key: 'realm', href: '/', label: m.nav_realm() },
    { key: 'apps', href: '/apps', label: m.nav_apps() },
    { key: 'sources', href: '/sources', label: m.nav_sources() },
    { key: 'library', href: '/library', label: m.nav_library() },
  ];
}
