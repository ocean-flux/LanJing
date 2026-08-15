import type { IconName } from '@/components/Icon';
import { m } from '@/shared/i18n/messages';

export interface NavigationItem {
  href: string;
  label: string;
  icon: IconName;
  /** 子项在侧栏里渲染为 SidebarMenuSub。 */
  children?: NavigationItem[];
}

export function getNavigationItems(): NavigationItem[] {
  return [
    { href: '/', label: m.nav_realm(), icon: 'compass' },
    { href: '/apps', label: m.nav_apps(), icon: 'squares-four' },
    {
      href: '/sources',
      label: m.nav_sources(),
      icon: 'broadcast',
      children: [{ href: '/sources/rules', label: m.nav_rules(), icon: 'tree-structure' }],
    },
    { href: '/library', label: m.nav_library(), icon: 'books' },
  ];
}

export function isNavigationActive(pathname: string, href: string) {
  return href === '/' ? pathname === '/' : pathname === href || pathname.startsWith(`${href}/`);
}
