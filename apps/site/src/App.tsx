/* OriginLM landing page:
   Hero (3-stage Agent -> Runtime & SDK -> App Output flow) -> 02 Problem & Architecture
   -> 03 How it works -> 04 SDKs -> 05 Control center -> 06 Install. */
import { useRef, useState, type ReactNode } from "react";
import mark from "./assets/brand/originlm-mark-white.svg";
import {
  CONTROL_CENTER, DOCS, HOW, INSTALL, ONE_LINER, PITCH, PROBLEM,
  QUICKSTART, REPO, SESSION_FEATURES, STATUS, STATUS_LONG, TAGLINE, TOOLKIT,
} from "./data/facts";
import { highlight } from "./lib/highlight";
import { RouteWorkbench } from "./RouteWorkbench";

type AllowanceCard = {
  id: string;
  name: string;
  sub: string;
  status: string;
  statusTone: "signal" | "amber" | "neutral";
  windowLabel: string;
  valueLabel: string;
  stale?: boolean;
  percentLeft: number | null;
  emptyBar?: boolean;
  footerLeft: string;
  footerRight?: string;
};

/* Modelled directly on the real OriginLM desktop Control Center Overview
   (public/screens/desktop-overview.webp): plain numbers, flat 20-cell segmented bar
   only when a percentage window is reported, and honest unknown/unbounded states. */
const ALLOWANCES: AllowanceCard[] = [
  {
    id: "codex-personal",
    name: "Codex",
    sub: "Personal workspace · Included plan",
    status: "Signed in",
    statusTone: "signal",
    windowLabel: "Session window",
    valueLabel: "75% left",
    percentLeft: 75,
    footerLeft: "Resets in 59m",
    footerRight: "Example data",
  },
  {
    id: "codex-work",
    name: "Codex",
    sub: "Work workspace · Included plan",
    status: "Cached sign-in",
    statusTone: "amber",
    windowLabel: "Weekly window",
    valueLabel: "30% left",
    stale: true,
    percentLeft: 30,
    footerLeft: "Reset time not reported",
  },
  {
    id: "antigravity",
    name: "Antigravity",
    sub: "Google workspace · Gemini 4 Neon",
    status: "Signed in",
    statusTone: "signal",
    windowLabel: "Count without a published total",
    valueLabel: "17 requests used",
    percentLeft: null,
    emptyBar: true,
    footerLeft: "No published total",
  },
  {
    id: "ollama",
    name: "Ollama",
    sub: "On this device · Local runtime",
    status: "Local",
    statusTone: "neutral",
    windowLabel: "Allowance window",
    valueLabel: "Not reported",
    percentLeft: null,
    emptyBar: true,
    footerLeft: "Local route — no allowance window",
  },
];

const CELLS = 20;

export default function App() {
  return (
    <div className="sb">
      <a className="sb-skip" href="#main">Skip to content</a>

      <header className="sb-top">
        <div className="sb-top-in">
          <a className="sb-brand" href="#top" aria-label="OriginLM, back to top">
            <img src={mark} alt="" width={20} height={20} />
            <span>OriginLM</span>
          </a>
          <span className="sb-top-sep" aria-hidden="true" />
          <nav aria-label="Sections">
            <ul>
              <li><a href="#problem">Architecture</a></li>
              <li><a href="#how">How it works</a></li>
              <li><a href="#sdk">SDKs</a></li>
              <li><a href="#control">Control center</a></li>
              <li><a href="#install">Install</a></li>
              <li><a href={REPO}>GitHub</a></li>
            </ul>
          </nav>
          <span className="sb-top-status sb-label">{STATUS}</span>
        </div>
      </header>

      <main id="main">
        {/* ---------------- HERO ---------------- */}
        <section className="sb-hero" id="top" aria-labelledby="sb-hero-title">
          <div className="sb-wrap">
            <div className="sb-hero-head">
              <p className="sb-label">Local runtime · {TAGLINE}</p>
              <h1 id="sb-hero-title">{PITCH.headline}</h1>
              <div className="sb-hero-right">
                <p className="sb-hero-lead">{PITCH.body}</p>
                <div className="sb-hero-act">
                  <div className="sb-cmd-bar">
                    <span className="sb-ps1" aria-hidden="true">$</span>
                    <code>{INSTALL[0].cmd}</code>
                    <Copy value={INSTALL[0].cmd} label="Copy the npm install command" />
                  </div>
                  <div className="sb-hero-links">
                    <a className="sb-btn sb-btn--primary" href={DOCS.quickstarts}>Read the quickstarts</a>
                    <a className="sb-btn" href={REPO}>Source on GitHub</a>
                    <span className="sb-label">TS · Python · Rust · local only</span>
                  </div>
                </div>
              </div>
            </div>
            <RouteWorkbench />
          </div>
        </section>

        {/* ---------------- 02: PROBLEM + LOCAL MACHINE BOUNDARY ---------------- */}
        <Section id="problem" n="02" label="Problem & architecture" title="Every app you build re-integrates every AI client.">
          <div className="sb-problem">
            <div className="sb-problem-copy">
              <p className="sb-lead">{PROBLEM.fix}</p>
              <p>
                Three example apps, five clients: fifteen integrations, each with its own {PROBLEM.burdens.join(", ")}.
                With one local runtime, each app integrates once.
              </p>
            </div>
            <div className="sb-problem-figs">
              <ManyToMany />
              <SingleRuntime />
            </div>
          </div>
          <LocalBoundary />
        </Section>

        {/* ---------------- 03: HOW IT WORKS ---------------- */}
        <Section id="how" n="03" label="How it works" title="Discover. Grant. Call.">
          <ol className="sb-steps">
            {HOW.map((step, i) => (
              <li key={step.id} className="sb-module">
                <p className="sb-label sb-module-n">
                  <span>Step 0{i + 1}</span>
                  <code>{step.method}</code>
                </p>
                <h3>{step.verb}</h3>
                <p>{step.body}</p>
                <p className="sb-module-detail">{step.detail}</p>
                <pre className="sb-snip" tabIndex={0} aria-label={`${step.verb} code`}><code>{highlight(STEP_CODE[step.id])}</code></pre>
              </li>
            ))}
          </ol>
          <div className="sb-session-bar">
            <span className="sb-label">Granted sessions carry:</span>
            <ul aria-label="Session capabilities">
              {SESSION_FEATURES.map((f) => <li key={f}>{f}</li>)}
            </ul>
          </div>
        </Section>

        {/* ---------------- 04: SDKS ---------------- */}
        <Section id="sdk" n="04" label="SDKs" title="One protocol. Three SDKs.">
          <Sdk />
        </Section>

        {/* ---------------- 05: CONTROL CENTER (QUIET REAL-UI READOUTS) ---------------- */}
        <Section id="control" n="05" label="Control center" title="And yes, it tracks allowances too.">
          <ControlCenterSection />
        </Section>

        {/* ---------------- 06: INSTALL ---------------- */}
        <Section id="install" n="06" label="Install" title="Install OriginLM.">
          <div className="sb-install-box">
            <ul className="sb-cmds">
              {INSTALL.map((cmd) => (
                <li key={cmd.id}>
                  <span className="sb-label">{cmd.label}</span>
                  <code>{cmd.cmd}</code>
                  <Copy value={cmd.cmd} label={`Copy the ${cmd.label} command`} />
                </li>
              ))}
            </ul>
            <p className="sb-install-note">{STATUS_LONG}</p>
            <div className="sb-install-links">
              <a className="sb-btn sb-btn--primary" href={DOCS.quickstarts}>Read the quickstarts</a>
              <a className="sb-btn" href={REPO}>Source on GitHub</a>
            </div>
          </div>
        </Section>
      </main>

      <footer className="sb-foot">
        <div className="sb-foot-in">
          <div className="sb-foot-brand">
            <img src={mark} alt="" width={24} height={24} />
            <div>
              <p className="sb-foot-name">OriginLM</p>
              <p>{ONE_LINER}</p>
            </div>
          </div>
          <nav aria-label="Footer">
            <ul>
              <li><a href={DOCS.quickstarts}>Quickstarts</a></li>
              <li><a href={DOCS.broker}>Shared broker</a></li>
              <li><a href={DOCS.matrix}>Support matrix</a></li>
              <li><a href={DOCS.threat}>Threat model</a></li>
              <li><a href={DOCS.privacy}>Privacy boundary</a></li>
              <li><a href={DOCS.license}>MIT license</a></li>
            </ul>
          </nav>
          <p className="sb-foot-fine">OriginLM · MIT-licensed, local-first. No telemetry on this page. Provider names are trademarks of their owners.</p>
        </div>
      </footer>
    </div>
  );
}

const STEP_CODE: Record<(typeof HOW)[number]["id"], string> = {
  discover: `const { connections, quotaPools } =
  await ai.snapshot.get();`,
  grant: `await ai.grants.request({
  connectionIds: [pick.connectionId],
  allowPaid: false,
}); // never approves itself`,
  call: `await ai.turns.start({
  sessionId: session.id,
  requestId,
  prompt,
});`,
};

function Section({ id, n, label, title, children }: { id: string; n: string; label: string; title: string; children: ReactNode }) {
  return (
    <section className="sb-sec" id={id} aria-labelledby={`${id}-title`}>
      <div className="sb-sec-in">
        <div className="sb-sec-head">
          <p className="sb-label">{n} · {label}</p>
          <h2 id={`${id}-title`}>{title}</h2>
        </div>
        {children}
      </div>
    </section>
  );
}

function Copy({ value, label }: { value: string; label: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      className="sb-btn sb-btn--sm"
      aria-label={label}
      onClick={() => {
        navigator.clipboard?.writeText(value).then(() => { setDone(true); window.setTimeout(() => setDone(false), 1400); }).catch(() => {});
      }}
    >
      {done ? "Copied" : "Copy"}
    </button>
  );
}

/* --- Problem diagrams: clean orthogonal architecture comparison ---------------- */
const APPS = ["your-app", "notes-app", "photo-sorter"];
const CLIENTS = ["Claude Code", "Codex", "Antigravity", "Cursor", "Ollama"];

function ManyToMany() {
  const ay = (i: number) => 56 + i * 74;
  const cy = (j: number) => 34 + j * 48;
  return (
    <figure className="sb-fig">
      <svg viewBox="0 0 420 260" role="img" aria-labelledby="sb-m2m-t">
        <title id="sb-m2m-t">Without OriginLM: three apps each integrating five clients, fifteen integrations.</title>
        {APPS.map((_, i) => CLIENTS.map((__, j) => (
          <line key={`${i}-${j}`} className="sb-fig-line sb-fig-line--amber" x1="116" y1={ay(i)} x2="298" y2={cy(j)} />
        )))}
        {APPS.map((a, i) => (
          <g key={a}>
            <rect x="12" y={ay(i) - 13} width="104" height="26" rx="3" className="sb-fig-box" />
            <text x="20" y={ay(i)} dominantBaseline="central" className="sb-fig-label">{a}</text>
          </g>
        ))}
        {CLIENTS.map((c, j) => (
          <g key={c}>
            <rect x="298" y={cy(j) - 13} width="110" height="26" rx="3" className="sb-fig-box" />
            <text x="306" y={cy(j)} dominantBaseline="central" className="sb-fig-label">{c}</text>
          </g>
        ))}
      </svg>
      <figcaption><span className="sb-label">Without OriginLM</span> 3 × 5 = 15 integrations</figcaption>
    </figure>
  );
}

function SingleRuntime() {
  /* Every card center is symmetric around y = 130 (hub center):
     Apps: 56, 130, 204 | Hub: y=106..154 (center 130) | Clients: 34, 82, 130, 178, 226.
     Single non-overlapping bus paths ensure uniform stroke weight and clean 90-degree junctions. */
  const ay = (i: number) => 56 + i * 74;
  const cy = (j: number) => 34 + j * 48;
  return (
    <figure className="sb-fig">
      <svg viewBox="0 0 420 260" role="img" aria-labelledby="sb-one-t">
        <title id="sb-one-t">With OriginLM: three apps each integrate once with OriginLM, which connects to all clients.</title>
        <path
          className="sb-fig-line sb-fig-line--signal"
          d={`M116,${ay(0)} H144 V${ay(2)} H116 M116,${ay(1)} H164`}
        />
        <path
          className="sb-fig-line sb-fig-line--dim"
          d={`M298,${cy(0)} H276 V${cy(4)} H298 M256,130 H298 M276,${cy(1)} H298 M276,${cy(3)} H298`}
        />
        <rect x="164" y="106" width="92" height="48" rx="4" className="sb-fig-hub" />
        <text x="210" y="124" textAnchor="middle" dominantBaseline="central" className="sb-fig-label sb-fig-label--ink">OriginLM</text>
        <text x="210" y="139" textAnchor="middle" dominantBaseline="central" className="sb-fig-sub">local runtime</text>
        {APPS.map((a, i) => (
          <g key={a}>
            <rect x="12" y={ay(i) - 13} width="104" height="26" rx="3" className="sb-fig-box sb-fig-box--on" />
            <text x="20" y={ay(i)} dominantBaseline="central" className="sb-fig-label">{a}</text>
          </g>
        ))}
        {CLIENTS.map((c, j) => (
          <g key={c}>
            <rect x="298" y={cy(j) - 13} width="110" height="26" rx="3" className="sb-fig-box" />
            <text x="306" y={cy(j)} dominantBaseline="central" className="sb-fig-label">{c}</text>
          </g>
        ))}
      </svg>
      <figcaption><span className="sb-label">With OriginLM</span> 3 apps, one integration each. Example apps.</figcaption>
    </figure>
  );
}

/* --- Local Machine Boundary ---------------------------------------------------- */
const ARCH_LOCAL = [
  { n: "01", title: "Your app", body: "Electron, Tauri, GPUI, Node, Python, or a browser extension page.", chips: ["user clicks Connect"] },
  { n: "02", title: "originlm SDK", body: "npm, crates.io and PyPI. Imports are inert until you call it.", chips: ["shared broker", "stdio helper", "loopback HTTP"] },
  { n: "03", title: "OriginLM runtime", body: "Discovery, evidence, grants, sessions and events. Runs locally.", chips: ["grants", "sessions", "events"] },
  { n: "04", title: "Installed clients", body: "The tools the user already signed in to, driven via their own interfaces.", chips: ["Claude Code", "Codex", "Antigravity", "Ollama"] },
];

function LocalBoundary() {
  return (
    <div className="sb-arch" aria-labelledby="sb-arch-title">
      <div className="sb-arch-head">
        <p className="sb-label">One machine · No relay</p>
        <h3 id="sb-arch-title">Nothing passes through an OriginLM server, because there isn’t one.</h3>
        <p className="sb-arch-sub">
          Your code talks to the SDK. The SDK talks to a runtime on the same machine, which drives the clients the user installed.
        </p>
      </div>
      <div className="sb-arch-grid" role="list" aria-label="Request path across the local machine boundary">
        {ARCH_LOCAL.map((node) => (
          <div key={node.n} className="sb-arch-node" role="listitem">
            <span className="sb-label">{node.n} · local</span>
            <h4>{node.title}</h4>
            <p>{node.body}</p>
            <ul className="sb-arch-chips" aria-label={`${node.title} details`}>
              {node.chips.map((c) => <li key={c}>{c}</li>)}
            </ul>
          </div>
        ))}
        <div className="sb-arch-divider" aria-hidden="true" title="Machine boundary" />
        <div className="sb-arch-node sb-arch-node--out" role="listitem">
          <span className="sb-label">05 · outside (unless local)</span>
          <h4>Provider</h4>
          <p>The route’s provider, under its own terms. Local model routes stay on the device.</p>
          <ul className="sb-arch-chips" aria-label="Provider route types">
            <li>subscription</li>
            <li>metered</li>
            <li>local</li>
          </ul>
        </div>
      </div>
      <div className="sb-arch-foot">
        <span><span className="sb-dot sb-dot--signal" aria-hidden="true" /> Steps 01–04 stay on the user’s machine</span>
        <span><span className="sb-dot sb-dot--amber" aria-hidden="true" /> Dashed line: machine boundary — no OriginLM relay or telemetry</span>
      </div>
    </div>
  );
}

/* --- SDK: unified tabbed console component ------------------------------------- */
const SDKS = [
  { id: "ts", label: "TypeScript", file: "hello.mjs", code: QUICKSTART.node, install: INSTALL[0].cmd },
  { id: "py", label: "Python", file: "hello.py", code: QUICKSTART.python, install: INSTALL[1].cmd },
  { id: "rs", label: "Rust", file: "src/main.rs", code: QUICKSTART.rust, install: INSTALL[2].cmd },
] as const;

function Sdk() {
  const [active, setActive] = useState<string>(SDKS[0].id);
  const refs = useRef<Record<string, HTMLButtonElement | null>>({});
  const current = SDKS.find((s) => s.id === active) ?? SDKS[0];

  const onKey = (event: React.KeyboardEvent, index: number) => {
    if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const last = SDKS.length - 1;
    const next = event.key === "Home" ? 0 : event.key === "End" ? last : event.key === "ArrowRight" ? (index + 1) % SDKS.length : (index + last) % SDKS.length;
    setActive(SDKS[next].id);
    refs.current[SDKS[next].id]?.focus();
  };

  return (
    <div className="sb-sdk">
      <div className="sb-sdk-copy">
        <p className="sb-lead">
          TypeScript, Rust and Python clients speak the same JSON-RPC protocol to the same runtime. The
          TypeScript and Python SDKs are typed protocol clients generated from the Rust contracts.
        </p>
        <p>Importing the package never scans, launches a process or reads a credential. Discovery runs when you call it.</p>
        <ul className="sb-toolkit">
          {TOOLKIT.map(([name, text]) => (
            <li key={name}><strong>{name}</strong><span>{text}</span></li>
          ))}
        </ul>
      </div>

      <div className="sb-sdk-panel">
        <div className="sb-sdk-header">
          <div className="sb-tablist" role="tablist" aria-label="SDK language">
            {SDKS.map((s, i) => (
              <button
                key={s.id}
                ref={(el) => { refs.current[s.id] = el; }}
                type="button" role="tab" id={`sb-tab-${s.id}`} className="sb-tab"
                aria-selected={active === s.id} aria-controls="sb-sdk-panel" tabIndex={active === s.id ? 0 : -1}
                onClick={() => setActive(s.id)} onKeyDown={(e) => onKey(e, i)}
              >{s.label}</button>
            ))}
          </div>
          <div className="sb-sdk-meta">
            <span className="sb-label">{current.file}</span>
            <Copy value={current.code} label={`Copy the ${current.label} quickstart code`} />
          </div>
        </div>

        <div className="sb-sdk-body" role="tabpanel" id="sb-sdk-panel" aria-labelledby={`sb-tab-${current.id}`}>
          <pre tabIndex={0} aria-label={`${current.label} quickstart`}><code>{highlight(current.code)}</code></pre>
        </div>

        <p className="sb-sdk-install">
          <span aria-hidden="true">$ </span>
          <code>{current.install}</code>
          <Copy value={current.install} label={`Copy the ${current.label} install command`} />
        </p>
      </div>
    </div>
  );
}

/* --- Control Center: quiet allowance readouts matching the real desktop app ---- */
function ControlCenterSection() {
  const [showShot, setShowShot] = useState(false);

  return (
    <div className="sb-cc">
      <div className="sb-cc-top">
        <div className="sb-cc-copy">
          <p>{CONTROL_CENTER.line} {CONTROL_CENTER.views.join(", ")}.</p>
          <p className="sb-cc-note">{CONTROL_CENTER.note}</p>
        </div>
        <div className="sb-cc-actions">
          <div className="sb-cmd-bar">
            <span className="sb-ps1" aria-hidden="true">$</span>
            <code>{CONTROL_CENTER.run}</code>
            <Copy value={CONTROL_CENTER.run} label="Copy the desktop app command" />
          </div>
          <div>
            <button type="button" className="sb-btn sb-btn--sm" aria-expanded={showShot} onClick={() => setShowShot((s) => !s)}>
              {showShot ? "Hide desktop app screenshot" : "Preview desktop app screenshot"}
            </button>
          </div>
        </div>
      </div>

      <div className="sb-allow-head">
        <span className="sb-label">Allowances · percent left in each reported window</span>
        <span className="sb-label">Example data · synthetic</span>
      </div>

      <ul className="sb-allow-grid" aria-label="Sample allowance readouts matching the OriginLM desktop app">
        {ALLOWANCES.map((card) => {
          const filled = card.percentLeft !== null ? Math.round((card.percentLeft / 100) * CELLS) : 0;
          const badgeClass = card.statusTone === "signal" ? "sb-tag sb-tag--signal" : card.statusTone === "amber" ? "sb-tag sb-tag--amber" : "sb-tag";
          return (
            <li key={card.id} className="sb-allow-card">
              <div className="sb-allow-top">
                <div className="sb-allow-who">
                  <span className="sb-allow-name">{card.name}</span>
                  <span className="sb-allow-sub">{card.sub}</span>
                </div>
                <span className={badgeClass}>{card.status}</span>
              </div>

              <div className="sb-allow-metric">
                <span className="sb-allow-win">{card.windowLabel}</span>
                <span className={`sb-allow-val${card.stale ? " is-stale" : card.percentLeft === null && card.valueLabel === "Not reported" ? " is-muted" : ""}`}>
                  {card.stale && <span className="sb-tag sb-tag--amber">Stale</span>}
                  {card.valueLabel}
                </span>
              </div>

              <span
                className={`sb-segbar${card.stale ? " is-stale" : ""}${card.emptyBar ? " is-empty" : ""}`}
                role="img"
                aria-label={card.percentLeft !== null ? `${card.percentLeft}% left` : card.valueLabel}
              >
                {Array.from({ length: CELLS }, (_, i) => (
                  <i key={i} className={i < filled ? "is-on" : undefined} />
                ))}
              </span>

              <div className="sb-allow-foot">
                <span>{card.footerLeft}</span>
                {card.footerRight && <span>{card.footerRight}</span>}
              </div>
            </li>
          );
        })}
      </ul>

      {showShot && (
        <figure className="sb-shot-box">
          <img
            src="/screens/desktop-overview.webp"
            srcSet="/screens/desktop-overview-800.webp 800w, /screens/desktop-overview.webp 1440w"
            sizes="(max-width: 760px) 100vw, 1080px"
            alt="OriginLM desktop control center in example mode showing per-account allowance windows and unknown readings."
            width={1440}
            height={920}
            loading="lazy"
          />
          <figcaption>
            <span>Control center overview · example mode (synthetic data, no accounts inspected).</span>
            <a href="/screens/desktop-overview.webp" target="_blank" rel="noreferrer">Open full size ↗</a>
          </figcaption>
        </figure>
      )}
    </div>
  );
}
