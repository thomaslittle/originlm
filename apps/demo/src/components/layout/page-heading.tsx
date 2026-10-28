import { Badge } from '@/components/ui/badge';
import { usePageMeta } from '@/lib/pages';
import { useDemo } from '@/state/context';

export function PageHeading() {
  const { page, description } = usePageMeta();
  const live = useDemo((state) => state.mode !== 'synthetic');
  const modelCount = useDemo((state) => state.snapshot?.models.length);
  return (
    <div className="mb-8 flex items-end justify-between gap-4">
      <div className="min-w-0">
        <h1 id="page-title" className="text-3xl font-semibold tracking-tight sm:text-4xl">
          {page}
        </h1>
        <p className="mt-2 max-w-2xl text-sm text-muted-foreground sm:text-base">{description}</p>
      </div>
      <Badge variant="outline" className="hidden h-7 shrink-0 px-3 sm:inline-flex">
        {modelCount === undefined ? 'Not connected' : `${modelCount.toLocaleString()}${live ? ' ' : ' example '}models`}
      </Badge>
    </div>
  );
}
