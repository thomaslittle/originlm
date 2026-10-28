import { RouterProvider, type AnyRouter } from '@tanstack/react-router';
import { DemoProvider } from '@/state/context';
import type { DemoStore } from '@/state/store';

/** React host for the developer showcase. Consumes public SDK clients only. */
export function App({ demo, router }: { demo: DemoStore; router: AnyRouter }) {
  return (
    <DemoProvider demo={demo}>
      <RouterProvider router={router} />
    </DemoProvider>
  );
}
