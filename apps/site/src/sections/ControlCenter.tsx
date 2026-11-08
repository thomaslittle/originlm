import { UsagePanel } from "../components/UsagePanel";
import { Title } from "../components/Title";

const POINTS: [string, string][] = [
  ["Every window, every account", "Each account gets its own card with a meter per allowance window, percent left and a reset countdown. Two logins never blur into one bar."],
  ["Unknown stays unknown", "A client that reports nothing shows “not reported”, with a button to check again. Stale readings are marked stale."],
  ["Apps and revocation", "Connected apps, what they may do, and a revoke that ends their sessions without touching anyone else’s."],
  ["A glance while you work", "Ctrl+Shift+M opens a compact allowance view inside the app. There is no OS tray icon yet."],
];

export function ControlCenter() {
  return (
    <section className="section cc" id="usage" aria-labelledby="cc-title">
      <div className="container">
        <div className="cc-head">
          <Title id="cc-title" text="The user sees what your app sees." />
          <p className="lead">
            The control center is a native desktop app over the same runtime. It shows each account’s
            allowance windows and when they reset, the routes your app could use, and which apps are
            connected.
          </p>
        </div>

        <div className="cc-grid">
          <dl className="cc-points">
            {POINTS.map(([term, text]) => (
              <div key={term}><dt>{term}</dt><dd>{text}</dd></div>
            ))}
          </dl>
          <UsagePanel />
        </div>

        <figure className="cc-shot">
          <a className="cc-shot-link" href="/screens/desktop-overview.webp" target="_blank" rel="noreferrer" aria-label="Open the control center overview at full size">
            <img
              src="/screens/desktop-overview.webp"
              srcSet="/screens/desktop-overview-800.webp 800w, /screens/desktop-overview.webp 1440w"
              sizes="(max-width: 60rem) 100vw, min(1312px, 100vw)"
              alt="The OriginLM control center overview in example mode: a runtime summary, eight account cards with per-window allowance meters, percent left and reset times, unknown readings marked “Not reported”, and needs-attention and developer panels."
              width={1440}
              height={920}
              loading="lazy"
              decoding="async"
            />
          </a>
          <figcaption>Control center, overview. Example mode: synthetic data, no accounts read. Open the image at full size to inspect the window; the readable meter list is above.</figcaption>
        </figure>
      </div>
    </section>
  );
}
