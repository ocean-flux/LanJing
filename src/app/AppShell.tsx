import { useEffect } from 'react';
import { Outlet } from 'react-router-dom';
import { AppSidebar } from './AppSidebar';
import { SidebarInset, SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar';
import { Titlebar } from './Titlebar';
import { useMessages } from '@/shared/i18n/messages';

export function AppShell() {
  const m = useMessages();
  useEffect(() => {
    document
      .querySelector<HTMLAnchorElement>('.skip-link')
      ?.replaceChildren(m.shell_skip_to_main());
  }, [m]);
  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset className="min-h-screen bg-(--canvas) text-(--text)">
        <Titlebar />
        <div className="flex min-h-10 items-center border-b border-(--border) px-3 md:hidden">
          <SidebarTrigger aria-label={m.nav_main()} title={m.nav_main()} />
        </div>
        <main id="main-content" className="min-w-0 flex-1 pb-6">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  );
}
