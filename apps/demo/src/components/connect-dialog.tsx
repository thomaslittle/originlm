import { useState, type FormEvent } from 'react';
import { PlugsConnectedIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Field, FieldDescription, FieldGroup, FieldLabel } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { useActions } from '@/state/context';

/** The token is held only in this form's state and cleared on submit. It is never persisted. */
export default function ConnectDialog() {
  const actions = useActions();
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:4499');
  const [token, setToken] = useState('');

  function submit(event: FormEvent) {
    event.preventDefault();
    const sessionToken = token;
    setToken('');
    actions.patch({ connectOpen: false });
    void actions.connectLive(endpoint, sessionToken);
  }

  return (
    <Dialog open onOpenChange={(open) => actions.patch({ connectOpen: open })}>
      <DialogContent className="sm:max-w-lg" aria-label="Connect an authorized local runtime">
        <form onSubmit={submit} className="grid gap-5">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2 text-lg">
              <PlugsConnectedIcon className="size-5 text-brand-foreground" aria-hidden="true" />
              Connect local runtime
            </DialogTitle>
            <DialogDescription>
              Connect only to a runtime you started and explicitly authorized. Use an app-session token, never an AI provider key.
            </DialogDescription>
          </DialogHeader>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="runtime-endpoint">Runtime endpoint</FieldLabel>
              <Input id="runtime-endpoint" type="url" required value={endpoint} onChange={(event) => setEndpoint(event.target.value)} />
            </Field>
            <Field>
              <FieldLabel htmlFor="app-token">App session token</FieldLabel>
              <Input id="app-token" type="password" autoComplete="off" required value={token} onChange={(event) => setToken(event.target.value)} />
              <FieldDescription>Live execution is disabled in this preview. Failed live connection attempts never fall back to example accounts.</FieldDescription>
            </Field>
          </FieldGroup>
          <DialogFooter>
            <Button type="submit">Connect runtime</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
