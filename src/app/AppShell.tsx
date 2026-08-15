import { Outlet, useLocation } from 'react-router-dom';
import { AppSidebar } from './AppSidebar';
import { Titlebar } from './Titlebar';
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar';
import { appConfig } from '@/shared/config/app';
import { useMessages } from '@/shared/i18n/messages';

type Messages = ReturnType<typeof useMessages>;

/** 页面标题由路由派生，页面自身不再重复承担标题职责。 */
function pageTitle(pathname: string, m: Messages): string {
  if (pathname === '/') return m.nav_realm();
  if (pathname.startsWith('/apps')) return m.nav_apps();
  if (pathname.startsWith('/sources/rules')) return m.nav_rules();
  if (pathname.startsWith('/sources')) return m.nav_sources();
  if (pathname.startsWith('/library/item')) return m.library_detail_title();
  if (pathname.startsWith('/library')) return m.nav_library();
  if (pathname.startsWith('/settings')) return m.settings();
  return appConfig.name;
}

export function AppShell() {
  const m = useMessages();
  const { pathname } = useLocation();

  return (
    <>
      <a className="skip-link" href="#main-content">
        {m.shell_skip_to_main()}
      </a>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset className="min-h-dvh bg-canvas text-ink">
          <Titlebar title={pageTitle(pathname, m)} />
          <main id="main-content" className="app-scroll-region min-w-0 flex-1">
            <Outlet />
          </main>
        </SidebarInset>
      </SidebarProvider>
    </>
  );
}
