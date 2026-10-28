import { Link } from '@tanstack/react-router';
import { GearIcon, ArrowSquareOutIcon, CubeIcon, FlaskIcon, GaugeIcon, PlugsConnectedIcon, PulseIcon, SquaresFourIcon, ChatsIcon, type Icon } from '@phosphor-icons/react';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  useSidebar,
} from '@/components/ui/sidebar';
import { Badge } from '@/components/ui/badge';
import { BrandMark } from '@/components/layout/brand-mark';
import { pathFor, usePageMeta } from '@/lib/pages';
import { useDemo } from '@/state/context';
import type { Page } from '@/state/types';

const NAV: { label: string; items: { page: Page; icon: Icon }[] }[] = [
  {
    label: 'Explore',
    items: [
      { page: 'Overview', icon: SquaresFourIcon },
      { page: 'Connections', icon: PlugsConnectedIcon },
      { page: 'Models', icon: CubeIcon },
      { page: 'Usage', icon: GaugeIcon },
      { page: 'Settings', icon: GearIcon },
    ],
  },
  {
    label: 'Build',
    items: [
      { page: 'Selector lab', icon: FlaskIcon },
      { page: 'Playground', icon: ChatsIcon },
      { page: 'Diagnostics', icon: PulseIcon },
    ],
  },
];

function NavItem({ page, icon: IconComponent }: { page: Page; icon: Icon }) {
  const active = usePageMeta().page === page;
  const { setOpenMobile } = useSidebar();
  const count = useDemo((state) =>
    page === 'Connections' ? state.snapshot?.connections.length : page === 'Models' ? state.snapshot?.models.length : undefined,
  );
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active} tooltip={page} render={<Link to={pathFor(page)} aria-label={page} onClick={() => setOpenMobile(false)} />}>
        <IconComponent weight={active ? 'fill' : 'regular'} />
        <span>{page}</span>
      </SidebarMenuButton>
      {count ? <SidebarMenuBadge>{count.toLocaleString()}</SidebarMenuBadge> : null}
    </SidebarMenuItem>
  );
}

export function AppSidebar() {
  return (
    <Sidebar collapsible="icon">
      <SidebarHeader className="p-3">
        <Link
          to="/overview"
          aria-label="OriginLM home"
          className="inline-flex w-fit rounded-[26%] p-0.5 outline-none focus-visible:ring-2 focus-visible:ring-sidebar-ring"
        >
          <BrandMark />
        </Link>
      </SidebarHeader>
      <SidebarContent>
        {NAV.map((group) => (
          <SidebarGroup key={group.label}>
            <SidebarGroupLabel>{group.label}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {group.items.map((item) => (
                  <NavItem key={item.page} {...item} />
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
      </SidebarContent>
      <SidebarFooter className="gap-3 p-3 group-data-[collapsible=icon]:hidden">
        <div className="rounded-xl border bg-surface-subtle p-3 text-xs">
          <Badge variant="brand">Implementation preview</Badge>
          <p className="mt-2 leading-relaxed text-muted-foreground">Evidence over assumption. Selection is never authorization.</p>
        </div>
        <a href="https://github.com/thomaslittle/originlm" target="_blank" rel="noreferrer" className="inline-flex items-center gap-1.5 px-1 text-xs text-brand-foreground hover:underline">
          View repository
          <ArrowSquareOutIcon className="size-3" aria-hidden="true" />
          <span className="sr-only">(opens in a new tab)</span>
        </a>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
