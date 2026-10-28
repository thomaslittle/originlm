import { BroadcastIcon, FlaskIcon } from '@phosphor-icons/react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { useActions, useDemo } from '@/state/context';
import { cn } from '@/lib/utils';

/** Always-visible provenance strip: synthetic example data must never be mistaken for live results. */
export function ModeBanner() {
  const actions = useActions();
  const mode = useDemo((state) => state.mode);
  const canPair = useDemo((state) => state.canPair);
  const live = mode === 'live';
  return (
    <div
      id="mode-banner"
      role="status"
      className={cn(
        mode,
        'flex flex-wrap items-center gap-x-3 gap-y-1 border-b px-4 py-2 text-xs md:px-6',
        live ? 'bg-linear-to-r from-success/15 via-success/5 to-transparent' : 'bg-linear-to-r from-info/15 via-info/5 to-transparent',
      )}
    >
      {live ? <BroadcastIcon className="size-3.5 text-success" aria-hidden="true" /> : <FlaskIcon className="size-3.5 text-info" aria-hidden="true" />}
      <strong className="font-semibold whitespace-nowrap">{live ? 'Live connection' : 'Synthetic showcase'}</strong>
      <span className="text-muted-foreground">
        {live
          ? 'Only native data is shown. Execution requires your explicit action and a host-granted workspace.'
          : canPair
            ? 'Example accounts and models. Pair a runtime to see yours. No credentials inspected or inference used.'
            : 'Example accounts and models. No credentials inspected. No inference used.'}
      </span>
      {live ? (
        <Button variant="ghost" size="xs" onClick={() => void actions.revokeConnection()} className="ml-auto">
          Revoke connection
        </Button>
      ) : canPair ? (
        <Button size="xs" onClick={() => actions.pair()} className="ml-auto">
          Pair a runtime
        </Button>
      ) : (
        <Badge variant="info" className="ml-auto hidden sm:inline-flex">
          Example data
        </Badge>
      )}
    </div>
  );
}
