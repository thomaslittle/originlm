import { MagnifyingGlassIcon, PlugsConnectedIcon, BinocularsIcon } from '@phosphor-icons/react';
import { Breadcrumb, BreadcrumbItem, BreadcrumbList, BreadcrumbPage, BreadcrumbSeparator } from '@/components/ui/breadcrumb';
import { Button } from '@/components/ui/button';
import { Kbd, KbdGroup } from '@/components/ui/kbd';
import { Separator } from '@/components/ui/separator';
import { SidebarTrigger } from '@/components/ui/sidebar';
import { ThemeToggle } from '@/components/layout/theme-toggle';
import { usePageMeta } from '@/lib/pages';
import { useActions, useDemo } from '@/state/context';

export function Header() {
  const actions = useActions();
  const { page } = usePageMeta();
  const live = useDemo((state) => state.mode !== 'synthetic');
  const hasClient = useDemo((state) => state.hasClient);
  const openPalette = () => actions.patch({ paletteOpen: true });
  return (
    <header className="sticky top-0 z-20 flex h-14 shrink-0 items-center gap-2 border-b bg-background px-3 md:px-5">
      <SidebarTrigger className="-ml-1 size-11" />
      <Separator orientation="vertical" className="mx-1 data-[orientation=vertical]:h-4" />
      <Breadcrumb className="hidden sm:block">
        <BreadcrumbList>
          <BreadcrumbItem className="text-muted-foreground">Workspace</BreadcrumbItem>
          <BreadcrumbSeparator />
          <BreadcrumbItem>
            <BreadcrumbPage>{page}</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>
      <div data-top-actions="" className="ml-auto flex items-center gap-2">
        <Button variant="outline" aria-label="Open model command palette" onClick={openPalette} className="hidden h-8 w-64 justify-start gap-2 text-muted-foreground md:inline-flex">
          <MagnifyingGlassIcon aria-hidden="true" />
          <span className="flex-1 text-left">Find a route…</span>
          <KbdGroup aria-hidden="true">
            <Kbd>Ctrl</Kbd>
            <Kbd>K</Kbd>
          </KbdGroup>
        </Button>
        <Button variant="outline" size="icon" aria-label="Open model command palette" onClick={openPalette} className="md:hidden">
          <MagnifyingGlassIcon />
        </Button>
        <ThemeToggle />
        {live ? (
          <Button variant="outline" onClick={() => void actions.scan()} disabled={!hasClient}>
            <BinocularsIcon data-icon="inline-start" />
            Scan local clients
          </Button>
        ) : (
          <Button onClick={() => actions.patch({ connectOpen: true })}>
            <PlugsConnectedIcon data-icon="inline-start" />
            Connect local runtime
          </Button>
        )}
      </div>
    </header>
  );
}
