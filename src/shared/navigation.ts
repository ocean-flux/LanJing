import { Compass, FolderHeart, Home, Layers3, Settings, type LucideIcon } from 'lucide-react';
import { m } from '@/shared/i18n/messages';

export interface NavigationItem {
  href: string;
  label: string;
  icon: LucideIcon;
}

export function getNavigationItems(): NavigationItem[] {
  return [
    { href: '/', label: m.nav_realm(), icon: Home },
    { href: '/apps', label: m.nav_apps(), icon: Layers3 },
    { href: '/sources', label: m.nav_sources(), icon: Compass },
    { href: '/library', label: m.nav_library(), icon: FolderHeart },
    { href: '/settings', label: m.settings(), icon: Settings },
  ];
}

export function isNavigationActive(pathname: string, href: string) {
  return href === '/' ? pathname === '/' : pathname === href || pathname.startsWith(`${href}/`);
}
