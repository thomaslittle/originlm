import { lazy, Suspense, useEffect, useEffectEvent } from 'react';
import { CSPProvider } from '@base-ui/react/csp-provider';
import { HeadContent, Outlet, useRouterState } from '@tanstack/react-router';
import { PlugsIcon, WarningIcon } from '@phosphor-icons/react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { SidebarInset, SidebarProvider } from '@/components/ui/sidebar';
import { Skeleton } from '@/components/ui/skeleton';
import { TooltipProvider } from '@/components/ui/tooltip';
import { AppSidebar } from '@/components/layout/app-sidebar';
import { Header } from '@/components/layout/header';
import { ModeBanner } from '@/components/layout/mode-banner';
import { PageHeading } from '@/components/layout/page-heading';
import { EmptyState } from '@/components/common/empty-state';
import { useActions, useDemo } from '@/state/context';

const PaletteDialog = lazy(() => import('@/components/palette'));
const ConnectDialog = lazy(() => import('@/components/connect-dialog'));

export function PageSkeleton() {
  return (
    <div className="grid gap-5" aria-hidden="true">
      <Skeleton className="h-28 rounded-2xl" />
      <div className="grid gap-5 md:grid-cols-2">
        <Skeleton className="h-56 rounded-2xl" />
        <Skeleton className="h-56 rounded-2xl" />
      </div>
    </div>
  );
}

function Notices() {
  const notice = useDemo((state) => state.notice);
  const connectionError = useDemo((state) => state.connectionError);
  // The Playground shows its own destructive alert for a failed turn; one alert per problem.
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const inlineError = useDemo((state) => state.debugError !== null) && pathname === '/playground';
  return (
    <>
      {connectionError ? (
        <Alert id="connection-error" variant="warning" className="mb-4">
          <PlugsIcon />
          <AlertDescription>{connectionError}</AlertDescription>
        </Alert>
      ) : null}
      {notice && !inlineError ? (
        <Alert variant="warning" className="mb-4">
          <WarningIcon />
          <AlertDescription>{notice}</AlertDescription>
        </Alert>
      ) : null}
    </>
  );
}

function PageOutlet() {
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const connected = useDemo((state) => state.snapshot !== null);
  const connectionError = useDemo((state) => state.connectionError);
  if (!connected) {
    return (
      <EmptyState
        icon={PlugsIcon}
        title="No connected catalog"
        description={connectionError ?? 'Pair this browser with the running local helper to load its discovery snapshot.'}
      />
    );
  }
  return (
    <div key={pathname}>
      <Suspense fallback={<PageSkeleton />}>
        <Outlet />
      </Suspense>
    </div>
  );
}

/** Ctrl/⌘+K toggles the palette. A subscription to the document is a legitimate effect. */
function GlobalShortcuts() {
  const actions = useActions();
  const paletteOpen = useDemo((state) => state.paletteOpen);
  const connectOpen = useDemo((state) => state.connectOpen);
  const onKeyDown = useEffectEvent((event: KeyboardEvent) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k' && !connectOpen) {
      event.preventDefault();
      actions.patch({ paletteOpen: !paletteOpen });
    }
  });
  useEffect(() => {
    const listener = (event: KeyboardEvent) => onKeyDown(event);
    document.addEventListener('keydown', listener);
    return () => document.removeEventListener('keydown', listener);
  }, []);
  return null;
}

function Dialogs() {
  const paletteOpen = useDemo((state) => state.paletteOpen);
  const connectOpen = useDemo((state) => state.connectOpen);
  return (
    <Suspense fallback={null}>
      {paletteOpen ? <PaletteDialog /> : null}
      {connectOpen ? <ConnectDialog /> : null}
    </Suspense>
  );
}

/** Root layout: chrome around whichever page route is active. */
export function AppShell() {
  return (
    <CSPProvider disableStyleElements>
      <TooltipProvider delay={200}>
      <HeadContent />
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset className="min-w-0 bg-transparent">
          <Header />
          <ModeBanner />
          <div className="mx-auto w-full max-w-[96rem] flex-1 px-4 py-8 md:px-8">
            <PageHeading />
            <Notices />
            <PageOutlet />
            <footer className="mt-12 flex flex-col gap-1 border-t pt-4 text-xs text-muted-foreground md:flex-row md:justify-between">
              <span>Local-first by design. Honest about what is known.</span>
              <span>v0.1 · work in progress</span>
            </footer>
          </div>
        </SidebarInset>
      </SidebarProvider>
      <GlobalShortcuts />
      <Dialogs />
      </TooltipProvider>
    </CSPProvider>
  );
}
