import { RuntimeEvidence } from '@/components/common/runtime-evidence';
import { ArrowsClockwiseIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { CodeBlock } from '@/components/common/code-block';
import { Section } from '@/components/common/section';
import { useActions, useDemo } from '@/state/context';

export default function DiagnosticsPage() {
  const actions = useActions();
  const diagnostics = useDemo((state) => state.diagnostics);
  const handshake = useDemo((state) => state.handshake);
  const selection = useDemo((state) => state.selection);
  return (
    <div className="grid gap-5">
      <RuntimeEvidence />
      <section data-capture="sdk" className="border-y py-5">
        <h2 className="text-xl font-semibold">One dependency</h2>
        <p className="my-3 text-sm text-muted-foreground">Node.js, using a locally packed SDK and helper bundle.</p>
        <CodeBlock label="TypeScript">{`import { createLocalAI } from "originlm";
const ai = await createLocalAI();
try {
  const snapshot = await ai.snapshot.get();
  console.log(snapshot.revision);
} finally {
  await ai.close();
}`}</CodeBlock>
      </section>
      <section data-capture="transport" className="border-y py-5">
        <h2 className="text-xl font-semibold">One public protocol</h2>
        <p className="mt-2 text-sm text-muted-foreground">Protocol {handshake?.protocolVersion ?? 'not negotiated'}. Methods advertised by this runtime:</p>
        <ul className="mt-4 flex flex-wrap gap-3">
          {handshake?.methods.slice(0, 6).map((method) => <li key={method} className="rounded-md border px-3 py-2 font-mono text-sm">{method}</li>)}
        </ul>
      </section>
      <Section title="SDK / protocol inspector" description="Only normalized public responses are shown. No credentials are displayed.">
        <Button variant="outline" size="sm" onClick={() => void actions.refreshDiagnostics()}>
          <ArrowsClockwiseIcon data-icon="inline-start" />
          Refresh diagnostics
        </Button>
        <div className="mt-4">
          {diagnostics ? <CodeBlock>{JSON.stringify(diagnostics, null, 2)}</CodeBlock> : <p className="text-xs text-muted-foreground">Diagnostics have not been requested.</p>}
        </div>
        <details className="mt-5 border-t pt-3.5">
          <summary className="cursor-pointer text-xs text-muted-foreground select-none hover:text-foreground">Negotiated public methods</summary>
          <CodeBlock className="mt-3">{JSON.stringify(handshake ?? {}, null, 2)}</CodeBlock>
        </details>
      </Section>
      <Section title="Integration recipe" description="Use the same selection with a compatible runtime. Token values are never included.">
        <CodeBlock label="TypeScript">{`import { createLocalAI, createHttpTransport } from "originlm";

const ai = await createLocalAI({
  transport: createHttpTransport({
    baseUrl: "http://127.0.0.1:4499",
    token: appSessionToken, // obtained through explicit local pairing
  }),
});

const selection = ${JSON.stringify(selection, null, 2)};
// Request host approval and validate before starting a session.
// No turn starts merely by selecting a client.
await ai.close();`}</CodeBlock>
      </Section>
      <Section title="Known preview boundaries" description="These are implementation limitations, not hidden successful fallbacks.">
        <p className="max-w-3xl text-sm text-muted-foreground">
          This showcase consumes the public SDK. Native discovery, model/account metadata and execution are reported per adapter. Inspect the negotiated methods and repository support matrix; unavailable features are not silently substituted.
        </p>
      </Section>
    </div>
  );
}
