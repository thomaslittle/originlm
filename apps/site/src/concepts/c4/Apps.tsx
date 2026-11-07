import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { highlight } from "../../lib/highlight";
import { CODE, STORY_NODES } from "../../data/story";
import {
  CONTROL_CENTER, DEMO_ROUTES, DISCOVERED, DISCOVERED_TAIL, DOCS, GRANT_SCOPES, HOW, INSTALL, OP_LABELS,
  PROBLEM, PROVIDERS, QUICKSTART, SAFETY, SESSION_FEATURES, STATUS_LONG, TOOLKIT, grantManifest, type OpState,
} from "../shared/facts";
import { Win } from "./Window";
import { Lock } from "./icons";
import { useWM } from "./wm";

/* ---------- small parts ---------- */

function SecHead({ path, id, title, children }: { path: string; id: string; title: ReactNode; children?: ReactNode }) {
  return (
    <div className="c4-sec-head">
      <p className="c4-path">{path}</p>
      <h2 id={id} className="c4-h2">{title}</h2>
      {children && <div className="c4-lead">{children}</div>}
    </div>
  );
}

function Code({ code, label, wrap }: { code: string; label: string; wrap?: boolean }) {
  return (
    <pre className={`c4-code c4-code-block ${wrap ? "is-wrap" : ""}`} tabIndex={0} aria-label={label} data-lenis-prevent>
      <code>{highlight(code)}</code>
    </pre>
  );
}

function Copy({ value }: { value: string }) {
  const [done, setDone] = useState(false);
  const timer = useRef(0);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return (
    <button
      type="button"
      className="c4-btn c4-btn-small"
      onClick={async () => {
        try { await navigator.clipboard.writeText(value); } catch { /* clipboard blocked; the command is visible */ }
        setDone(true);
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => setDone(false), 1600);
      }}
    >
      {done ? "Copied" : "Copy"}<span className="visually-hidden"> {value}</span>
    </button>
  );
}

/** Tabs with roving focus. */
function Tabs<T extends string>({ items, value, onChange, label }: { items: { id: T; label: string }[]; value: T; onChange: (id: T) => void; label: string }) {
  const refs = useRef<Record<string, HTMLButtonElement | null>>({});
  const base = useId();
  const onKey = (event: KeyboardEvent, index: number) => {
    if (!["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const last = items.length - 1;
    const next = event.key === "Home" ? 0 : event.key === "End" ? last : event.key === "ArrowRight" ? (index + 1) % items.length : (index - 1 + items.length) % items.length;
    onChange(items[next].id);
    refs.current[items[next].id]?.focus();
  };
  return (
    <div className="c4-tabs" role="tablist" aria-label={label}>
      {items.map((item, i) => (
        <button
          key={item.id}
          ref={(n) => { refs.current[item.id] = n; }}
          type="button"
          role="tab"
          id={`${base}-${item.id}`}
          aria-selected={value === item.id}
          tabIndex={value === item.id ? 0 : -1}
          className="c4-tab"
          onClick={() => onChange(item.id)}
          onKeyDown={(e) => onKey(e, i)}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}

/* ---------- 01 the problem ---------- */

const SAMPLE_APPS = ["notes-app", "photo-sorter", "your-new-app"];
const SAMPLE_CLIENTS = ["Codex", "Claude Code", "Gemini CLI", "Ollama"];

export function Problem() {
  return (
    <section className="c4-sec" id="problem" aria-labelledby="c4-problem-title" data-bounds>
      <SecHead path="~/Documents/integrations — 2 windows" id="c4-problem-title" title={PROBLEM.line}>
        <p>{PROBLEM.fix}</p>
      </SecHead>
      <div className="c4-pair">
        <div className="c4-shell">
          <Win id="before" title="before.md" meta="sample apps">
            <p className="c4-win-lead">Three apps, four clients. Each cell is its own set of {PROBLEM.burdens.join(", ")}.</p>
            <div className="c4-matrix" role="table" aria-label="Integrations each app maintains without OriginLM">
              <div role="row" className="c4-mx-row c4-mx-head">
                <span role="columnheader" className="c4-mx-corner">app \ client</span>
                {SAMPLE_CLIENTS.map((c) => <span role="columnheader" key={c}>{c}</span>)}
              </div>
              {SAMPLE_APPS.map((app) => (
                <div role="row" className="c4-mx-row" key={app}>
                  <span role="rowheader" className="c4-mx-app">{app}</span>
                  {SAMPLE_CLIENTS.map((c) => (
                    <span role="cell" key={c} className="c4-mx-cell">
                      {PROBLEM.burdens.map((b) => <span key={b} className="c4-mx-b">{b.replace("client-specific ", "")}</span>)}
                      <span className="c4-mx-n" aria-hidden="true">×{PROBLEM.burdens.length}</span>
                    </span>
                  ))}
                </div>
              ))}
            </div>
            <p className="c4-mx-sum"><strong>{SAMPLE_APPS.length * SAMPLE_CLIENTS.length}</strong> integrations to write and keep alive, in this sample.</p>
          </Win>
        </div>
        <div className="c4-shell c4-shell-offset">
          <Win id="after" title="after.md" meta="same apps">
            <p className="c4-win-lead">One runtime on the machine. Each app integrates once.</p>
            <div className="c4-after">
              <ul className="c4-after-apps">
                {SAMPLE_APPS.map((app) => <li key={app}><span>{app}</span><code>createLocalAI()</code></li>)}
              </ul>
              <div className="c4-after-hub" aria-hidden="true"><span>OriginLM</span><small>one local runtime</small></div>
              <ul className="c4-after-clients">
                {DISCOVERED.slice(0, 6).map((c) => <li key={c}>{c}</li>)}
                <li className="c4-more">+ {DISCOVERED.length - 6} more {DISCOVERED_TAIL}</li>
              </ul>
            </div>
            <p className="c4-mx-sum"><strong>{SAMPLE_APPS.length}</strong> integrations. The runtime handles the clients.</p>
          </Win>
        </div>
      </div>
    </section>
  );
}

/* ---------- 02 how it works ---------- */

const HOW_CODE: Record<string, string> = { discover: CODE.discover, grant: CODE.grant, call: CODE.run };

export function How() {
  return (
    <section className="c4-sec" id="how" aria-labelledby="c4-how-title" data-bounds>
      <SecHead path="~/Documents/how-it-works — 3 windows" id="c4-how-title" title="Discover. Grant. Call it from your app.">
        <p>Three calls, in this order, on one machine. Discovery is passive, execution is granted, and the turn streams back as typed events.</p>
      </SecHead>
      <div className="c4-cascade">
        {HOW.map((step, i) => (
          <div className="c4-shell" key={step.id} style={{ ["--i" as string]: i }}>
            <Win id={step.id as "discover" | "grant" | "call"} title={`${i + 1} — ${step.verb}`} meta={<code>{step.method}</code>}>
              <p className="c4-how-body">{step.body}</p>
              <Code code={HOW_CODE[step.id]} label={`${step.verb} code`} wrap />
              <p className="c4-how-detail">{step.detail}</p>
            </Win>
          </div>
        ))}
      </div>
    </section>
  );
}

/* ---------- 03 SDK / quickstart ---------- */

type Lang = "node" | "python" | "rust";
const LANGS: { id: Lang; label: string; file: string; install: string }[] = [
  { id: "node", label: "Node", file: "hello.mjs", install: INSTALL[0].cmd },
  { id: "python", label: "Python", file: "hello.py", install: INSTALL[1].cmd },
  { id: "rust", label: "Rust", file: "src/main.rs", install: INSTALL[2].cmd },
];

export function Sdk() {
  const [lang, setLang] = useState<Lang>("node");
  const current = LANGS.find((l) => l.id === lang)!;
  return (
    <section className="c4-sec" id="sdk" aria-labelledby="c4-sdk-title" data-bounds>
      <SecHead path="~/Applications/Quickstart.app" id="c4-sdk-title" title="Three SDKs, one protocol.">
        <p>TypeScript, Python and Rust clients speak the same local protocol to the same runtime. Importing never scans, spawns a process or reads a credential.</p>
      </SecHead>
      <div className="c4-pair c4-pair-sdk">
        <div className="c4-shell">
          <Win id="sdk" title={`Quickstart — ${current.file}`} meta="from the quickstart docs">
            <Tabs items={LANGS} value={lang} onChange={setLang} label="SDK language" />
            <div role="tabpanel" aria-label={`${current.label} quickstart`}>
              <div className="c4-cmd"><span className="c4-ps1" aria-hidden="true">$</span><code>{current.install}</code><Copy value={current.install} /></div>
              <Code code={QUICKSTART[lang]} label={`${current.file} code`} />
              <p className="c4-fine">Verbatim from the quickstart. Metadata probes and execution need additional policy; a discovery signal is not permission to run a provider.</p>
            </div>
          </Win>
        </div>
        <div className="c4-shell c4-shell-offset">
          <Win id="toolkit" title="About This Toolkit" meta="one runtime">
            <ul className="c4-about">
              {TOOLKIT.map(([name, text]) => <li key={name}><strong>{name}</strong><span>{text}</span></li>)}
            </ul>
            <a className="c4-link" href={DOCS.quickstarts}>All quickstarts, incl. Tauri, Electron and GPUI</a>
          </Win>
        </div>
      </div>
    </section>
  );
}

/* ---------- 04 Providers.app ---------- */

const KINDS = ["All", "Native client", "Local runtime", "API route"] as const;
type Kind = (typeof KINDS)[number];
const STATE_TEXT: Record<OpState, string> = { yes: "yes", partial: "partial", none: "no" };

function StateMark({ state }: { state: OpState }) {
  return <span className={`c4-op is-${state}`}><span aria-hidden="true">{state === "yes" ? "●" : state === "partial" ? "◐" : "–"}</span><span className="visually-hidden">{STATE_TEXT[state]}</span></span>;
}

export function Providers() {
  const [kind, setKind] = useState<Kind>("All");
  const [sel, setSel] = useState(PROVIDERS[0].id);
  const rows = useMemo(() => PROVIDERS.filter((p) => kind === "All" || p.kind === kind), [kind]);
  const current = PROVIDERS.find((p) => p.id === sel) ?? PROVIDERS[0];
  return (
    <section className="c4-sec" id="providers" aria-labelledby="c4-prov-title" data-bounds>
      <SecHead path="~/Applications/Providers.app" id="c4-prov-title" title="Every client already on the machine, with evidence.">
        <p>Passive discovery finds {DISCOVERED.join(", ")} {DISCOVERED_TAIL}. Each operation is marked by evidence. <em>Partial</em> means implemented but conditional, fixture-tested only, or gated by a capability the provider may not report: never an inferred yes.</p>
      </SecHead>
      <div className="c4-shell">
        <Win id="providers" title="Providers" meta={`${rows.length} items`} className="c4-finder">
          <div className="c4-finder-grid">
            <nav className="c4-side" aria-label="Provider kinds">
              <p className="c4-label">Locations</p>
              <ul>
                {KINDS.map((k) => (
                  <li key={k}>
                    <button type="button" className="c4-side-item" aria-pressed={kind === k} onClick={() => setKind(k)}>
                      <span>{k === "All" ? "All providers" : `${k}s`}</span>
                      <span className="c4-count">{k === "All" ? PROVIDERS.length : PROVIDERS.filter((p) => p.kind === k).length}</span>
                    </button>
                  </li>
                ))}
              </ul>
              <p className="c4-legend"><StateMark state="yes" /> yes <StateMark state="partial" /> partial <StateMark state="none" /> no</p>
            </nav>
            <div className="c4-table-wrap" role="region" aria-label="Provider evidence table" tabIndex={0} data-lenis-prevent>
              <table className="c4-table">
                <caption className="visually-hidden">Operations each provider supports, from the support matrix</caption>
                <thead>
                  <tr><th scope="col">Name</th>{OP_LABELS.map((o) => <th scope="col" key={o.key} className="c4-opcol">{o.label}</th>)}</tr>
                </thead>
                <tbody>
                  {rows.map((p) => (
                    <tr key={p.id} className={p.id === sel ? "is-sel" : ""}>
                      <th scope="row">
                        <button type="button" className="c4-row-btn" aria-pressed={p.id === sel} onClick={() => setSel(p.id)}>
                          <span className={`c4-glyph is-${p.kind === "Local runtime" ? "local" : p.kind === "API route" ? "api" : "native"}`} aria-hidden="true">{p.glyph}</span>
                          <span className="c4-row-name">{p.name}<small>{p.kind}</small></span>
                        </button>
                      </th>
                      {OP_LABELS.map((o) => <td key={o.key} className="c4-opcol"><StateMark state={p.ops[o.key]} /></td>)}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <aside className="c4-info" aria-label={`${current.name} info`} aria-live="polite">
              <p className="c4-label">Get Info</p>
              <p className="c4-info-name"><span className={`c4-glyph is-${current.kind === "Local runtime" ? "local" : current.kind === "API route" ? "api" : "native"}`} aria-hidden="true">{current.glyph}</span>{current.name}</p>
              <p className="c4-info-kind">{current.kind}</p>
              <p className="c4-info-note">{current.note}</p>
              <a className="c4-link" href={DOCS.matrix}>Support matrix</a>
            </aside>
          </div>
        </Win>
      </div>
    </section>
  );
}

/* ---------- 05 Grants.app + safety ---------- */

const GRANT_APPS = ["your-new-app", "notes-app", "photo-sorter"];
type Scopes = Record<string, boolean>;
const NEEDS_EXECUTION = new Set(["workspace", "tools", "paid"]);

export function Grants() {
  const [app, setApp] = useState(GRANT_APPS[0]);
  const [provider, setProvider] = useState(DEMO_ROUTES[0].providerId);
  const [scopes, setScopes] = useState<Record<string, Scopes>>({
    "your-new-app": { metadata: true, execution: true, workspace: true },
    "notes-app": { metadata: true },
    "photo-sorter": {},
  });
  const current = scopes[app] ?? {};
  const toggle = (id: string) => setScopes((s) => {
    const next = { ...s[app], [id]: !s[app]?.[id] };
    if (id === "execution" && !next.execution) { next.workspace = false; next.tools = false; next.paid = false; }
    return { ...s, [app]: next };
  });
  const revoke = () => setScopes((s) => ({ ...s, [app]: {} }));
  const json = grantManifest({ app, providerId: provider, scopes: current });
  const any = Object.values(current).some(Boolean);

  return (
    <section className="c4-sec" id="grants" aria-labelledby="c4-grants-title" data-bounds>
      <SecHead path="~/Applications/Grants.app" id="c4-grants-title" title="Discovery is not permission.">
        <p>{HOW[1].body}</p>
      </SecHead>
      <div className="c4-pair c4-pair-grants">
        <div className="c4-shell">
          <Win id="grants" title="Grants" meta="per app · revocable" className="c4-grants">
            <div className="c4-grants-grid">
              <nav className="c4-side" aria-label="Apps">
                <p className="c4-label">Connected apps <span className="c4-tag">sample</span></p>
                <ul>
                  {GRANT_APPS.map((a) => {
                    const on = Object.values(scopes[a] ?? {}).filter(Boolean).length;
                    return (
                      <li key={a}>
                        <button type="button" className="c4-side-item" aria-pressed={a === app} onClick={() => setApp(a)}>
                          <span>{a}</span><span className="c4-count">{on ? `${on} on` : "none"}</span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              </nav>
              <div className="c4-grant-main">
                <label className="c4-select">
                  <span className="c4-label">Provider</span>
                  <select value={provider} onChange={(e) => setProvider(e.target.value)}>
                    {DEMO_ROUTES.map((r) => <option key={r.id} value={r.providerId}>{r.name} ({r.providerId})</option>)}
                  </select>
                </label>
                <ul className="c4-scopes">
                  {GRANT_SCOPES.map((s) => {
                    const blocked = NEEDS_EXECUTION.has(s.id) && !current.execution;
                    const on = !!current[s.id];
                    return (
                      <li key={s.id} className={blocked ? "is-blocked" : ""}>
                        <span className="c4-scope-text">
                          <span className="c4-scope-name" id={`c4-scope-${s.id}`}>{s.label}</span>
                          <span className="c4-scope-note" id={`c4-scope-${s.id}-note`}>{blocked ? "Needs execution first." : s.note} <code>{s.field}</code></span>
                        </span>
                        <button
                          type="button" role="switch" className="c4-switch" aria-checked={on}
                          aria-labelledby={`c4-scope-${s.id}`} aria-describedby={`c4-scope-${s.id}-note`}
                          aria-disabled={blocked} onClick={() => { if (!blocked) toggle(s.id); }}
                        >
                          <span className="c4-switch-knob" aria-hidden="true" />
                        </button>
                      </li>
                    );
                  })}
                </ul>
                <button type="button" className="c4-btn c4-btn-danger" onClick={revoke} disabled={!any}>Revoke {app}</button>
              </div>
              <div className="c4-grant-json">
                <p className="c4-label">grants.json <span className="c4-tag">owner-only file</span></p>
                <Code code={json} label="Resulting grants manifest" wrap />
                <p className="c4-fine">The shared broker mints one credential per app from this manifest. No protocol request can widen its own grant.</p>
              </div>
            </div>
          </Win>
        </div>
        <div className="c4-shell c4-shell-offset">
          <Win id="safety" title="Privacy & Security" meta={<Lock />}>
            <ul className="c4-safety">
              {SAFETY.map(([title, text]) => (
                <li key={title}><span className="c4-check" aria-hidden="true">✓</span><span><strong>{title}</strong><span>{text}</span></span></li>
              ))}
            </ul>
            <p className="c4-links-row"><a className="c4-link" href={DOCS.threat}>Threat model</a><a className="c4-link" href={DOCS.privacy}>Privacy boundary</a></p>
          </Win>
        </div>
      </div>
    </section>
  );
}

/* ---------- 06 Sessions.app ---------- */

type SessionEv = { n: number; type: string; detail: string };
const S_ROUTE = DEMO_ROUTES[1];
const S_PROMPT = "Find the TODOs in this workspace.";
const FEATURES_TEXT = SESSION_FEATURES.map((f) => f[0].toLowerCase() + f.slice(1));
type SPhase = "idle" | "running" | "approval" | "streaming" | "done" | "cancelled";

export function Sessions({ capture, reduced }: { capture: boolean; reduced: boolean }) {
  const [phase, setPhase] = useState<SPhase>(capture || reduced ? "done" : "idle");
  const [evs, setEvs] = useState<SessionEv[]>(() => (capture || reduced ? fullSession(true) : []));
  const [chars, setChars] = useState(capture || reduced ? S_ROUTE.reply.length : 0);
  const [denied, setDenied] = useState(false);
  const timers = useRef<number[]>([]);
  const ref = useRef<HTMLDivElement>(null);
  const clear = () => { timers.current.forEach((t) => window.clearTimeout(t)); timers.current = []; };
  const at = (ms: number, fn: () => void) => timers.current.push(window.setTimeout(fn, ms));
  const add = (type: string, detail: string) => setEvs((e) => [...e, { n: 41 + e.length, type, detail }]);

  const start = () => {
    clear(); setDenied(false); setChars(0);
    setEvs([{ n: 41, type: "state", detail: "ready" }]);
    setPhase("running");
    if (reduced) { setEvs((e) => [...e, { n: 42, type: "state", detail: "running" }, { n: 43, type: "approval_request", detail: "read · workspace app" }]); setPhase("approval"); return; }
    at(500, () => add("state", "running"));
    at(1100, () => { add("approval_request", "read · workspace app"); setPhase("approval"); });
  };

  const answer = (allow: boolean) => {
    setDenied(!allow);
    add("tool", allow ? "read · completed" : "read · denied");
    setPhase("streaming");
    const reply = allow ? S_ROUTE.reply : "Understood. Nothing was read.";
    if (reduced) { setChars(reply.length); setEvs((e) => [...e, ...tail(e.length, reply.length)]); setPhase("done"); return; }
    const words = reply.split(/(?<= )/);
    let count = 0;
    words.forEach((w, i) => { count += w.length; const n = count; at(300 + i * 80, () => setChars(n)); });
    const end = 300 + words.length * 80;
    at(end, () => add("text_delta", `${reply.length} chars`));
    at(end + 200, () => add("usage", "unit tokens, used 412"));
    at(end + 400, () => { add("state", "completed"); add("final", "completed"); setPhase("done"); });
  };

  const cancel = () => { clear(); add("state", "cancelled"); setPhase("cancelled"); };

  useEffect(() => {
    if (capture || reduced || !ref.current) return;
    const io = new IntersectionObserver(([entry]) => { if (entry.isIntersecting) { start(); io.disconnect(); } }, { threshold: 0.35 });
    io.observe(ref.current);
    return () => { io.disconnect(); clear(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const reply = (denied ? "Understood. Nothing was read." : S_ROUTE.reply).slice(0, chars);
  const showReply = phase === "streaming" || phase === "done" || (phase === "cancelled" && chars > 0);
  const running = phase === "running" || phase === "approval" || phase === "streaming";

  return (
    <section className="c4-sec" id="sessions" aria-labelledby="c4-sess-title" data-bounds>
      <SecHead path="~/Applications/Sessions.app" id="c4-sess-title" title="Sessions that stream, ask first, and resume.">
        <p>Granted sessions carry {FEATURES_TEXT.slice(0, -1).join(", ")} and {FEATURES_TEXT[FEATURES_TEXT.length - 1]}, only after the host app grants them explicitly.</p>
      </SecHead>
      <div className="c4-shell" ref={ref}>
        <Win id="sessions" title={`Sessions — ${S_ROUTE.name} · ${S_ROUTE.model}`} meta={<span className={`c4-status is-${phase === "done" ? "done" : running ? "running" : "idle"}`}>{phase === "approval" ? "awaiting_approval" : phase === "done" ? "completed" : phase}</span>} className="c4-sessions">
          <div className="c4-sess-grid">
            <div className="c4-transcript" aria-live="polite">
              <p className="c4-label">Transcript <span className="c4-tag">sample</span></p>
              {phase !== "idle" && <div className="c4-msg is-user"><span className="c4-who">your-new-app</span><span>{S_PROMPT}</span></div>}
              {(phase === "approval" || evs.some((e) => e.type === "tool")) && (
                <div className="c4-msg is-approval">
                  <span className="c4-who">approval_request</span>
                  <span>{S_ROUTE.name} asks to read files in workspace <code>app</code>.</span>
                  {phase === "approval" ? (
                    <span className="c4-approve">
                      <button type="button" className="c4-btn c4-btn-go" onClick={() => answer(true)}>Allow once</button>
                      <button type="button" className="c4-btn" onClick={() => answer(false)}>Deny</button>
                    </span>
                  ) : <span className="c4-answered">answered once with approvals.respond: {denied ? "deny" : "allow"}</span>}
                </div>
              )}
              {showReply && <div className="c4-msg is-ai"><span className="c4-who">{S_ROUTE.name}</span><span>{reply}{phase === "streaming" && <span className="c4-caret" aria-hidden="true" />}</span></div>}
              <div className="c4-sess-actions">
                {running && <button type="button" className="c4-btn" onClick={cancel}>Cancel turn <code>turns.cancel</code></button>}
                {!running && <button type="button" className="c4-btn" onClick={start}>{phase === "idle" ? "Start session" : "Replay"}</button>}
              </div>
            </div>
            <div className="c4-evlog">
              <p className="c4-label">events.subscribe · cursor</p>
              <ol data-lenis-prevent>
                {evs.map((e) => <li key={e.n} className={`c4-ev is-${e.type}`}><span className="c4-ev-n">#{e.n}</span><span className="c4-ev-type">{e.type}</span><span className="c4-ev-detail">{e.detail}</span></li>)}
              </ol>
              <p className="c4-fine">Reconnect with the last cursor to resume. If the runtime cannot fill a gap it says so with <code>EVENT_GAP</code>. Cancel only touches turns your app started.</p>
            </div>
          </div>
          <ul className="c4-chips" aria-label="Session features">
            {SESSION_FEATURES.map((f) => <li key={f}>{f}</li>)}
          </ul>
        </Win>
      </div>
    </section>
  );
}

function fullSession(allow: boolean): SessionEv[] {
  return [
    { n: 41, type: "state", detail: "ready" },
    { n: 42, type: "state", detail: "running" },
    { n: 43, type: "approval_request", detail: "read · workspace app" },
    { n: 44, type: "tool", detail: allow ? "read · completed" : "read · denied" },
    ...tail(4, S_ROUTE.reply.length),
  ];
}
function tail(len: number, chars: number): SessionEv[] {
  return [
    { n: 41 + len, type: "text_delta", detail: `${chars} chars` },
    { n: 42 + len, type: "usage", detail: "unit tokens, used 412" },
    { n: 43 + len, type: "state", detail: "completed" },
    { n: 44 + len, type: "final", detail: "completed" },
  ];
}

/* ---------- 07 Usage.app (the control center, small on purpose) ---------- */

export function Usage() {
  const wm = useWM();
  const minimized = wm.states.usage !== "open";
  return (
    <section className="c4-sec c4-sec-usage" id="usage" aria-labelledby="c4-usage-title" data-bounds>
      <SecHead path="~/Applications/Usage.app — minimized" id="c4-usage-title" title="And yes, it has charts.">
        <p>{CONTROL_CENTER.line} It is the smallest window on this page, and it starts in the dock. That is the right size for it.</p>
      </SecHead>
      <div className="c4-shell c4-shell-usage">
        <Win id="usage" title="Usage" meta="sample" resizable minimizedNote="Usage.app is in the dock. Open it if you like charts.">
          <ul className="c4-meters">
            {STORY_NODES.map((n) => (
              <li key={n.id}>
                <span className="c4-meter-name">{n.name}<small>{n.window ?? "no window"}</small></span>
                {n.reading.state === "known" ? (
                  <span className="c4-meter" role="img" aria-label={`${n.reading.used}% used, resets ${n.resetsAt}`}>
                    <span className="c4-meter-bar"><span style={{ width: `${n.reading.used}%` }} /></span>
                    <span className="c4-meter-v">{n.reading.used}%</span>
                  </span>
                ) : (
                  <span className="c4-meter-unknown">{n.reading.state}: {n.reading.reason}</span>
                )}
              </li>
            ))}
          </ul>
          <p className="c4-fine">{CONTROL_CENTER.note}</p>
          <p className="c4-views">{CONTROL_CENTER.views.join(" · ")}</p>
          <div className="c4-cmd"><span className="c4-ps1" aria-hidden="true">$</span><code>{CONTROL_CENTER.run}</code><Copy value={CONTROL_CENTER.run} /></div>
        </Win>
      </div>
      {minimized && <p className="c4-fine c4-usage-hint">Drag its corner to resize once it is open.</p>}
    </section>
  );
}

/* ---------- 08 Install.app ---------- */

const SCRIPT: { comment: string; cmd: string }[] = [
  { comment: "# TypeScript / Node", cmd: INSTALL[0].cmd },
  { comment: "# Python", cmd: INSTALL[1].cmd },
  { comment: "# Rust", cmd: INSTALL[2].cmd },
];
const SCRIPT_TEXT = SCRIPT.map((s) => `${s.comment}\n$ ${s.cmd}`).join("\n");

export function Install({ capture, reduced }: { capture: boolean; reduced: boolean }) {
  const [typed, setTyped] = useState(capture || reduced ? SCRIPT_TEXT.length : 0);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (capture || reduced || !ref.current) return;
    let timer = 0;
    const io = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return;
      io.disconnect();
      let n = 0;
      const tick = () => {
        const ch = SCRIPT_TEXT[n];
        n += 1;
        setTyped(n);
        if (n < SCRIPT_TEXT.length) timer = window.setTimeout(tick, ch === "\n" ? 260 : SCRIPT_TEXT[n - 2] === "#" || ch === "#" ? 10 : 26);
      };
      tick();
    }, { threshold: 0.4 });
    io.observe(ref.current);
    return () => { io.disconnect(); window.clearTimeout(timer); };
  }, [capture, reduced]);
  const shown = SCRIPT_TEXT.slice(0, typed);
  const done = typed >= SCRIPT_TEXT.length;
  return (
    <section className="c4-sec" id="install" aria-labelledby="c4-install-title" data-bounds>
      <SecHead path="~/Applications/Install.app" id="c4-install-title" title="Install it. It is an alpha.">
        <p>{STATUS_LONG}</p>
      </SecHead>
      <div className="c4-pair c4-pair-install">
        <div className="c4-shell" ref={ref}>
          <Win id="install" title="Terminal — zsh — install" meta="80×24" className="c4-term">
            <pre className="c4-term-pre" aria-hidden="true">
              {shown.split("\n").map((line, i) => (
                <span key={i} className={line.startsWith("#") ? "c4-t-com" : "c4-t-cmd"}>{line}{"\n"}</span>
              ))}
              <span className={`c4-t-caret ${done ? "is-idle" : ""}`}>{done ? "$ " : ""}</span>
            </pre>
            <ul className="visually-hidden">
              {SCRIPT.map((s) => <li key={s.cmd}>{s.comment.replace("# ", "")}: {s.cmd}</li>)}
            </ul>
          </Win>
        </div>
        <div className="c4-copylist">
          {INSTALL.map((i) => (
            <div className="c4-cmd" key={i.id}><span className="c4-ps1">{i.label}</span><code>{i.cmd}</code><Copy value={i.cmd} /></div>
          ))}
          <p className="c4-links-row">
            <a className="c4-btn c4-btn-go" href={DOCS.quickstarts}>Read the quickstarts</a>
            <a className="c4-btn" href={DOCS.install}>Install guide</a>
          </p>
        </div>
      </div>
    </section>
  );
}
