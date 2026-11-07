import "./c1.css";
import { useEffect, useRef, useState, type ReactNode } from "react";
import mark from "../../assets/brand/originlm-mark-white.svg";
import { gsap, ScrollTrigger, prefersReducedMotion, useLenis, usePageChrome } from "../shared/motion";
import {
  CONTROL_CENTER, DISCOVERED, DISCOVERED_TAIL, DOCS, HOW, INSTALL, OP_LABELS, ONE_LINER, PITCH, PROBLEM,
  PROVIDERS, QUICKSTART, REPO, SAFETY, STATUS, STATUS_LONG, TAGLINE, TOOLKIT, type OpState,
} from "../shared/facts";
import { STORY_NODES } from "../../data/story";
import { highlight } from "../../lib/highlight";
import { PatchBay } from "./PatchBay";
import { PatchNotes } from "./PatchNotes";

export default function Switchboard() {
  usePageChrome("OriginLM: the switchboard for the AI you already have", "c1-body", "#0c0e0d");
  useLenis();
  const root = useRef<HTMLDivElement>(null);

  /* Scroll = plugging deeper: each row's rail jack lights as it enters, and section
     titles print in like silkscreen through a stepped wipe. */
  useEffect(() => {
    const el = root.current;
    if (!el) return;
    if (prefersReducedMotion()) { el.querySelectorAll(".c1-row").forEach((row) => row.classList.add("is-live")); return; }
    const ctx = gsap.context(() => {
      el.querySelectorAll<HTMLElement>(".c1-row").forEach((row) => {
        ScrollTrigger.create({ trigger: row, start: "top 70%", end: "bottom 30%", toggleClass: "is-live" });
        const title = row.querySelector(".c1-stencil");
        if (title) gsap.fromTo(title, { clipPath: "inset(0 100% 0 0)" }, { clipPath: "inset(0 0% 0 0)", duration: 0.7, ease: "steps(14)", scrollTrigger: { trigger: row, start: "top 78%", once: true } });
      });
    }, el);
    return () => ctx.revert();
  }, []);

  return (
    <div className="c1" ref={root}>
      <a className="c1-skip" href="#main">Skip to content</a>
      <header className="c1-top">
        <a className="c1-brand" href="#patch" aria-label="OriginLM, back to the patch bay">
          <img src={mark} alt="" width={22} height={22} />
          <span>OriginLM</span>
        </a>
        <nav aria-label="Sections">
          <ul>
            <li><a href="#how">Signal path</a></li>
            <li><a href="#board">The board</a></li>
            <li><a href="#patch-notes">Patch notes</a></li>
            <li><a href="#install">Install</a></li>
            <li><a href={REPO}>GitHub</a></li>
          </ul>
        </nav>
        <span className="c1-top-status c1-silk">{STATUS}</span>
      </header>

      <main id="main">
        <section className="c1-hero" id="patch" aria-labelledby="c1-hero-title">
          <div className="c1-hero-head">
            <p className="c1-silk c1-eyebrow"><span className="c1-led c1-led--on" aria-hidden="true" /> Patch bay · {TAGLINE}</p>
            <h1 id="c1-hero-title">Patch your app into the models you already pay for.</h1>
            <p className="c1-hero-lead">{PITCH.body}</p>
          </div>
          <PatchBay />
        </section>

        <Row id="problem" n="02" label="The problem" title="Every app you build re-integrates every AI client.">
          <div className="c1-problem">
            <div className="c1-problem-copy">
              <p className="c1-lead">{PROBLEM.fix}</p>
              <p>
                Three example apps, five clients: fifteen cables, each with its own {PROBLEM.burdens.join(", ")}.
                With one local runtime, each app patches in once.
              </p>
            </div>
            <div className="c1-problem-figs">
              <Tangle />
              <Bus />
            </div>
          </div>
        </Row>

        <Row id="how" n="03" label="Signal path" title="Discover. Grant. Call.">
          <ol className="c1-steps">
            {HOW.map((step, i) => (
              <li key={step.id} className="c1-module">
                <p className="c1-silk c1-module-n">Step 0{i + 1} · <code>{step.method}</code></p>
                <h3>{step.verb}</h3>
                <p>{step.body}</p>
                <p className="c1-module-detail">{step.detail}</p>
                <pre className="c1-snip" tabIndex={0} aria-label={`${step.verb} code`}><code>{highlight(STEP_CODE[step.id])}</code></pre>
              </li>
            ))}
          </ol>
        </Row>

        <Row id="sdk" n="04" label="SDKs" title="One protocol. Three SDKs.">
          <Sdk />
        </Row>

        <Row id="board" n="05" label="The board" title="Every jack found on your machine.">
          <Board />
        </Row>

        <Row id="patch-notes" n="06" label="Patch notes" title="Discovery is not permission.">
          <p className="c1-lead c1-notes-lead">
            Every cable on this board is a grant the host app made explicitly, scoped to connections and
            workspaces. Yank one and that app is revoked; the others keep playing.
          </p>
          <PatchNotes />
          <ul className="c1-safety">
            {SAFETY.map(([term, text]) => (
              <li key={term}><h3>{term}</h3><p>{text}</p></li>
            ))}
          </ul>
        </Row>

        <Row id="meters" n="07" label="Meters" title="And yes, the board has meters." small>
          <Meters />
        </Row>

        <section className="c1-install" id="install" aria-labelledby="c1-install-title">
          <p className="c1-silk c1-eyebrow"><span className="c1-led c1-led--on" aria-hidden="true" /> Row 08 · Install</p>
          <h2 id="c1-install-title">Patch in.</h2>
          <ul className="c1-cmds">
            {INSTALL.map((cmd) => (
              <li key={cmd.id}>
                <span className="c1-silk c1-cmd-label">{cmd.label}</span>
                <code>{cmd.cmd}</code>
                <Copy value={cmd.cmd} label={`Copy the ${cmd.label} command`} />
              </li>
            ))}
          </ul>
          <p className="c1-install-note">{STATUS_LONG}</p>
          <p className="c1-install-links">
            <a className="c1-btn c1-btn--signal" href={DOCS.quickstarts}>Read the quickstarts</a>
            <a className="c1-btn" href={REPO}>Source on GitHub</a>
          </p>
        </section>
      </main>

      <footer className="c1-foot">
        <div className="c1-foot-brand">
          <img src={mark} alt="" width={28} height={28} />
          <div>
            <p className="c1-foot-name">OriginLM</p>
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
            <li><a href="/concepts">← All concepts</a></li>
          </ul>
        </nav>
        <p className="c1-silk c1-foot-fine">Concept 01 · The Switchboard · design exploration. Provider names are trademarks of their owners.</p>
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
  sessionId: session.id, requestId, prompt,
});`,
};

function Row({ id, n, label, title, small, children }: { id: string; n: string; label: string; title: string; small?: boolean; children: ReactNode }) {
  return (
    <section className={`c1-row${small ? " c1-row--small" : ""}`} id={id} aria-labelledby={`${id}-title`}>
      <div className="c1-rail" aria-hidden="true"><span className="c1-rail-jack" /><span className="c1-rail-line" /></div>
      <div className="c1-row-body">
        <p className="c1-silk c1-eyebrow">Row {n} · {label}</p>
        <h2 id={`${id}-title`} className="c1-stencil">{title}</h2>
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
      className="c1-copy"
      aria-label={label}
      onClick={() => {
        navigator.clipboard?.writeText(value).then(() => { setDone(true); window.setTimeout(() => setDone(false), 1400); }).catch(() => {});
      }}
    >
      {done ? "Copied" : "Copy"}
    </button>
  );
}

/* --- The problem: fifteen cables vs one bus ------------------------------------- */
const APPS = ["your-app", "notes-app", "photo-sorter"];
const CLIENTS = ["Claude Code", "Codex", "Gemini CLI", "Cursor", "Ollama"];

function Tangle() {
  const ay = (i: number) => 60 + i * 70;
  const cy = (j: number) => 30 + j * 46;
  return (
    <figure className="c1-fig">
      <svg viewBox="0 0 420 260" role="img" aria-labelledby="c1-tangle-t">
        <title id="c1-tangle-t">Without a runtime: three apps each wired to five clients, fifteen integrations.</title>
        {APPS.map((_, i) => CLIENTS.map((__, j) => (
          <path key={`${i}-${j}`} className="c1-fig-wire c1-fig-wire--amber" d={`M112,${ay(i)} Q${208 + (j - 2) * 14},${(ay(i) + cy(j)) / 2 + 38 + ((i + j) % 3) * 12} 304,${cy(j)}`} />
        )))}
        {APPS.map((a, i) => (
          <g key={a}><circle cx="112" cy={ay(i)} r="5" className="c1-fig-jack" /><text x="102" y={ay(i) + 4} textAnchor="end" className="c1-fig-label">{a}</text></g>
        ))}
        {CLIENTS.map((c, j) => (
          <g key={c}><circle cx="304" cy={cy(j)} r="5" className="c1-fig-jack" /><text x="314" y={cy(j) + 4} className="c1-fig-label">{c}</text></g>
        ))}
      </svg>
      <figcaption><span className="c1-silk">Today</span> 3 × 5 = 15 integrations</figcaption>
    </figure>
  );
}

function Bus() {
  const ay = (i: number) => 60 + i * 70;
  const cy = (j: number) => 30 + j * 46;
  return (
    <figure className="c1-fig">
      <svg viewBox="0 0 420 260" role="img" aria-labelledby="c1-bus-t">
        <title id="c1-bus-t">With OriginLM: each app patches into one local runtime, which reaches every client.</title>
        {APPS.map((a, i) => <path key={a} className="c1-fig-wire c1-fig-wire--signal" d={`M112,${ay(i)} Q160,${(ay(i) + 130) / 2 + 26} 196,130`} />)}
        {CLIENTS.map((c, j) => <path key={c} className="c1-fig-wire c1-fig-wire--dim" d={`M236,130 Q280,${(130 + cy(j)) / 2 + 22} 304,${cy(j)}`} />)}
        <rect x="190" y="104" width="52" height="52" rx="8" className="c1-fig-bus" />
        <text x="216" y="182" textAnchor="middle" className="c1-fig-label c1-fig-label--signal">originlm</text>
        {APPS.map((a, i) => (
          <g key={a}><circle cx="112" cy={ay(i)} r="5" className="c1-fig-jack c1-fig-jack--on" /><text x="102" y={ay(i) + 4} textAnchor="end" className="c1-fig-label">{a}</text></g>
        ))}
        {CLIENTS.map((c, j) => (
          <g key={c}><circle cx="304" cy={cy(j)} r="5" className="c1-fig-jack" /><text x="314" y={cy(j) + 4} className="c1-fig-label">{c}</text></g>
        ))}
      </svg>
      <figcaption><span className="c1-silk">With OriginLM</span> 3 apps, one integration each. Example apps.</figcaption>
    </figure>
  );
}

/* --- SDK ---------------------------------------------------------------------- */
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
    <div className="c1-sdk">
      <div className="c1-sdk-copy">
        <p className="c1-lead">
          TypeScript, Rust and Python clients speak the same JSON-RPC protocol to the same runtime. The
          TypeScript and Python SDKs are typed protocol clients generated from the Rust contracts.
        </p>
        <p>Importing the package never scans, launches a process or reads a credential. Discovery runs when you call it.</p>
        <ul className="c1-toolkit">
          {TOOLKIT.map(([name, text]) => (
            <li key={name}><span className="c1-led c1-led--on" aria-hidden="true" /><strong>{name}</strong><span>{text}</span></li>
          ))}
        </ul>
      </div>
      <div className="c1-sdk-code">
        <div className="c1-tabs" role="tablist" aria-label="SDK language">
          {SDKS.map((s, i) => (
            <button
              key={s.id}
              ref={(el) => { refs.current[s.id] = el; }}
              type="button" role="tab" id={`c1-tab-${s.id}`}
              aria-selected={active === s.id} aria-controls="c1-sdk-panel" tabIndex={active === s.id ? 0 : -1}
              onClick={() => setActive(s.id)} onKeyDown={(e) => onKey(e, i)}
            >{s.label}</button>
          ))}
        </div>
        <figure className="c1-console c1-console--flat" role="tabpanel" id="c1-sdk-panel" aria-labelledby={`c1-tab-${current.id}`}>
          <figcaption className="c1-console-head"><span className="c1-silk">{current.file}</span><span className="c1-chip">from the quickstart</span></figcaption>
          <pre tabIndex={0} aria-label={`${current.label} quickstart`}><code>{highlight(current.code)}</code></pre>
          <p className="c1-sdk-install"><span aria-hidden="true">$ </span><code>{current.install}</code><Copy value={current.install} label={`Copy the ${current.label} install command`} /></p>
        </figure>
      </div>
    </div>
  );
}

/* --- The board: every provider as a jack with operation LEDs -------------------- */
const LED_TEXT: Record<OpState, string> = { yes: "yes", partial: "partial", none: "no" };

function Board() {
  return (
    <>
      <p className="c1-lead c1-board-lead">
        Passive discovery finds the clients already installed, from metadata only: no secrets read, no network
        calls. Each LED is one operation. Unknown and unsupported are different states; nothing is guessed.
      </p>
      <p className="c1-legend c1-silk" aria-hidden="true">
        <span><span className="c1-led c1-led--yes" /> supported</span>
        <span><span className="c1-led c1-led--partial" /> partial: conditional, fixture-tested or gated</span>
        <span><span className="c1-led c1-led--none" /> unsupported</span>
      </p>
      <ul className="c1-grid">
        {PROVIDERS.map((p) => (
          <li key={p.id} className="c1-cell">
            <div className="c1-cell-top">
              <span className="c1-cell-socket" aria-hidden="true"><span>{p.glyph}</span></span>
              <div>
                <h3>{p.name}</h3>
                <p className="c1-silk c1-cell-kind">{p.kind}</p>
              </div>
            </div>
            <ul className="c1-ops" aria-label={`${p.name} operations`}>
              {OP_LABELS.map((op) => (
                <li key={op.key} className={`c1-op c1-op--${p.ops[op.key]}`}>
                  <span className="c1-led" aria-hidden="true" />
                  <span className="c1-op-name">{op.label}</span>
                  <span className="visually-hidden">: {LED_TEXT[p.ops[op.key]]}</span>
                </li>
              ))}
            </ul>
            <p className="c1-cell-note">{p.note}</p>
          </li>
        ))}
      </ul>
      <p className="c1-board-foot">
        Discovery covers {DISCOVERED.join(", ")}, {DISCOVERED_TAIL}. Coverage is operation-specific: read
        the <a href={DOCS.matrix}>support matrix</a> before relying on a provider.
      </p>
    </>
  );
}

/* --- Meters: the control center, small and late -------------------------------- */
function Meters() {
  const ref = useRef<HTMLDivElement>(null);
  const meters = STORY_NODES.filter((n) => n.id !== "cursor");
  useEffect(() => {
    const el = ref.current;
    if (!el || prefersReducedMotion()) return;
    const ctx = gsap.context(() => {
      el.querySelectorAll<SVGGElement>(".c1-needle").forEach((needle, i) => {
        const angle = Number(needle.dataset.angle);
        gsap.fromTo(needle, { rotation: -60, svgOrigin: "60 66" }, { rotation: angle, svgOrigin: "60 66", duration: 1.4, delay: i * 0.12, ease: "elastic.out(1, 0.45)", scrollTrigger: { trigger: el, start: "top 80%", once: true } });
      });
    }, el);
    return () => ctx.revert();
  }, []);
  return (
    <div className="c1-meters" ref={ref}>
      <div className="c1-meters-copy">
        <p>{CONTROL_CENTER.line} {CONTROL_CENTER.views.join(", ")}.</p>
        <p className="c1-meters-note">{CONTROL_CENTER.note}</p>
        <p className="c1-sdk-install"><span aria-hidden="true">$ </span><code>{CONTROL_CENTER.run}</code><Copy value={CONTROL_CENTER.run} label="Copy the desktop app command" /></p>
      </div>
      <ul className="c1-vu" aria-label="Sample quota windows, as the control center shows them">
        {meters.map((m) => {
          const known = m.reading.state === "known";
          const used = m.reading.state === "known" ? m.reading.used : 0;
          const angle = known ? -60 + (used / 100) * 120 : -60;
          return (
            <li key={m.id} className={`c1-vu-cell${known ? "" : " is-unknown"}`}>
              <svg viewBox="0 0 120 72" aria-hidden="true">
                <path d="M14,64 A50,50 0 0 1 106,64" className="c1-vu-arc" />
                <path d="M86,24 A50,50 0 0 1 106,64" className="c1-vu-red" />
                {[-60, -30, 0, 30, 60].map((a) => (
                  <line key={a} x1="60" y1="18" x2="60" y2="24" transform={`rotate(${a} 60 66)`} className="c1-vu-tick" />
                ))}
                <g className="c1-needle" data-angle={angle} transform={`rotate(${angle} 60 66)`}>
                  <line x1="60" y1="66" x2="60" y2="22" />
                </g>
                <circle cx="60" cy="66" r="4" className="c1-vu-hub" />
              </svg>
              <p className="c1-vu-name">{m.name}</p>
              <p className="c1-silk c1-vu-read">
                {m.reading.state === "known" ? `${m.window} · ${used}% used` : `${m.reading.state}: ${m.reading.reason}`}
              </p>
            </li>
          );
        })}
      </ul>
      <p className="c1-silk c1-vu-sample">Sample readings · example data</p>
    </div>
  );
}
