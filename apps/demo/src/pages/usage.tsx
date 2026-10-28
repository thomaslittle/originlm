import { ResetCountdown } from '@/components/common/reset-countdown';
import { CostTrend } from '@/components/common/cost-trend';
import { lazy, Suspense } from 'react';
import { ClockIcon, GaugeIcon, LinkIcon, StackIcon } from '@phosphor-icons/react';
import type { QuotaPool, UsageWindow } from 'originlm';
import { Badge } from '@/components/ui/badge';
import { Card } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { EmptyState } from '@/components/common/empty-state';
import { percent, readingOf, type Reading } from '@/lib/usage';
import { useDemo } from '@/state/context';

// Charts (recharts) load after the numbers: the page is readable before the chart chunk arrives.
const UsageRing = lazy(() => import('@/pages/usage-charts').then((module) => ({ default: module.UsageRing })));
const UtilizationChart = lazy(() => import('@/pages/usage-charts').then((module) => ({ default: module.UtilizationChart })));

function CountBadge({ reading }: { reading: Reading }) {
  return (
    <div className="relative mx-auto grid size-40 shrink-0 place-content-center rounded-full border-2 border-dashed text-center">
      <span className="text-3xl font-semibold tracking-tight tabular-nums">{reading.used}</span>
      <span className="text-[0.65rem] text-muted-foreground">{reading.unit}</span>
    </div>
  );
}

function WindowReading({ window }: { window: UsageWindow }) {
  const reading = readingOf(window);
  if (!reading) {
    return <EmptyState icon={GaugeIcon} title="Usage unavailable" description={window.reading.state === 'known' ? undefined : window.reading.reason} />;
  }
  return (
    <div className="grid items-center gap-5 sm:grid-cols-[10rem_minmax(0,1fr)]">
      {reading.fraction === null ? (
        <CountBadge reading={reading} />
      ) : (
        <Suspense fallback={<Skeleton className="mx-auto size-40 rounded-full" />}>
          <UsageRing reading={reading} />
        </Suspense>
      )}
      <div className="min-w-0">
        <p className="text-xs font-medium text-muted-foreground">{window.label}</p>
        <div className="mt-1 text-4xl font-semibold tracking-tight tabular-nums">
          {reading.used}
          <span className="ml-1.5 text-sm font-normal text-muted-foreground">{`${reading.unit}${reading.limit == null ? '' : ` / ${reading.limit}`}`}</span>
        </div>
        {reading.fraction === null ? <p className="mt-2 text-xs text-muted-foreground">No published total \u00b7 no percentage implied</p> : null}
        <div className="mt-4 flex flex-wrap items-center gap-2.5 text-xs text-muted-foreground">
          <Badge variant={reading.freshness === 'stale' ? 'warning' : 'outline'}>{reading.freshness}</Badge>
          <span className="inline-flex items-center gap-1.5">
            <ClockIcon className="size-3.5" aria-hidden="true" />
            <ResetCountdown at={window.resetsAt} />
          </span>
        </div>
      </div>
    </div>
  );
}

function PoolCard({ pool, synthetic }: { pool: QuotaPool; synthetic: boolean }) {
  return (
    <Card data-pool={pool.id} className="gap-0 bg-card p-6 [--card-spacing:--spacing(0)]">
      <div className="mb-5 flex items-start justify-between gap-3">
        <h2 className="text-lg font-semibold tracking-tight">{pool.windows[0]?.label ?? pool.id}</h2>
        <Badge variant={pool.sharing === 'verified' ? 'success' : 'outline'}>{pool.sharing === 'verified' ? 'Verified sharing' : 'Sharing unknown'}</Badge>
      </div>
      <div className="grid gap-6">
        {pool.windows.map((window) => (
          <WindowReading key={window.id} window={window} />
        ))}
      </div>
      <p className="mt-5 flex items-center gap-1.5 border-t pt-4 text-[0.7rem] leading-relaxed text-muted-foreground">
        <LinkIcon className="size-3 shrink-0" aria-hidden="true" />
        {`${pool.routeIds.length} linked route${pool.routeIds.length === 1 ? '' : 's'}. ${synthetic ? 'Illustrative allowance only.' : ''}`}
      </p>
    </Card>
  );
}

function Stat({ label, value, note }: { label: string; value: string; note: string }) {
  return (
    <Card className="gap-1 bg-card p-5 [--card-spacing:--spacing(0)]">
      <span className="text-3xl font-semibold tracking-tight tabular-nums">{value}</span>
      <span className="text-sm font-medium">{label}</span>
      <small className="text-xs text-muted-foreground">{note}</small>
    </Card>
  );
}

export default function UsagePage() {
  const pools = useDemo((state) => state.snapshot?.quotaPools ?? []);
  const synthetic = useDemo((state) => state.mode === 'synthetic');

  if (!pools.length) {
    return <EmptyState icon={GaugeIcon} title="No quota pools reported" description="Usage appears only when an authorized check returns provider-reported windows." />;
  }

  const withTotals = pools.flatMap((pool) => {
    const reading = pool.windows.map(readingOf).find((item) => item?.fraction != null);
    return reading ? [{ pool, reading }] : [];
  });
  const peak = withTotals.reduce((max, item) => Math.max(max, item.reading.fraction ?? 0), 0);
  const windows = pools.reduce((sum, pool) => sum + pool.windows.length, 0);
  const verified = pools.filter((pool) => pool.sharing === 'verified').length;

  return (
    <div className="grid min-w-0 grid-cols-1 gap-5">
      <p className="max-w-3xl text-sm text-muted-foreground">Windows belong to quota pools, not model rows. Shared pools appear once; unknown totals stay unknown.</p>
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <Stat label="Quota pools" value={String(pools.length)} note={`${windows} usage window${windows === 1 ? '' : 's'}`} />
        <Stat label="With a published total" value={String(withTotals.length)} note="Percentages appear only for these" />
        <Stat label="Highest utilization" value={withTotals.length ? percent(peak) : 'None'} note={withTotals.length ? 'Across pools with totals' : 'No pool reports a total'} />
        <Stat label="Verified sharing" value={String(verified)} note="Pools shared across routes, as reported" />
      </div>
      {withTotals.length ? (
        <Suspense fallback={<Skeleton className="h-72 rounded-2xl" />}>
          <UtilizationChart readings={withTotals} />
        </Suspense>
      ) : null}
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        {pools.map((pool) => (
          <PoolCard key={pool.id} pool={pool} synthetic={synthetic} />
        ))}
      </div>
      <p className="flex items-center gap-1.5 text-[0.7rem] text-muted-foreground">
        <StackIcon className="size-3" aria-hidden="true" />
        Quota windows come from the connection that owns them and are never summed across providers.
      </p>
      <CostTrend />
    </div>
  );
}
