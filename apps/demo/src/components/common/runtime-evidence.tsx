import { useDemo } from '@/state/context';
const object = (value: unknown): Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {};
const flag = (value: unknown) => value === true ? 'Allowed' : value === false ? 'Not allowed' : 'Not reported';
export function RuntimeEvidence() {
  const diagnostics = object(useDemo((state) => state.diagnostics));
  const handshake = useDemo((state) => state.handshake);
  const mode = useDemo((state) => state.mode);
  const grants = object(diagnostics.grants);
  return <section data-capture="runtime" className="border-y py-6">
    <h2 className="text-xl font-semibold">Runtime and grants</h2>
    <p className="mt-2 text-sm text-muted-foreground">{mode === 'synthetic' ? 'Example runtime. No provider execution.' : 'Permissions reported for this application.'}</p>
    <dl className="mt-6 grid gap-5 sm:grid-cols-2">
      {[
        ['Protocol', handshake?.protocolVersion ?? 'Not connected'],
        ['Passive discovery', flag(grants.allowPassiveDiscovery)],
        ['Identity access', flag(grants.revealIdentity)],
        ['Execution', mode === 'synthetic' ? 'Example responses only' : flag(grants.execution)],
      ].map(([name, value]) => <div key={name} className="border-l pl-4"><dt className="text-sm text-muted-foreground">{name}</dt><dd className="mt-1 text-lg font-medium">{value}</dd></div>)}
    </dl>
    <p className="mt-6 text-sm text-muted-foreground">Discovery is not permission to execute. Grants are scoped to the requesting application.</p>
  </section>;
}
