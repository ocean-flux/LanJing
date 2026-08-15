import { ArrowUpRight } from 'lucide-react';
import { NavLink, useLocation } from 'react-router-dom';
import { Badge } from '@/components/ui/badge';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from '@/components/ui/sidebar';
import { useMessages } from '@/shared/i18n/messages';
import { getNavigationItems } from '@/shared/navigation';

export function AppSidebar() {
  const m = useMessages();
  const location = useLocation();
  const navigationItems = getNavigationItems();

  return (
    <Sidebar collapsible="offcanvas">
      <SidebarHeader className="px-5 pt-7 pb-5">
        <p className="eyebrow">{m.shell_workspace_label()}</p>
        <h1 className="font-display mt-2 text-xl font-semibold">{m.shell_workspace_title()}</h1>
        <p className="mt-2 text-sm leading-6 text-(--muted-text)">
          {m.shell_workspace_description()}
        </p>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarMenu aria-label={m.nav_main()}>
            {navigationItems.map(({ href, label, icon: Icon }) => {
              const active =
                href === '/' ? location.pathname === '/' : location.pathname.startsWith(href);
              return (
                <SidebarMenuItem key={href}>
                  <SidebarMenuButton asChild isActive={active} tooltip={label}>
                    <NavLink to={href} end={href === '/'}>
                      <Icon aria-hidden="true" />
                      <span>{label}</span>
                    </NavLink>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              );
            })}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter className="border-t border-sidebar-border p-4">
        <Badge className="w-fit border-(--accent)/30 bg-(--accent-soft) text-(--accent-strong)">
          {m.shell_local_only()}
        </Badge>
        <p className="mt-3 text-xs leading-5 text-(--muted-text)">
          {m.shell_local_only_description()}
        </p>
        <a
          className="mt-3 inline-flex items-center gap-1 text-xs text-(--muted-text) hover:text-(--accent-strong)"
          href="https://tauri.app"
          target="_blank"
          rel="noreferrer"
        >
          {m.shell_about_tauri()} <ArrowUpRight size={12} />
        </a>
      </SidebarFooter>
    </Sidebar>
  );
}
