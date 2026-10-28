import { EyeOff, KeyRound, Lock, Shield } from "../components/Icon";

const CARDS = [
  {
    icon: EyeOff,
    title: "No secrets exported",
    body: "Provider credentials remain with native clients or explicitly configured credential sources. Snapshots expose evidence, not credential values. Browser cookies are never read.",
  },
  {
    icon: Lock,
    title: "Loopback only",
    body: "The HTTP surface binds to 127.0.0.1 with strict origin checks and one-time pairing. The extension bridge opens no port at all.",
  },
  {
    icon: KeyRound,
    title: "Revocable per-app grants",
    body: "Every application gets its own grant, scoped to routes and workspaces. Revoke it and owned sessions stop; another app's work is untouched.",
  },
  {
    icon: Shield,
    title: "No telemetry, no relay",
    body: "History and grants stay on your machine. No telemetry or hosted relay. Authorized requests go to your selected provider; local model routes can stay local.",
  },
];

export function Security() {
  return (
    <section className="section security" id="security" aria-labelledby="security-title">
      <div className="container">
        <h2 id="security-title">A local runtime is only useful if it stays local.</h2>
        <p className="statement">
          A local permission boundary records which app asked for which operation.
          Cloud requests go to the selected provider under its terms.
        </p>

        <div className="guarantees">
          {CARDS.map(({ icon: Icon, title, body }) => (
            <article className="guarantee" key={title}>
              <h3><Icon size={20} /> {title}</h3>
              <p>{body}</p>
            </article>
          ))}
        </div>

        <div className="security-foot">
          <p>
            <strong>Read the actual guarantees.</strong> The threat model names the assets, the trust
            boundaries and the attacks that are out of scope. Provider coverage and platform verification
            are documented separately; local does not mean every operation is offline.
          </p>
          <div className="security-links">
            <a className="btn btn-ghost" href="https://github.com/thomaslittle/originlm/blob/main/SECURITY.md">Security policy</a>
            <a className="btn btn-ghost" href="https://github.com/thomaslittle/originlm/blob/main/docs/security/threat-model.md">Threat model</a>
          </div>
        </div>
      </div>
    </section>
  );
}
