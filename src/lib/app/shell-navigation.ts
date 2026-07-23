/**
 * 四境主导航目的地：桌面标题栏与移动底栏共用，避免双数据源。
 * 设置独立位于 app bar；搜索和媒体子路由不进入此清单。
 */
import { m } from '$lib/i18n';
import type { ShellRoute } from './shell-types';

/** 主导航可跳转 href。 */
export type PrimaryNavHref = '/' | '/apps' | '/sources' | '/library';
type PrimaryNavigationIcon = 'compass' | 'squares-four' | 'broadcast' | 'database';

/** 四境导航项（无设置/搜索/媒体子路径）。 */
export type PrimaryNavigationItem = {
  key: ShellRoute;
  icon: PrimaryNavigationIcon;
  href: PrimaryNavHref;
  label: string;
};

/** 固定四境顺序：境场 → 应用 → 来源 → 资料库。 */
export function getPrimaryNavigationItems(): PrimaryNavigationItem[] {
  return [
    { key: 'realm', href: '/', icon: 'compass', label: m.nav_realm() },
    { key: 'apps', href: '/apps', icon: 'squares-four', label: m.nav_apps() },
    { key: 'sources', href: '/sources', icon: 'broadcast', label: m.nav_sources() },
    { key: 'library', href: '/library', icon: 'database', label: m.nav_library() },
  ];
}
