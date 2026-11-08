import "./c5.css";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { highlight } from "../../lib/highlight";
import { usePageChrome, useReducedMotion } from "../shared/motion";
import {
  CONTROL_CENTER, DEMO_ROUTES, DISCOVERED, DISCOVERED_TAIL, DOCS, HOW, INSTALL, OP_LABELS, PITCH, PROBLEM,
  PROVIDERS, QUICKSTART, REPO, SAFETY, SESSION_FEATURES, STATUS, STATUS_LONG, TAGLINE, TOOLKIT, VERSION, tsCall,
  type OpState, type ProviderEntry,
} from "../shared/facts";
import { FIRST_EVENTS, eventAt, type WireEvent } from "./feed";
import { GrantSlip } from "./GrantSlip";

/* ---------- headline fitting: each line fills its column, like hand-set wood type ---------- */
function Fit({ children, className }: { children: ReactNode; className?: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    const box = el?.parentElement;
    if (!el || !box) return;
    const fit = () => {
      el.style.fontSize = "100px";
      const natural = el.scrollWidth;
      const avail = box.clientWidth;
      if (natural > 0 && avail > 0) el.style.fontSize = `${Math.floor((100 * avail) / natural * 0.995 * 10) / 10}px`;
    };
    fit();
    document.fonts?.ready.then(fit).catch(() => {});
    const ro = new ResizeObserver(fit);
    ro.observe(box);
    return () => ro.disconnect();
  }, []);
  return <span ref={ref} className={`c5-fit ${className ?? ""}`}>{children}</span>;
}

/* ---------- the wire ---------- */
function useWire(still: boolean, paused: boolean, size = 7) {
  const [events, setEvents] = useState<WireEvent[]>(() => FIRST_EVENTS(size).reverse());
  useEffect(() => {
    if (still || paused) return;
    const timer = window.setInterval(() => {
      setEvents((list) => [eventAt(list[0].id + 1), ...list].slice(0, size));
    }, 1700);
    return () => window.clearInterval(timer);
  }, [still, paused, size]);
  return events;
}

function WireLine({ event, fresh }: { event: WireEvent; fresh?: boolean }) {
  return (
    <li className={`c5-ev c5-ev-${event.tone}${fresh ? " is-fresh" : ""}`} style={{ ["--n" as string]: event.text.length + 16 }}>
      <span className="c5-ev-time">{event.time}</span>
      <span className="c5-ev-text">{event.text}</span>
    </li>
  );
}

const MARQUEE = FIRST_EVENTS(12);
function Marquee() {
  const row = MARQUEE.map((e) => (
    <span key={e.id} className="c5-mq-item"><b>{e.time}</b> {e.text} <i>SAMPLE</i></span>
  ));
  return (
    <div className="c5-mq" aria-hidden="true">
      <div className="c5-mq-track">{row}{row}</div>
    </div>
  );
}

/* ---------- small pieces ---------- */
function Copy({ value }: { value: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      className="c5-copy"
      onClick={() => {
        navigator.clipboard?.writeText(value).then(() => { setDone(true); window.setTimeout(() => setDone(false), 1400); }).catch(() => {});
      }}
      aria-label={`Copy: ${value}`}
    >
      {done ? "Copied" : "Copy"}
    </button>
  );
}

function Code({ code, label }: { code: string; label: string }) {
  return <pre className="c5-code" tabIndex={0} aria-label={label}><code>{highlight(code)}</code></pre>;
}

function Cell({ state }: { state: OpState }) {
  if (state === "yes") return <span className="c5-op c5-op-yes">Yes</span>;
  if (state === "partial") return <span className="c5-op c5-op-part">Part.</span>;
  return <span className="c5-op c5-op-none"><span aria-hidden="true">—</span><span className="c5-vh">No</span></span>;
}

const KINDS: { kind: ProviderEntry["kind"]; title: string }[] = [
  { kind: "Native client", title: "Native clients" },
  { kind: "Local runtime", title: "Local runtimes" },
  { kind: "API route", title: "API routes (configured, not discovered)" },
];

function useReveal(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    const items = document.querySelectorAll(".c5 .c5-rv");
    const io = new IntersectionObserver((entries) => {
      for (const entry of entries) if (entry.isIntersecting) { entry.target.classList.add("is-in"); io.unobserve(entry.target); }
    }, { rootMargin: "0px 0px -12% 0px" });
    items.forEach((el) => io.observe(el));
    return () => io.disconnect();
  }, [enabled]);
}

const STORY_CODE = {
  discover: `const ai = await createLocalAI();
const { connections, quotaPools } = await ai.snapshot.get();`,
  grant: `await ai.grants.request({
  connectionIds: [pick.connectionId],
  allowPaid: false,
}); // never approves itself`,
};

const DISPATCH_META = [
  { slug: "DISCOVERED", time: "10:41:07", head: "What’s already installed, found from metadata alone", code: STORY_CODE.discover },
  { slug: "GRANTED", time: "10:41:09", head: "Discovery is not permission", code: STORY_CODE.grant },
  { slug: "CALLED", time: "10:41:12", head: "Your app calls the model. The text streams back.", code: tsCall(DEMO_ROUTES[1]) },
];

/* ---------- page ---------- */
export default function Wire() {
  usePageChrome("OriginLM · The Wire", "c5-body", "#0a0a0a");
  const still = useReducedMotion();
  const [paused, setPaused] = useState(false);
  const events = useWire(still, paused);
  useReveal(!still);
  const motion = !still;

  return (
    <div className={`c5${motion ? " is-motion" : " is-still"}${paused ? " is-paused" : ""}`}>
      <a className="c5-skip" href="#main">Skip to content</a>
      <div className="c5-grain" aria-hidden="true" />

      <header className="c5-mast">
        <div className="c5-wrap">
          <div className="c5-ears">
            <p className="c5-ear">{TAGLINE}</p>
            <p className="c5-ear c5-ear-mid">Vol. 0.1 · No. alpha.25 · Local edition</p>
            <p className="c5-ear c5-ear-r">{STATUS}</p>
          </div>
          <p className="c5-plate"><a href="/c5" aria-label="OriginLM, The Wire, top of page">OriginLM <span>The Wire</span></a></p>
          <nav className="c5-nav" aria-label="Sections">
            <ul>
              <li><a href="#problem">Lead story</a></li>
              <li><a href="#dispatches">Dispatches</a></li>
              <li><a href="#sdk">SDK</a></li>
              <li><a href="#matrix">The matrix</a></li>
              <li><a href="#slip">Grant slip</a></li>
              <li><a href="#install">Install</a></li>
              <li><a href={REPO}>GitHub</a></li>
            </ul>
          </nav>
        </div>
        <div className="c5-wirebar">
          <span className="c5-wirebar-tag" aria-hidden="true">Wire · sample</span>
          <Marquee />
          {motion && (
            <button type="button" className="c5-pause" aria-pressed={paused} onClick={() => setPaused((p) => !p)}>
              {paused ? "Resume wire" : "Pause wire"}
            </button>
          )}
        </div>
      </header>

      <main id="main">
        {/* ---------------- FRONT PAGE ---------------- */}
        <section className="c5-hero" aria-labelledby="c5-h1">
          <div className="c5-wrap">
            <p className="c5-kicker c5-kicker-inv"><span>Extra</span> Filed from your machine · {VERSION}</p>
            <h1 id="c5-h1" className="c5-h1">
              <span className="c5-h1-line"><Fit>Stop bolting AI on.</Fit></span>
              <span className="c5-h1-line c5-h1-hot"><Fit>Build with the models</Fit></span>
              <span className="c5-h1-line c5-h1-hot"><Fit>you already own.</Fit></span>
            </h1>
            <div className="c5-hero-row">
              <div className="c5-deck">
                <p className="c5-deck-lead">{PITCH.headline}</p>
                <p>{PITCH.body}</p>
                <p className="c5-cta-row">
                  <a className="c5-btn c5-btn-hot" href="#install">Install the alpha</a>
                  <a className="c5-btn c5-btn-ghost" href="#dispatches">Read the dispatches</a>
                </p>
              </div>
              <div className="c5-feed">
                <div className="c5-feed-head">
                  <p><span className={`c5-dot${motion && !paused ? " is-live" : ""}`} aria-hidden="true" /> The wire</p>
                  <p className="c5-feed-sample">Sample events · example apps</p>
                </div>
                <ol className="c5-feed-list" role="log" aria-live="off" aria-label="Sample event wire: example apps calling routes through OriginLM">
                  {events.map((event, i) => <WireLine key={event.id} event={event} fresh={motion && i === 0} />)}
                </ol>
              </div>
            </div>
          </div>
        </section>

        {/* ---------------- LEAD STORY ---------------- */}
        <section className="c5-sec c5-problem" id="problem" aria-labelledby="c5-problem-h">
          <div className="c5-wrap">
            <div className="c5-dateline-row">
              <p className="c5-slug">Lead story</p>
              <p className="c5-dateline">Every machine · every app · every client</p>
            </div>
            <h2 id="c5-problem-h" className="c5-h2 c5-rv">{PROBLEM.line}</h2>
            <div className="c5-problem-grid">
              <div className="c5-cols3">
                <p className="c5-drop">
                  Your app wants one thing: to call a model. The person running it already has Claude Code, or Codex,
                  or Ollama, or three of them. Each client keeps its own {PROBLEM.burdens[0]}, needs its own{" "}
                  {PROBLEM.burdens[1]}, and speaks its own {PROBLEM.burdens[2]}.
                </p>
                <p>
                  So every app re-learns every client. The photo sorter writes a Codex parser. The notes app writes a
                  Claude Code parser. The next app writes both again, and a third for whatever ships next week.
                </p>
                <p>
                  OriginLM is the integration you write once. {PROBLEM.fix} It is a Rust toolkit with an embedded
                  library, a native helper, authenticated local transports, and TypeScript and Python SDKs.
                </p>
              </div>
              <aside className="c5-tally" aria-label="Integrations without and with OriginLM">
                <p className="c5-tally-h">The old way</p>
                <table>
                  <caption className="c5-vh">Example: three apps each integrating three clients</caption>
                  <thead><tr><th scope="col">App (example)</th><th scope="col">Codex</th><th scope="col">Claude Code</th><th scope="col">Ollama</th></tr></thead>
                  <tbody>
                    {["photo-sorter", "notes-app", "pr-reviewer"].map((app) => (
                      <tr key={app}><th scope="row">{app}</th><td>paths · parser · stream</td><td>paths · parser · stream</td><td>paths · parser · stream</td></tr>
                    ))}
                  </tbody>
                </table>
                <p className="c5-tally-h c5-tally-new">With OriginLM</p>
                <p className="c5-tally-one">Each app: <code>createLocalAI()</code>. One protocol, every route the user has.</p>
              </aside>
            </div>
          </div>
        </section>

        {/* ---------------- DISPATCHES ---------------- */}
        <section className="c5-sec c5-dispatches" id="dispatches" aria-labelledby="c5-disp-h">
          <div className="c5-wrap">
            <div className="c5-sechead">
              <h2 id="c5-disp-h" className="c5-h2 c5-rv">Dispatches</h2>
              <p className="c5-sechead-note">How it works, as three wire stories. Times are sample.</p>
            </div>
            <div className="c5-disp-grid">
              {HOW.map((step, i) => {
                const meta = DISPATCH_META[i];
                return (
                  <article key={step.id} className={`c5-disp c5-block c5-disp-${step.id}`} aria-labelledby={`c5-d-${step.id}`}>
                    <p className="c5-disp-slug"><span className="c5-num">{String(i + 1).padStart(2, "0")}</span> {meta.slug}</p>
                    <p className="c5-type c5-rv" style={{ ["--n" as string]: 34 }}>LOCALHOST, {meta.time} (SAMPLE) —</p>
                    <h3 id={`c5-d-${step.id}`} className="c5-h3">{meta.head}</h3>
                    <div className="c5-disp-body">
                      <div className="c5-disp-text">
                        <p>{step.body}</p>
                        <p className="c5-muted">{step.detail}</p>
                        {step.id === "discover" && (
                          <p className="c5-runin">
                            <b>On the list:</b> {DISCOVERED.join(", ")}, {DISCOVERED_TAIL}.
                          </p>
                        )}
                        {step.id === "call" && (
                          <>
                            <p className="c5-runin"><b>Also on the wire:</b> {SESSION_FEATURES.join(" · ")}. Only after the host grants them.</p>
                            <p className="c5-sample-reply" aria-label="Sample streamed reply">
                              <span>text_delta · sample</span> “{DEMO_ROUTES[1].reply}”
                            </p>
                          </>
                        )}
                      </div>
                      <div className="c5-disp-code">
                        <p className="c5-codecap"><code>{step.method}</code>{step.id === "call" ? ` · route: ${DEMO_ROUTES[1].name}, ${DEMO_ROUTES[1].model} (example)` : ""}</p>
                        <Code code={meta.code} label={`${step.verb} code sample`} />
                      </div>
                    </div>
                  </article>
                );
              })}
            </div>
          </div>
        </section>

        {/* ---------------- SDK / MARKETS ---------------- */}
        <section className="c5-sec c5-sdk" id="sdk" aria-labelledby="c5-sdk-h">
          <div className="c5-wrap">
            <div className="c5-sechead">
              <h2 id="c5-sdk-h" className="c5-h2 c5-rv">The SDK, by registry</h2>
              <p className="c5-sechead-note">{STATUS_LONG}</p>
            </div>
            <div className="c5-board" role="region" aria-label="Packages by registry" tabIndex={0}>
              <table className="c5-stock">
                <caption className="c5-vh">OriginLM packages, registries and install commands</caption>
                <thead>
                  <tr><th scope="col">Sym.</th><th scope="col">Package</th><th scope="col">Registry</th><th scope="col">Version</th><th scope="col">Command</th></tr>
                </thead>
                <tbody>
                  {[
                    ["npm", "originlm", "npm (alpha tag)", VERSION],
                    ["pip", "originlm", "PyPI", "0.1.0a25"],
                    ["cargo", "originlm", "crates.io", `=${VERSION}`],
                  ].map(([sym, pkg, reg, ver], i) => (
                    <tr key={sym} className="c5-block">
                      <th scope="row" className="c5-sym">{sym}</th>
                      <td><code>{pkg}</code></td>
                      <td>{reg}</td>
                      <td className="c5-ver">{ver}</td>
                      <td className="c5-cmd"><code>{INSTALL[i].cmd}</code> <Copy value={INSTALL[i].cmd} /></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className="c5-fine">npm’s <code>latest</code> tag still points at an older alpha: install with <code>@alpha</code> or an exact version.</p>

            <div className="c5-classifieds">
              {([
                ["Node", "hello.mjs", QUICKSTART.node],
                ["Python", "hello.py", QUICKSTART.python],
                ["Rust", "src/main.rs", QUICKSTART.rust],
              ] as const).map(([lang, file, code]) => (
                <article key={lang} className="c5-ad c5-block" aria-labelledby={`c5-ad-${lang}`}>
                  <h3 id={`c5-ad-${lang}`} className="c5-ad-h">Wanted: one client. <span>{lang} · {file}</span></h3>
                  <Code code={code} label={`${lang} quickstart, ${file}`} />
                </article>
              ))}
              <aside className="c5-ad c5-ad-index" aria-labelledby="c5-toolkit-h">
                <h3 id="c5-toolkit-h" className="c5-ad-h">In this edition</h3>
                <dl>
                  {TOOLKIT.map(([term, text]) => (
                    <div key={term}><dt>{term}</dt><dd>{text}</dd></div>
                  ))}
                </dl>
                <p className="c5-fine">Imports are inert: no scanning, spawning or credential reads on import. <a href={DOCS.quickstarts}>Quickstarts</a></p>
              </aside>
            </div>
          </div>
        </section>

        {/* ---------------- THE MATRIX ---------------- */}
        <section className="c5-sec c5-matrix" id="matrix" aria-labelledby="c5-matrix-h">
          <div className="c5-wrap">
            <div className="c5-sechead">
              <h2 id="c5-matrix-h" className="c5-h2 c5-rv">The matrix</h2>
              <p className="c5-sechead-note">
                Every client, every operation, as the support matrix records it. <b>Part.</b> means implemented but
                conditional, fixture-tested only, or gated by a capability the provider or account may not report.
                Never an inferred yes. <a href={DOCS.matrix}>Read the full support matrix</a>
              </p>
            </div>
            <div className="c5-matrix-scroll" role="region" aria-label="Provider support matrix (scrolls sideways)" tabIndex={0}>
              <table className="c5-mx">
                <caption className="c5-vh">Provider operations: yes, partial or no</caption>
                <thead>
                  <tr>
                    <th scope="col" className="c5-mx-name">Client</th>
                    {OP_LABELS.map((op) => <th scope="col" key={op.key} className="c5-mx-op">{op.label}</th>)}
                    <th scope="col" className="c5-mx-note">Note</th>
                  </tr>
                </thead>
                {KINDS.map(({ kind, title }) => (
                  <tbody key={kind}>
                    <tr className="c5-mx-group"><th scope="rowgroup" colSpan={OP_LABELS.length + 2}>{title}</th></tr>
                    {PROVIDERS.filter((p) => p.kind === kind).map((p) => (
                      <tr key={p.id} className="c5-mx-row">
                        <th scope="row" className="c5-mx-name"><span className="c5-glyph" aria-hidden="true">{p.glyph}</span>{p.name}</th>
                        {OP_LABELS.map((op) => <td key={op.key} className="c5-mx-op"><Cell state={p.ops[op.key]} /></td>)}
                        <td className="c5-mx-note">{p.note}</td>
                      </tr>
                    ))}
                  </tbody>
                ))}
              </table>
            </div>
          </div>
        </section>

        {/* ---------------- GRANT SLIP + NOTICE ---------------- */}
        <section className="c5-sec c5-grants" id="slip" aria-labelledby="c5-slip-h">
          <div className="c5-wrap">
            <div className="c5-sechead">
              <h2 id="c5-slip-h" className="c5-h2 c5-rv">The grant slip</h2>
              <p className="c5-sechead-note">
                Finding a client lets an app read, not run. Fill in the slip the way a host app would and print the
                grant it produces: the shared broker’s manifest entry and the call it allows.
              </p>
            </div>
            <GrantSlip still={still} />
            <aside className="c5-notice" aria-labelledby="c5-notice-h">
              <h3 id="c5-notice-h" className="c5-notice-h">Public notice: the safety model</h3>
              <ol>
                {SAFETY.map(([term, text]) => (
                  <li key={term}><b>{term}.</b> {text}</li>
                ))}
              </ol>
              <p className="c5-fine"><a href={DOCS.threat}>Threat model</a> · <a href={DOCS.security}>Security policy</a> · <a href={DOCS.privacy}>Privacy boundary</a></p>
            </aside>
          </div>
        </section>

        {/* ---------------- CONTROL CENTER (one column-inch) ---------------- */}
        <section className="c5-sec c5-also" id="control" aria-labelledby="c5-also-h">
          <div className="c5-wrap c5-also-wrap">
            <p className="c5-also-margin" aria-hidden="true">Page 9 · Briefs</p>
            <div className="c5-inch c5-block">
              <h2 id="c5-also-h" className="c5-inch-h">Also: charts, if you want them.</h2>
              <p>{CONTROL_CENTER.line} {CONTROL_CENTER.views.join(", ")}.</p>
              <p className="c5-muted">{CONTROL_CENTER.note}</p>
              <p><code>{CONTROL_CENTER.run}</code></p>
            </div>
          </div>
        </section>

        {/* ---------------- INSTALL / CTA ---------------- */}
        <section className="c5-install" id="install" aria-labelledby="c5-install-h">
          <div className="c5-wrap">
            <p className="c5-kicker c5-kicker-inv"><span>Late edition</span> {STATUS}</p>
            <h2 id="c5-install-h" className="c5-h1 c5-install-h">
              <span className="c5-h1-line"><Fit>Go to press.</Fit></span>
            </h2>
            <ul className="c5-cmds">
              {INSTALL.map((item) => (
                <li key={item.id} className="c5-cmdline">
                  <span className="c5-cmd-label">{item.label}</span>
                  <code>{item.cmd}</code>
                  <Copy value={item.cmd} />
                </li>
              ))}
            </ul>
            <p className="c5-install-note">{STATUS_LONG}</p>
            <p className="c5-cta-row">
              <a className="c5-btn c5-btn-hot" href={DOCS.quickstarts}>Read the quickstarts</a>
              <a className="c5-btn c5-btn-ghost" href={REPO}>Source on GitHub</a>
            </p>
          </div>
        </section>
      </main>

      <footer className="c5-foot">
        <div className="c5-wrap">
          <p className="c5-foot-plate">OriginLM</p>
          <div className="c5-foot-cols">
            <p>{TAGLINE} A local runtime and SDK for the AI already on your machine. No telemetry, no hosted relay: history and grants stay on your machine.</p>
            <p>Colophon: set in Archivo and Martian Mono. Every wire event on this page is sample data. Provider names are trademarks of their owners; no affiliation implied.</p>
            <ul className="c5-foot-links">
              <li><a href={REPO}>GitHub</a></li>
              <li><a href={DOCS.license}>MIT license</a></li>
              <li><a href={DOCS.matrix}>Support matrix</a></li>
              <li><a href={DOCS.broker}>Shared broker</a></li>
              <li><a href="/concepts">All five concepts</a></li>
            </ul>
          </div>
          <p className="c5-foot-line">The Wire · concept 05 of 05 · design exploration, not the published site</p>
        </div>
      </footer>
    </div>
  );
}
