import { createMemoryHistory, createRootRoute, createRoute, createRouter, lazyRouteComponent, redirect, type RouterHistory } from '@tanstack/react-router';
import { CompassIcon } from '@phosphor-icons/react';
import { AppShell, PageSkeleton } from '@/components/app-shell';
import { EmptyState } from '@/components/common/empty-state';
import { DEFAULT_PATH, PAGES } from '@/lib/pages';
import { SelectorsPage } from '@/pages/selectors/selectors-page';

const title = (page: string) => ({ meta: [{ title: `${page} · OriginLM` }] });

const rootRoute = createRootRoute({
  component: AppShell,
  head: () => ({ meta: [{ title: 'OriginLM — Developer Showcase' }] }),
  notFoundComponent: () => (
    <EmptyState icon={CompassIcon} title="Page not found" description="That address is not part of the showcase. Use the sidebar to pick a page." />
  ),
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => {
    throw redirect({ to: DEFAULT_PATH, replace: true });
  },
});

// Keep the selector lab in the entry chunk for host integrations. Other pages load lazily.
const routeTree = rootRoute.addChildren([
  indexRoute,
  createRoute({ getParentRoute: () => rootRoute, path: '/overview', component: lazyRouteComponent(() => import('@/pages/overview')), head: () => title('Overview') }),
  createRoute({ getParentRoute: () => rootRoute, path: '/connections', component: lazyRouteComponent(() => import('@/pages/connections')), head: () => title('Connections') }),
  createRoute({ getParentRoute: () => rootRoute, path: '/models', component: () => <SelectorsPage catalog />, head: () => title('Models') }),
  createRoute({ getParentRoute: () => rootRoute, path: '/usage', component: lazyRouteComponent(() => import('@/pages/usage')), head: () => title('Usage') }),
  createRoute({ getParentRoute: () => rootRoute, path: '/selector-lab', component: () => <SelectorsPage />, head: () => title('Selector lab') }),
  createRoute({ getParentRoute: () => rootRoute, path: '/playground', component: lazyRouteComponent(() => import('@/pages/playground')), head: () => title('Playground') }),
  createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: lazyRouteComponent(() => import('@/pages/settings')), head: () => title('Settings') }),
  createRoute({ getParentRoute: () => rootRoute, path: '/diagnostics', component: lazyRouteComponent(() => import('@/pages/diagnostics')), head: () => title('Diagnostics') }),
]);

/** Browser history by default; tests inject an in-memory history. */
export function createAppRouter(history?: RouterHistory) {
  return createRouter({ routeTree, history, defaultPreload: 'intent', defaultPendingMs: 0, defaultPendingMinMs: 0, defaultPendingComponent: PageSkeleton, scrollRestoration: true });
}

export function createTestRouter(initialPath: string = DEFAULT_PATH) {
  return createAppRouter(createMemoryHistory({ initialEntries: [initialPath] }));
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}

/** Fetch every page chunk once the shell is up so navigation is instant. Chunks are small and same-origin. */
export function warmRoutes(router: ReturnType<typeof createAppRouter>): void {
  for (const page of PAGES) void router.preloadRoute({ to: page.path });
}
