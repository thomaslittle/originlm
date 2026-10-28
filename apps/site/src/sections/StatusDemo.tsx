import { useState } from "react";
import { Activity, Bell, CheckCircle, Gauge, Terminal } from "../components/Icon";


const SURFACES = [
  { icon: Activity, title: "SDK badge", body: "The Rust facade exposes mode, providers and model counts." },
  { icon: Gauge, title: "Desktop mini-view", body: "Ctrl+Shift+M opens allowances and reset times inside the app." },
  { icon: Bell, title: "Notifications", body: "The runtime records issues; OS notification delivery is not qualified." },
  { icon: Terminal, title: "CLI", body: "Inspect the runtime through the originlm-control terminal app." },
];

export function StatusDemo() {
  const [connected, setConnected] = useState(true);

  return (
    <div className="container status" aria-labelledby="status-title" role="group">
      <div className="head-split">
        <h2 id="status-title">Your AI status, close at hand.</h2>
        <p className="lead">
          Apps can show whether local AI is available without a modal, a permission wall or a settings
          screen. The user decides what runs through explicit grants. The desktop mini-view stays inside the app; this build has no OS tray integration.
        </p>
      </div>

      <div className="status-layout">
        <div className="status-demo">
          <h3>Try the badge</h3>
          <p>
            This interactive example illustrates a consumer status badge. The fallback state links to
            the installer instead of failing silently.
          </p>
          <div className="segmented" role="group" aria-label="Demo connection state">
            <button type="button" aria-pressed={connected} onClick={() => setConnected(true)}>Connected</button>
            <button type="button" aria-pressed={!connected} onClick={() => setConnected(false)}>Not detected</button>
          </div>

          <div className="badge-stage">
            {connected ? (
              <p className="connected-badge" data-state="connected" role="status">
                <CheckCircle size={16} />
                <strong>Local AI connected</strong>
                <span>Claude, Codex, Ollama</span>
              </p>
            ) : (
              <p className="connected-badge" data-state="disconnected" role="status">
                <strong>Not detected</strong>
                <a href="#install">Install OriginLM</a>
              </p>
            )}
          </div>

          <dl className="surfaces">
            {SURFACES.map(({ icon: Icon, title, body }) => (
              <div className="surface" key={title}>
                <dt><Icon size={17} /> {title}</dt>
                <dd>{body}</dd>
              </div>
            ))}
          </dl>
        </div>

        <figure className="mini-view-figure" aria-label="Desktop in-app allowance mini-view" data-slot="mini-view">
          <img src="/screens/desktop-mini-view.webp" alt="OriginLM example dashboard with the in-app Allowances panel open: known quota values and an explicitly unknown allowance." width={1440} height={920} loading="lazy" />
          <figcaption>Actual desktop app, example data. Press Ctrl+Shift+M for the mini-view. No OS tray is provided.</figcaption>
        </figure>
      </div>
    </div>
  );
}
