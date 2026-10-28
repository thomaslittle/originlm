import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { useActions, useDemo } from '@/state/context';

const usd = new Intl.NumberFormat(undefined, { style: 'currency', currency: 'USD', minimumFractionDigits: 2, maximumFractionDigits: 6 });
const money = (value: number) => value !== 0 && Math.abs(value) < 0.000001 ? `$${value.toExponential(2)}` : usd.format(value);

/** Bars preserve missing observations instead of drawing an invented continuous spend curve. */
export function CostTrend() {
  const costs = useDemo((state) => state.costs);
  const error = useDemo((state) => state.costError);
  const mode = useDemo((state) => state.mode);
  const { refreshCosts } = useActions();
  const records = costs?.records.slice(-24) ?? [];
  const amounts = records.map((record) => record.amountUsd === null ? null : Number(record.amountUsd));
  const latest = records.at(-1);
  const latestAmount = amounts.at(-1);
  const maximum = Math.max(...amounts.filter((amount): amount is number => amount !== null && Number.isFinite(amount)), 0.000001);
  return (
    <section aria-labelledby="cost-trend-title" className="min-w-0 border-y py-5">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h2 id="cost-trend-title" className="text-xl font-semibold">Cost trend{mode === 'synthetic' ? ' — example data' : ''}</h2>
        <div className="flex items-center gap-4">
          <Button variant="ghost" onClick={() => void refreshCosts()}>Refresh costs</Button>
          <Link to="/settings" className="text-sm underline underline-offset-4">Edit prices</Link>
        </div>
      </div>
      {error ? <p role="status" className="mt-3 text-sm text-destructive">{error}</p> : null}
      {costs?.storageError ? <p role="status" className="mt-3 text-sm text-destructive">{costs.storageError}</p> : null}
      {records.length ? (
        <>
          <p className="mt-4 text-2xl font-semibold tabular-nums">{latestAmount != null && Number.isFinite(latestAmount) ? money(latestAmount) : 'Unknown'} <span className="text-sm font-normal text-muted-foreground">{latest?.basis === 'estimated' ? 'estimated for the latest turn' : latest?.basis === 'provider_reported' ? 'reported for the latest turn' : 'for the latest turn'}</span></p>
          <p className="mt-3 text-xs text-muted-foreground">Scale: $0 to {money(maximum)} per turn</p>
          <div className="mt-2 flex h-40 items-end gap-1 border-b" aria-hidden="true">
            {records.map((record, index) => (
              <div key={`${record.usage.sessionId}:${record.usage.turnId}`} className="flex h-full min-w-0 flex-1 items-end">
                {amounts[index] !== null && Number.isFinite(amounts[index]) ? (
                  <div className={record.basis === 'estimated' ? 'w-full border border-primary bg-primary/20' : 'w-full bg-primary'}
                    style={{ height: `${Math.max(1, (amounts[index] ?? 0) / maximum * 100)}%` }} />
                ) : <span className="w-full border-b border-dashed border-muted-foreground" />}
              </div>
            ))}
          </div>
          <p className="mt-2 text-sm text-muted-foreground">Latest {records.length} observed turns, oldest first. Solid: provider reported. Outlined: estimated. Gaps: unknown.</p>
          <details className="mt-3 text-sm">
            <summary className="cursor-pointer py-2 underline underline-offset-4">Read cost observations</summary>
            <ul className="divide-y">
              {records.map((record, index) => (
                <li key={`${record.usage.sessionId}:${record.usage.turnId}`} className="flex flex-wrap justify-between gap-2 py-3">
                  <span>{record.nativeModelId} <time dateTime={record.usage.observedAt}>{new Date(record.usage.observedAt).toLocaleString()}</time></span>
                  <span className="tabular-nums">{amounts[index] === null || !Number.isFinite(amounts[index]) ? 'Unknown — usage or price missing' : `${money(amounts[index] ?? 0)} ${record.basis === 'estimated' ? 'estimated' : 'provider reported'}`}{record.usage.complete ? '' : ' (in progress)'}</span>
                </li>
              ))}
            </ul>
          </details>
        </>
      ) : <p className="py-8 text-muted-foreground">No observed costs yet. A turn needs reported money, or both token counts and a matching local price.</p>}
      <p className="mt-3 max-w-3xl text-sm text-muted-foreground">{costs?.scope ?? 'Cost evidence has not been loaded.'} Estimates use standard input/output rates; caching, tools, tier discounts and subscriptions may differ.</p>
    </section>
  );
}
