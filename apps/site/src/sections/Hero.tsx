import { ArrowRight, Github } from "../components/Icon";
import { CopyButton } from "../components/CopyButton";
import { Words } from "../components/Title";

const INSTALL = "npm install originlm@alpha";
const HEADLINE = "Ship AI features on your users’ own subscriptions.";

/* First paint is this HTML on dark ground. The eye arrives a moment later on the fixed
   stage behind it; without motion or WebGL, .hero-still shows a still of the same scene. */
export function Hero() {
  return (
    <header className="hero" id="top">
      <div className="hero-still" aria-hidden="true" />
      <div className="container hero-grid">
        <p className="hero-kicker">Local runtime and SDK for TypeScript, Rust and Python</p>
        <h1 className="hero-title"><Words text={HEADLINE} /></h1>
        <div className="hero-foot">
          <p className="hero-lead">
            OriginLM finds the Claude Code, Codex, Cursor, Gemini CLI and Ollama installs already on
            a machine. Your app gets one typed API to see what they offer and run turns the user
            approves. No API keys to ship, no relay in between.
          </p>
          <div className="hero-act">
            <div className="cmd">
              <span className="prompt" aria-hidden="true">$</span>
              <code>{INSTALL}</code>
              <CopyButton value={INSTALL} label="Copy" />
            </div>
            <div className="actions">
              <a className="btn btn-primary" data-magnetic href="#sdk">Read the quick start <ArrowRight size={16} /></a>
              <a className="btn btn-ghost" data-magnetic href="https://github.com/thomaslittle/originlm"><Github size={16} /> Source</a>
            </div>
            <p className="hero-note">MIT-licensed alpha. Local only, no telemetry, no OriginLM account.</p>
          </div>
        </div>
      </div>
      <a className="hero-scroll" href="#story" aria-label="Follow one request, step by step">
        <span aria-hidden="true">Follow one request</span>
      </a>
    </header>
  );
}
