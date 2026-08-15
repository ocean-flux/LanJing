import { Link, useLocation } from 'react-router-dom';
import { Icon } from '@/components/Icon';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarRail,
} from '@/components/ui/sidebar';
import { appConfig } from '@/shared/config/app';
import { useMessages } from '@/shared/i18n/messages';
import { getNavigationItems, isNavigationActive } from '@/shared/navigation';

export function AppSidebar() {
  const m = useMessages();
  const { pathname } = useLocation();
  const items = getNavigationItems();

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader className="h-(--shell-titlebar-height) flex-row items-center gap-2 border-b border-hairline px-2 py-0">
        <span
          aria-hidden="true"
          className="grid size-5 shrink-0 place-items-center bg-lantern-strong font-serif text-[0.8rem] leading-none font-semibold text-on-lantern"
        >
          L
        </span>
        <span className="truncate font-medium group-data-[collapsible=icon]:hidden">
          {appConfig.name}
        </span>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {items.map((item) => (
                <SidebarMenuItem key={item.href}>
                  <SidebarMenuButton
                    isActive={isNavigationActive(pathname, item.href)}
                    tooltip={item.label}
                    render={<Link to={item.href} />}
                  >
                    <Icon name={item.icon} className="text-base" />
                    <span>{item.label}</span>
                  </SidebarMenuButton>
                  {item.children ? (
                    <SidebarMenuSub>
                      {item.children.map((child) => (
                        <SidebarMenuSubItem key={child.href}>
                          <SidebarMenuSubButton
                            isActive={isNavigationActive(pathname, child.href)}
                            render={<Link to={child.href} />}
                          >
                            <Icon name={child.icon} className="text-base" />
                            <span>{child.label}</span>
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                      ))}
                    </SidebarMenuSub>
                  ) : null}
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter className="border-t border-hairline">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              isActive={isNavigationActive(pathname, '/settings')}
              tooltip={m.settings()}
              render={<Link to="/settings" />}
            >
              <Icon name="gear-six" className="text-base" />
              <span>{m.settings()}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton tooltip={m.shell_local_only()} render={<span />}>
              <Icon name="lock" className="text-base text-ink-subtle" />
              <span className="text-ink-muted">{m.shell_local_only()}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>
  );
}
