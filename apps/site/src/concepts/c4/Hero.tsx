import { useEffect, useMemo, useRef } from "react";
import { highlight } from "../../lib/highlight";
import { PITCH, PROVIDERS } from "../shared/facts";
import { Win } from "./Window";
import { stepLines, type Demo } from "./demo";
import { scrollToId } from "./wm";

const GLYPH: Record<string, string> = Object.fromEntries(PROVIDERS.map((p) => [p.id, p.glyph]));
const GLYPH_OF: Record<string, string> = { codex: GLYPH["codex"], claude: GLYPH["claude-code"], gemini: GLYPH["gemini-cli"], ollama: GLYPH["ollama"] };

export function Hero({ demo }: { demo: Demo }) {
  const picked = demo.touched;
  const granted = demo.isGranted;
  const ran = demo.phase === "done";
  return (
    <section className="c4-hero" id="demo" aria-labelledby="c4-h1">
      <div className="c4-desk" data-bounds>
        <div className="c4-hero-copy">
          <p className="c4-eyebrow"><span className="c4-dot" aria-hidden="true" /> ~/Desktop · a working demo, sample data</p>
          <h1 id="c4-h1" className="c4-h1">{PITCH.headline}</h1>
          <p className="c4-hero-lead">{PITCH.body}</p>
          <ol className="c4-steps" aria-label="Try it">
            <li className={picked ? "is-done" : "is-now"}><span className="c4-step-n" aria-hidden="true">{picked ? "✓" : "1"}</span> Pick a model in <strong>OriginLM</strong><span className="visually-hidden">{picked ? " (done)" : ""}</span></li>
            <li className={granted ? "is-done" : picked ? "is-now" : ""}><span className="c4-step-n" aria-hidden="true">{granted ? "✓" : "2"}</span> Flip the execution grant<span className="visually-hidden">{granted ? " (done)" : ""}</span></li>
            <li className={ran ? "is-done" : granted ? "is-now" : ""}><span className="c4-step-n" aria-hidden="true">{ran ? "✓" : "3"}</span> Press <strong>Run</strong> in your app<span className="visually-hidden">{ran ? " (done)" : ""}</span></li>
          </ol>
          <a className="c4-read" href="#problem">Just read instead <span aria-hidden="true">↓</span></a>
          <div className="c4-files" aria-label="Desktop shortcuts">
            <a className="c4-file" href="#grants"><span className="c4-file-ic is-json" aria-hidden="true">{"{ }"}</span><span>grants.json</span></a>
            <a className="c4-file" href="#providers"><span className="c4-file-ic is-dir" aria-hidden="true" /><span>Providers</span></a>
            <a className="c4-file" href="#install"><span className="c4-file-ic is-sh" aria-hidden="true">&gt;_</span><span>install.sh</span></a>
          </div>
        </div>

        <div className="c4-shell c4-shell-ol">
          <OriginLMWindow demo={demo} />
        </div>
        <div className="c4-shell c4-shell-app">
          <AppWindow demo={demo} />
        </div>
      </div>
    </section>
  );
}

function OriginLMWindow({ demo }: { demo: Demo }) {
  const grantRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!demo.nudge || !grantRef.current) return;
    const el = grantRef.current;
    el.classList.remove("is-nudged");
    void el.offsetWidth;
    el.classList.add("is-nudged");
  }, [demo.nudge]);

  return (
    <Win id="originlm" title="OriginLM" meta={<span>broker · local</span>} className="c4-ol">
      <div className="c4-ol-head">
        <span className="c4-label">Discovered on this machine</span>
        <span className="c4-tag">sample</span>
      </div>
      <p className="c4-ol-sub">Passive discovery, metadata only. Model ids are examples.</p>
      <fieldset className="c4-routes">
        <legend className="visually-hidden">Pick a model for your-new-app</legend>
        {demo.routes.map((r) => {
          const on = r.id === demo.route.id;
          const ok = demo.granted.has(r.id);
          return (
            <label key={r.id} className={`c4-route ${on ? "is-on" : ""}`}>
              <input type="radio" name="c4-route" value={r.id} checked={on} onChange={() => demo.pick(r.id)} onClick={() => demo.pick(r.id)} />
              <span className={`c4-glyph is-${r.kind === "local" ? "local" : "native"}`} aria-hidden="true">{GLYPH_OF[r.id]}</span>
              <span className="c4-route-text">
                <span className="c4-route-name">{r.name}</span>
                <span className="c4-route-model">{r.model}</span>
              </span>
              <span className={`c4-route-state ${ok ? "is-ok" : ""}`}>{ok ? "granted" : r.kind}</span>
            </label>
          );
        })}
      </fieldset>

      <div className="c4-grant" ref={grantRef}>
        <div className="c4-grant-row">
          <span className="c4-grant-text">
            <span className="c4-grant-title" id="c4-grant-label">Execution for your-new-app</span>
            <span className="c4-grant-on">on {demo.route.name} · {demo.route.kind === "local" ? "local route" : "subscription route"}</span>
          </span>
          <button
            type="button"
            role="switch"
            aria-checked={demo.isGranted}
            aria-labelledby="c4-grant-label"
            aria-describedby="c4-grant-why"
            className="c4-switch"
            onClick={demo.toggleGrant}
          >
            <span className="c4-switch-knob" aria-hidden="true" />
          </button>
        </div>
        <p className="c4-grant-why" id="c4-grant-why">
          {demo.isGranted
            ? <>your-new-app may start turns on {demo.route.name} in workspace <code>notes</code>. Revocable any time. Paid routes stay off.</>
            : <><strong>Discovery is not permission.</strong> your-new-app can see this route. It cannot run it until you grant execution.</>}
        </p>
      </div>
    </Win>
  );
}

function AppWindow({ demo }: { demo: Demo }) {
  const lines = useMemo(() => demo.code.split("\n"), [demo.code]);
  const starts = useMemo(() => stepLines(demo.code), [demo.code]);
  const codeRef = useRef<HTMLPreElement>(null);
  const reply = demo.route.reply.slice(0, demo.chars);
  const active = (i: number) => {
    if (demo.step < 0 || demo.step >= starts.length) return false;
    const from = starts[demo.step];
    const to = demo.step + 1 < starts.length ? starts[demo.step + 1] : lines.length;
    return i >= from && i < to;
  };

  useEffect(() => {
    const pre = codeRef.current;
    if (!pre || demo.step < 0 || demo.step >= starts.length) return;
    const line = pre.querySelector(`[data-line="${starts[demo.step]}"]`) as HTMLElement | null;
    if (line) pre.scrollTo({ top: Math.max(0, line.offsetTop - 40), behavior: "smooth" });
  }, [demo.step, starts]);

  const status = demo.phase === "running" ? "streaming" : demo.phase === "done" ? "final" : demo.isGranted ? "ready" : "no grant";

  return (
    <Win id="app" title="your-new-app — untitled" meta={<span className={`c4-status is-${demo.phase}`}>{status}</span>} className="c4-app">
      <div className="c4-app-bar">
        <div className="c4-prompt">
          <span className="c4-label">Prompt</span>
          <span className="c4-prompt-text">Summarize the open questions in these notes.</span>
        </div>
        <button
          type="button"
          className={`c4-run ${demo.isGranted ? "" : "is-locked"}`}
          aria-disabled={!demo.isGranted || demo.phase === "running"}
          aria-describedby="c4-run-why"
          onClick={demo.run}
        >
          <span aria-hidden="true">▶</span> {demo.phase === "done" ? "Run again" : "Run"}
        </button>
      </div>
      <p id="c4-run-why" className={`c4-run-why ${demo.isGranted ? "" : "is-warn"}`} aria-live="polite">
        {demo.isGranted
          ? demo.phase === "idle" ? `Granted. Run calls ${demo.route.name} through OriginLM.` : demo.phase === "running" ? "Streaming typed events from a cursor…" : "Done. Pick another model to see the call change."
          : demo.nudge ? `Not yet: grant execution on ${demo.route.name} in OriginLM first.` : "Run stays locked until OriginLM grants execution."}
      </p>

      <div className="c4-editor">
        <div className="c4-editor-tab"><span>main.ts</span><span className="c4-editor-note">real SDK shape</span></div>
        <pre ref={codeRef} className="c4-code" tabIndex={0} aria-label="main.ts, the call your app makes" data-lenis-prevent>
          <code key={demo.route.id}>
            {lines.map((line, i) => (
              <span key={i} data-line={i} className={`c4-line ${active(i) ? "is-active" : ""} ${line.includes(`"${demo.route.query}"`) ? "is-query" : ""}`}>
                <span className="c4-ln" aria-hidden="true">{i + 1}</span>
                <span className="c4-lc">{line ? highlight(line) : " "}</span>
              </span>
            ))}
          </code>
        </pre>
      </div>

      <div className="c4-out">
        <div className="c4-reply">
          <span className="c4-label">Reply <span className="c4-tag">sample</span></span>
          <p className="c4-reply-text" aria-live="polite" aria-busy={demo.phase === "running"}>
            {reply || <span className="c4-reply-empty">{demo.isGranted ? "Waiting for Run." : "Nothing yet. No grant, no turn."}</span>}
            {demo.phase === "running" && <span className="c4-caret" aria-hidden="true" />}
          </p>
        </div>
        <ol className="c4-events" aria-label="Session events">
          {demo.events.length === 0 && <li className="c4-ev-empty">events.subscribe · no events</li>}
          {demo.events.map((e) => (
            <li key={e.type} className={`c4-ev is-${e.type}`}><span className="c4-ev-type">{e.type}</span><span className="c4-ev-detail">{e.detail}</span></li>
          ))}
        </ol>
      </div>
    </Win>
  );
}

export function goDemo() { scrollToId("demo"); }
