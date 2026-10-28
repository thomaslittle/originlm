import { useState } from 'react';
import { ArrowRightIcon, FlaskIcon, KeyIcon } from '@phosphor-icons/react';
import type { LocalAIClient } from 'originlm';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader } from '@/components/ui/card';
import { Field, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Spinner } from '@/components/ui/spinner';
import { fetchPairingCode, pairWithCode } from '@/lib/pairing';

export function Pairing({ onPaired, onExampleData }: { onPaired: (client: LocalAIClient) => void; onExampleData?: () => void }) {
  const [code, setCode] = useState('');
  const [status, setStatus] = useState('');
  const [failed, setFailed] = useState(false);
  const [working, setWorking] = useState(false);

  function showCode() {
    setWorking(true);
    setFailed(false);
    setStatus('Getting the current code\u2026');
    fetchPairingCode()
      .then(
        (pairing) => {
          setCode(pairing.code);
          setStatus(`${pairing.issued ? 'New code issued. ' : ''}Code expires in about ${pairing.expiresInSeconds} seconds.`);
        },
        () => {
          setFailed(true);
          setStatus('Could not get a pairing code from this local runtime. Check that the helper is running and reload the page.');
        },
      )
      .finally(() => setWorking(false));
  }

  function submit() {
    setWorking(true);
    setFailed(false);
    setStatus('Connecting\u2026');
    pairWithCode(code.trim()).then(
      (client) => {
        setCode('');
        onPaired(client);
      },
      () => {
        setFailed(true);
        setStatus('Pairing failed. Get a fresh code and try again.');
        setWorking(false);
      },
    );
  }

  return (
    <main className="pairing-screen">
      <Card className="pairing-card w-full max-w-lg gap-6 bg-card py-7 [--card-spacing:--spacing(7)]">
        <CardHeader>
          <span className="mb-2 grid size-11 place-items-center rounded-xl bg-primary/10 text-brand-foreground ring-1 ring-primary/25">
            <KeyIcon className="size-5" aria-hidden="true" />
          </span>
          <h1 data-slot="card-title" className="text-2xl font-semibold tracking-tight">
            Connect your local runtime
          </h1>
          <CardDescription>
            Connect to the local helper to inspect available providers and models. Pairing checks metadata only; model turns still require your action.
          </CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4">
          <Field>
            <FieldLabel htmlFor="pairing-code">Pairing code</FieldLabel>
            <Input
              id="pairing-code"
              maxLength={12}
              autoComplete="off"
              spellCheck={false}
              placeholder="12-character code"
              value={code}
              onChange={(event) => setCode(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && !working) {
                  event.preventDefault();
                  submit();
                }
              }}
              className="font-mono"
            />
          </Field>
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" onClick={showCode} disabled={working}>
              Show current pairing code
            </Button>
            <Button onClick={submit} disabled={working}>
              {working ? <Spinner data-icon="inline-start" /> : null}
              Pair with this runtime
            </Button>
          </div>
          <p aria-live="polite" role={failed ? 'alert' : undefined} className="text-sm text-muted-foreground empty:sr-only">
            {status}
          </p>
          <p className="text-xs leading-relaxed text-muted-foreground">
            Access is limited to the permissions granted when starting this runtime. Your provider credentials stay with the native client.
          </p>
          {onExampleData ? (
            <div className="grid gap-3 border-t pt-5">
              <p className="text-center text-[0.65rem] font-semibold text-muted-foreground">Or</p>
              <Button variant="outline" onClick={onExampleData} className="h-auto w-full justify-start gap-3 rounded-xl px-3.5 py-3 text-left whitespace-normal">
                <span aria-hidden="true" className="grid size-9 shrink-0 place-items-center rounded-lg bg-info/10 text-info ring-1 ring-info/25">
                  <FlaskIcon className="size-4.5" />
                </span>
                <span className="grid min-w-0 flex-1 gap-0.5">
                  <span className="text-sm font-semibold">Explore example data</span>
                  <span className="text-xs leading-snug font-normal text-muted-foreground">Sample accounts, clearly labeled. Never mixed with real results.</span>
                </span>
                <ArrowRightIcon aria-hidden="true" className="ml-auto size-4 shrink-0 text-muted-foreground" />
              </Button>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </main>
  );
}
