import { useId, useMemo, useState, type FormEvent } from "react";
import { highlight } from "../../lib/highlight";
import { DEMO_ROUTES, GRANT_SCOPES, grantManifest, tsCall } from "../shared/facts";

type ScopeId = (typeof GRANT_SCOPES)[number]["id"];
type Scopes = Record<ScopeId, boolean>;
type Slip = { app: string; routeId: string; scopes: Scopes };

const NEEDS_EXECUTION: ScopeId[] = ["workspace", "tools", "paid"];
const DEFAULT: Slip = {
  app: "photo-sorter",
  routeId: "claude",
  scopes: { metadata: true, execution: true, workspace: true, tools: false, paid: false },
};

const clean = (value: string) => value.toLowerCase().replace(/[^a-z0-9-]+/g, "-").replace(/^-+/, "").slice(0, 32);

/* The call the printed grant allows. Uses the manifest's "default" workspace so the slip
   and the code agree. Without execution, the app can still read the snapshot. */
function callFor(slip: Slip): { code: string; notes: string[] } {
  const route = DEMO_ROUTES.find((r) => r.id === slip.routeId) ?? DEMO_ROUTES[0];
  const { execution, workspace, paid, tools } = slip.scopes;
  const notes: string[] = [];
  if (!execution) {
    notes.push("No execution grant: turns.start is refused. This app can read the snapshot, models and usage only.");
    return {
      code: `import { createLocalAI } from "originlm";

const ai = await createLocalAI();
try {
  const { connections, quotaPools } = await ai.snapshot.get();
  render(connections, quotaPools);
} finally {
  await ai.close();
}`,
      notes,
    };
  }
  if (!workspace) notes.push("No workspace: execution needs a named workspace, so sessions.create has nowhere to run. Tick Workspace.");
  notes.push(paid
    ? "allowPaid is on for this app: metered or prepaid routes may run. Off by default."
    : "allowPaid is off: nothing falls back to a metered route.");
  if (tools) notes.push("allowNativePolicy lets the session use the client's native shell, file, web and build tools. Granted separately from execution.");
  return { code: tsCall(route).replace('workspaceId: "notes"', 'workspaceId: "default"'), notes };
}

export function GrantSlip({ still }: { still: boolean }) {
  const id = useId();
  const [slip, setSlip] = useState<Slip>(DEFAULT);
  const [printed, setPrinted] = useState<Slip>(DEFAULT);
  const [run, setRun] = useState(0);
  const dirty = JSON.stringify(slip) !== JSON.stringify(printed);

  const route = DEMO_ROUTES.find((r) => r.id === printed.routeId) ?? DEMO_ROUTES[0];
  const manifest = useMemo(
    () => grantManifest({ app: printed.app || "my-app", providerId: route.providerId, scopes: printed.scopes }),
    [printed, route.providerId],
  );
  const call = useMemo(() => callFor(printed), [printed]);

  function toggle(scope: ScopeId, on: boolean) {
    setSlip((s) => {
      const scopes = { ...s.scopes, [scope]: on };
      if (scope === "execution" && !on) for (const dep of NEEDS_EXECUTION) scopes[dep] = false;
      return { ...s, scopes };
    });
  }

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    setPrinted({ ...slip, app: slip.app || "my-app" });
    setRun((n) => n + 1);
  }

  return (
    <div className="c5-slipgrid">
      <form className="c5-slip" onSubmit={onSubmit} aria-labelledby={`${id}-title`}>
        <div className="c5-slip-head">
          <p className="c5-slip-no" aria-hidden="true">FORM G-1 · NO. {String(run + 1).padStart(4, "0")}</p>
          <h3 id={`${id}-title`}>Grant slip</h3>
          <p className="c5-slip-sub">One app, one route, the scopes you tick. Nothing else.</p>
        </div>

        <div className="c5-field">
          <label htmlFor={`${id}-app`}>App name</label>
          <input
            id={`${id}-app`}
            name="app"
            value={slip.app}
            onChange={(e) => setSlip((s) => ({ ...s, app: clean(e.target.value) }))}
            autoComplete="off"
            spellCheck={false}
            aria-describedby={`${id}-app-hint`}
          />
          <span className="c5-hint" id={`${id}-app-hint`}>Lowercase, digits and dashes. It names the app’s own token file.</span>
        </div>

        <div className="c5-field">
          <label htmlFor={`${id}-route`}>Route</label>
          <select id={`${id}-route`} name="route" value={slip.routeId} onChange={(e) => setSlip((s) => ({ ...s, routeId: e.target.value }))}>
            {DEMO_ROUTES.map((r) => (
              <option key={r.id} value={r.id}>{r.name} · {r.model} (example) · {r.kind}</option>
            ))}
          </select>
        </div>

        <fieldset className="c5-scopes">
          <legend>Scopes</legend>
          {GRANT_SCOPES.map((scope) => {
            const blocked = NEEDS_EXECUTION.includes(scope.id) && !slip.scopes.execution;
            return (
              <div className={`c5-scope${blocked ? " is-blocked" : ""}`} key={scope.id}>
                <input
                  type="checkbox"
                  id={`${id}-${scope.id}`}
                  name={scope.id}
                  checked={slip.scopes[scope.id]}
                  disabled={blocked}
                  onChange={(e) => toggle(scope.id, e.target.checked)}
                  aria-describedby={`${id}-${scope.id}-note`}
                />
                <label htmlFor={`${id}-${scope.id}`}>
                  <span className="c5-box" aria-hidden="true" />
                  <span className="c5-scope-name">{scope.label}</span>
                  <code>{scope.field}</code>
                </label>
                <span className="c5-scope-note" id={`${id}-${scope.id}-note`}>
                  {scope.note}{blocked ? " Needs Execution." : ""}
                </span>
              </div>
            );
          })}
        </fieldset>

        <div className="c5-slip-foot">
          <button type="submit" className="c5-btn c5-btn-print">Print grant</button>
          <span className="c5-hint" role="status">{dirty ? "Slip changed. Print to update the grant." : "Printed. This slip matches the grant."}</span>
        </div>
      </form>

      <div className="c5-tape" aria-labelledby={`${id}-out`}>
        <h3 className="c5-tape-title" id={`${id}-out`}>The grant, as printed</h3>
        <div key={run} className={`c5-tape-roll${still || run === 0 ? "" : " is-printing"}`}>
          <p className="c5-tape-label">grants.json · shared broker · owner-only file</p>
          <pre className="c5-code" tabIndex={0} aria-label="Printed grants.json"><code>{highlight(manifest)}</code></pre>
          <p className="c5-tape-label">your app · {printed.app || "my-app"}</p>
          <pre className="c5-code" tabIndex={0} aria-label="The SDK call this grant allows"><code>{highlight(call.code)}</code></pre>
          <ul className="c5-tape-notes">
            {call.notes.map((note) => <li key={note}>{note}</li>)}
            <li><code>grants.request</code> only asks: it returns <code>requires_host_approval</code> and changes nothing. No request can widen its own grant.</li>
            <li>Revoke with <code>apps.revoke</code> (administrator credential). It ends that app’s live connections and sessions only.</li>
          </ul>
        </div>
      </div>
    </div>
  );
}
