import { Title } from "../components/Title";
const ITEMS: [string, string][] = [
  ["Credentials stay with the clients", "Provider credentials remain where the native client keeps them. Snapshots carry evidence, never credential values. Browser cookies are never read."],
  ["Loopback only", "The HTTP surface binds to 127.0.0.1 with strict origin checks and one-time pairing. The extension bridge opens no port."],
  ["Per-app, revocable grants", "Each app has its own grant, scoped to connections and workspaces. Revoking it ends that app’s sessions only."],
  ["No telemetry, no relay", "History and grants stay on the machine. Requests go to the provider the selected route names, under its terms."],
];

export function Security() {
  return (
    <section className="section security" id="security" aria-labelledby="security-title">
      <div className="container security-grid">
        <div>
          <Title id="security-title" text="Local means local." />
          <p className="lead">
            The runtime records which app asked for which operation, on the user’s machine. Read the
            threat model for the assets, the trust boundaries and what is out of scope.
          </p>
          <p className="security-links">
            <a className="btn btn-ghost" href="https://github.com/thomaslittle/originlm/blob/main/docs/security/threat-model.md">Threat model</a>
            <a className="btn btn-ghost" href="https://github.com/thomaslittle/originlm/blob/main/SECURITY.md">Security policy</a>
          </p>
        </div>
        <dl className="security-list">
          {ITEMS.map(([term, text]) => (
            <div key={term}><dt>{term}</dt><dd>{text}</dd></div>
          ))}
        </dl>
      </div>
    </section>
  );
}
