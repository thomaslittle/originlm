import { CodeBlock } from "../components/CodeBlock";
import { Title } from "../components/Title";

const RULES: [string, string, string][] = [
  ["Read the snapshot, models and usage", "snapshot.get · models.page · usage.read", "An app credential. No execution grant."],
  ["Refresh an account’s metadata", "connections.probe", "A probe grant for that connection or provider."],
  ["Run a turn", "sessions.create · turns.start", "An execution grant for the connection or provider, and a named workspace."],
  ["Use a metered or prepaid route", "allowPaid", "Paid consent in the grant as well. Off by default."],
  ["Write files or use native tools", "workspace_write · native_policy", "Granted separately from execution."],
  ["Stop an app", "apps.revoke", "Administrator credential. Ends that app’s live connections and sessions only."],
];

const MANIFEST = `{
  "schemaVersion": 1,
  "apps": [{
    "id": "notes-app",
    "tokenFile": "/home/me/.config/notes-app/originlm.token",
    "allowPassiveDiscovery": true,
    "connectionIds": ["claude-personal"],
    "execution": {
      "connectionIds": ["claude-personal"],
      "workspaces": { "notes": "/home/me/notes" },
      "allowPaid": false
    }
  }]
}`;

export function Grants() {
  return (
    <section className="section grants" id="grants" aria-labelledby="grants-title">
      <div className="container">
        <div className="grants-head">
          <Title id="grants-title" text="Discovery is not permission." />
          <p className="lead">
            Seeing that a client is installed lets an app read, not run. Execution needs a grant from
            the trusted host, scoped to connections and workspaces. <code>grants.request</code> only
            asks: it returns <code>requires_host_approval</code> and changes nothing.
          </p>
        </div>

        <div className="grants-grid">
          <div className="rules" role="region" aria-label="What each operation requires" tabIndex={0}>
            <table>
              <caption className="visually-hidden">Operations and the permission each one needs</caption>
              <thead>
                <tr><th scope="col">To</th><th scope="col">Method</th><th scope="col">Needs</th></tr>
              </thead>
              <tbody>
                {RULES.map(([what, method, needs]) => (
                  <tr key={what}>
                    <th scope="row">{what}</th>
                    <td><code>{method}</code></td>
                    <td>{needs}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="grants-manifest">
            <CodeBlock code={MANIFEST} title="grants.json (shared broker, owner-only file)" />
            <p className="fine">
              A shared broker reads grants from an owner-only manifest and mints one credential per app.
              An embedded Rust host calls <code>Broker::authorize</code> per app instead. No protocol
              request can widen its own grant. <a href="https://github.com/thomaslittle/originlm/blob/main/docs/integration/shared-broker.md">Shared broker guide</a>
            </p>
          </div>
        </div>
      </div>
    </section>
  );
}
