import { ArrowRight } from "../components/Icon";

export function FinalCta() {
  return (
    <div className="final-cta">
      <h2 id="cta-title">Your AI is already on the machine. Use it everywhere.</h2>
      <div className="final-cta-side">
        <p>
          Install OriginLM for the dashboard, and every app you build on it already has a backend.
          No API keys, no relay, no surprises.
        </p>
        <div className="hero-actions">
          <a className="btn btn-primary" href="#install">Get OriginLM <ArrowRight size={17} /></a>
          <a className="btn btn-ghost" href="#integrations">Integration guides</a>
        </div>
        <p className="hero-note">Free and MIT-licensed. No account. No telemetry.</p>
      </div>
    </div>
  );
}
