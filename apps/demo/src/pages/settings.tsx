import { useState } from 'react';
import type { ModelPrice } from 'originlm';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useActions, useDemo } from '@/state/context';

function PriceEditor({ prices, editable }: { prices: ModelPrice[]; editable: boolean }) {
  const [draft, setDraft] = useState(() => prices.map((price, index) => ({ key: index, price: { ...price } })));
  const [message, setMessage] = useState('');
  const saving = useDemo((state) => state.savingPrices);
  const { savePrices } = useActions();
  const change = (key: number, field: keyof ModelPrice, value: string) => {
    setMessage('');
    setDraft((rows) => rows.map((row) => row.key === key ? { ...row, price: { ...row.price, [field]: value } } : row));
  };
  return (
    <form onSubmit={async (event) => { event.preventDefault(); if (await savePrices(draft.map((row) => row.price))) setMessage('Prices saved. Estimates have been recalculated.'); }}>
      <fieldset disabled={!editable || saving} className="min-w-0">
        <legend className="mb-5 text-xl font-semibold">Local price table</legend>
        <div className="divide-y border-y">
          {draft.map(({ key, price }, index) => (
            <div key={key} className="grid gap-4 py-5 sm:grid-cols-2 xl:grid-cols-3">
              {([
                ['providerId', 'Provider ID'], ['nativeModelId', 'Exact model ID'],
                ['inputUsdPerMillion', 'USD per million input tokens'], ['outputUsdPerMillion', 'USD per million output tokens'],
                ['reviewedAt', 'Reviewed on'], ['source', 'Source or local rationale'],
              ] as const).map(([field, label]) => (
                <label key={field} className="grid gap-2 text-sm">{label}
                  <Input className="h-11" value={price[field]} required maxLength={field === 'source' ? 1024 : field === 'nativeModelId' ? 256 : field === 'providerId' ? 64 : 24}
                    aria-label={`${label}, price ${index + 1}`} type={field === 'reviewedAt' ? 'date' : 'text'}
                    inputMode={field.includes('Usd') ? 'decimal' : undefined} pattern={field.includes('Usd') ? '[0-9]+(\\.[0-9]+)?' : undefined}
                    onChange={(event) => change(key, field, event.target.value)} />
                </label>
              ))}
              <Button type="button" variant="outline" onClick={() => setDraft((rows) => rows.filter((row) => row.key !== key))}>Remove price {index + 1}</Button>
            </div>
          ))}
        </div>
        <div className="mt-5 flex flex-wrap gap-3">
          <Button type="button" variant="outline" disabled={draft.length >= 128} onClick={() => setDraft((rows) => [...rows, { key: Math.max(-1, ...rows.map((row) => row.key)) + 1, price: { providerId: '', nativeModelId: '', inputUsdPerMillion: '', outputUsdPerMillion: '', reviewedAt: new Date().toISOString().slice(0, 10), source: '' } }])}>Add model price</Button>
          <Button type="submit">{saving ? 'Saving…' : 'Save prices'}</Button>
        </div>
      </fieldset>
      {message ? <p role="status" className="mt-3 text-sm">{message}</p> : null}
    </form>
  );
}

export default function SettingsPage() {
  const costs = useDemo((state) => state.costs);
  const error = useDemo((state) => state.costError);
  const mode = useDemo((state) => state.mode);
  return <div className="grid max-w-5xl gap-6">
    <p className="max-w-3xl text-muted-foreground">Prices stay with your local runtime. Match the exact provider and model IDs; aliases do not inherit a guessed rate. Estimates are API equivalents, not subscription invoices.</p>
    {error ? <p role="alert" className="text-destructive">{error}</p> : null}
    {costs?.storageError ? <p role="alert" className="text-destructive">{costs.storageError}</p> : null}
    {costs ? <>
      <p className="text-sm text-muted-foreground">{mode === 'synthetic' ? 'Example prices only. Changes last until the example server restarts.' : costs.pricesPersisted ? 'Saved in the runtime state directory.' : 'This host keeps prices in memory until it restarts.'} {costs.canEditPrices ? '' : 'Connect with administrator access to edit prices.'}</p>
      <PriceEditor key={mode} prices={costs.prices} editable={costs.canEditPrices} />
    </> : <p>Connect to a runtime that supports cost evidence to edit prices.</p>}
  </div>;
}
