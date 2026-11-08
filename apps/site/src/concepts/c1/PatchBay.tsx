/* The hero: a playable patch bay. One cable runs from YOUR APP's output jack; drag its
   plug onto a provider jack (or press a jack) and the console shows the real-shaped SDK
   call for that route while the app's screen streams a labelled sample reply.

   The cable is a quadratic bezier. Its control point is a damped spring chasing a
   "hanging" target below the chord, so the cable lags, sags and wobbles as the plug
   moves. The ticker only runs while something is moving. */
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { gsap, prefersReducedMotion } from "../shared/motion";
import { DEMO_ROUTES, PROVIDERS, VERSION, tsCall, type DemoRoute } from "../shared/facts";
import { highlight } from "../../lib/highlight";

type JackState = "exec" | "partial" | "refused";
type Jack = { id: string; name: string; sub: string; state: JackState; route: DemoRoute; note?: string };

const note = (id: string) => PROVIDERS.find((p) => p.id === id)?.note ?? "";
const route = (id: string) => DEMO_ROUTES.find((r) => r.id === id)!;
const bare = (id: string, name: string, providerId: string): DemoRoute => ({ id, name, providerId, query: id, model: "", kind: "subscription", reply: "" });

export const JACKS: Jack[] = [
  { id: "codex", name: "Codex", sub: "subscription", state: "exec", route: route("codex") },
  { id: "claude", name: "Claude Code", sub: "subscription", state: "exec", route: route("claude") },
  { id: "ollama", name: "Ollama", sub: "local runtime", state: "exec", route: route("ollama") },
  { id: "gemini", name: "Gemini CLI", sub: "conditional", state: "partial", route: route("gemini"), note: note("gemini-cli") },
  { id: "cursor", name: "Cursor", sub: "metadata only", state: "refused", route: bare("cursor", "Cursor", "cursor"), note: note("cursor") },
  { id: "copilot", name: "GitHub Copilot", sub: "detected only", state: "refused", route: bare("copilot", "GitHub Copilot", "copilot"), note: note("copilot") },
];

type Pt = { x: number; y: number };
type Sim = {
  A: Pt; E: Pt; C: Pt; V: Pt; park: Pt; w: number; h: number;
  dragging: boolean; tweening: number; running: boolean; vertical: boolean;
};

const HIT = 58;

export function PatchBay() {
  const reduced = useRef(prefersReducedMotion()).current;
  const [connected, setConnected] = useState<string | null>(reduced ? "codex" : null);
  const [hover, setHover] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const boardRef = useRef<HTMLDivElement>(null);
  const appSocketRef = useRef<HTMLSpanElement>(null);
  const socketRefs = useRef<Record<string, HTMLSpanElement | null>>({});
  const pathRef = useRef<SVGPathElement>(null);
  const shadowRef = useRef<SVGPathElement>(null);
  const signalRef = useRef<SVGPathElement>(null);
  const plugRef = useRef<SVGGElement>(null);
  const positions = useRef<Record<string, Pt>>({});
  const connectedRef = useRef<string | null>(connected);
  const hoverRef = useRef<string | null>(null);
  const touched = useRef(false);
  const sim = useRef<Sim>({
    A: { x: 0, y: 0 }, E: { x: 0, y: 0 }, C: { x: 0, y: 0 }, V: { x: 0, y: 0 }, park: { x: 0, y: 0 },
    w: 0, h: 0, dragging: false, tweening: 0, running: false, vertical: false,
  });

  /* Where the control point wants to be: below the chord by an amount that grows as the
     ends get closer, as a cable of fixed length would hang. Vertical runs bow sideways. */
  const target = useCallback((): Pt => {
    const s = sim.current;
    const dx = s.E.x - s.A.x, dy = s.E.y - s.A.y;
    const d = Math.hypot(dx, dy);
    const L = Math.max(s.w, 320) * (s.vertical ? 0.95 : 0.7);
    let sag = 0.18 * Math.sqrt(Math.max(L * L - d * d, 0));
    sag = Math.min(Math.max(sag, 26), s.h * 0.3);
    const side = d > 0 ? Math.abs(dy) / d : 0;
    return { x: (s.A.x + s.E.x) / 2 + dx * 0.08, y: (s.A.y + s.E.y) / 2 + 2 * sag * (1 - side * 0.6) };
  }, []);

  const render = useCallback(() => {
    const s = sim.current;
    const d = `M${s.A.x.toFixed(1)},${s.A.y.toFixed(1)} Q${s.C.x.toFixed(1)},${s.C.y.toFixed(1)} ${s.E.x.toFixed(1)},${s.E.y.toFixed(1)}`;
    pathRef.current?.setAttribute("d", d);
    signalRef.current?.setAttribute("d", d);
    shadowRef.current?.setAttribute("d", `M${s.A.x.toFixed(1)},${(s.A.y + 14).toFixed(1)} Q${s.C.x.toFixed(1)},${(s.C.y + 22).toFixed(1)} ${s.E.x.toFixed(1)},${(s.E.y + 14).toFixed(1)}`);
    const angle = (Math.atan2(s.E.y - s.C.y, s.E.x - s.C.x) * 180) / Math.PI;
    plugRef.current?.setAttribute("transform", `translate(${s.E.x.toFixed(1)} ${s.E.y.toFixed(1)}) rotate(${angle.toFixed(1)})`);
  }, []);

  const step = useCallback(() => {
    const s = sim.current;
    const dt = Math.min(gsap.ticker.deltaRatio(60) / 60, 1 / 20);
    const T = target();
    const k = 70, c = 9;
    s.V.x += (k * (T.x - s.C.x) - c * s.V.x) * dt;
    s.V.y += (k * (T.y - s.C.y) - c * s.V.y) * dt;
    s.C.x += s.V.x * dt;
    s.C.y += s.V.y * dt;
    render();
    const settled = Math.hypot(s.V.x, s.V.y) < 1.5 && Math.hypot(T.x - s.C.x, T.y - s.C.y) < 0.4;
    if (!s.dragging && s.tweening === 0 && settled) {
      gsap.ticker.remove(step);
      s.running = false;
    }
  }, [render, target]);

  const kick = useCallback(() => {
    const s = sim.current;
    if (reduced) { s.C = target(); render(); return; }
    if (!s.running) { s.running = true; gsap.ticker.add(step); }
  }, [reduced, render, step, target]);

  /* Measure sockets relative to the board. Called on mount and on every resize. */
  const measure = useCallback(() => {
    const board = boardRef.current, app = appSocketRef.current;
    if (!board || !app) return;
    const b = board.getBoundingClientRect();
    const center = (el: Element): Pt => { const r = el.getBoundingClientRect(); return { x: r.left - b.left + r.width / 2, y: r.top - b.top + r.height / 2 }; };
    const s = sim.current;
    s.w = b.width; s.h = b.height;
    s.vertical = b.width < 640;
    s.A = center(app);
    s.park = s.vertical ? { x: s.A.x + 56, y: s.A.y + 64 } : { x: s.A.x + 64, y: s.A.y + 92 };
    for (const jack of JACKS) { const el = socketRefs.current[jack.id]; if (el) positions.current[jack.id] = center(el); }
    if (!s.dragging && s.tweening === 0) {
      const at = connectedRef.current ? positions.current[connectedRef.current] : s.park;
      s.E = { ...at };
      s.C = target();
      s.V = { x: 0, y: 0 };
    }
    render();
  }, [render, target]);

  useLayoutEffect(() => {
    measure();
    const ro = new ResizeObserver(() => measure());
    if (boardRef.current) ro.observe(boardRef.current);
    document.fonts?.ready.then(() => measure()).catch(() => {});
    return () => { ro.disconnect(); gsap.ticker.remove(step); sim.current.running = false; gsap.killTweensOf(sim.current.E); };
  }, [measure, step]);

  const settle = useCallback((id: string | null) => {
    connectedRef.current = id;
    setConnected(id);
  }, []);

  /* Fly the plug to a jack (tap / keyboard) or snap it in after a drag. */
  const connect = useCallback((id: string, mode: "fly" | "snap") => {
    touched.current = true;
    const s = sim.current;
    const p = positions.current[id];
    if (!p) return;
    gsap.killTweensOf(s.E);
    if (reduced) { s.E = { ...p }; kick(); settle(id); return; }
    if (connectedRef.current && connectedRef.current !== id) settle(null);
    s.tweening++;
    kick();
    const tweenTo = (x: number, y: number, duration: number, ease: string, done: () => void) =>
      gsap.to(s.E, { x, y, duration, ease, onUpdate: kick, onComplete: done });
    const finish = () => { s.tweening--; s.V.y += 260; kick(); settle(id); };
    if (mode === "snap") tweenTo(p.x, p.y, 0.26, "back.out(2.4)", finish);
    else {
      // Out of the old socket, a short lift, then across into the new one.
      const lift = { x: (s.E.x + p.x) / 2, y: Math.min(s.E.y, p.y) - 30 };
      gsap.timeline({ onUpdate: kick })
        .to(s.E, { x: lift.x, y: lift.y, duration: 0.28, ease: "power2.out" })
        .to(s.E, { x: p.x, y: p.y, duration: 0.42, ease: "back.out(1.6)", onComplete: finish });
    }
  }, [kick, reduced, settle]);

  const unplug = useCallback(() => {
    const s = sim.current;
    settle(null);
    if (reduced) { s.E = { ...s.park }; kick(); return; }
    s.tweening++;
    gsap.to(s.E, { x: s.park.x, y: s.park.y, duration: 0.5, ease: "power3.out", onUpdate: kick, onComplete: () => { s.tweening--; kick(); } });
  }, [kick, reduced, settle]);

  /* The entrance: the cable patches itself into Codex unless the visitor got there first. */
  useEffect(() => {
    if (reduced) return;
    const timer = window.setTimeout(() => { if (!touched.current) connect("codex", "fly"); }, 900);
    return () => window.clearTimeout(timer);
  }, [connect, reduced]);

  const onPlugDown = (event: ReactPointerEvent<SVGGElement>) => {
    const board = boardRef.current;
    if (!board) return;
    event.preventDefault();
    touched.current = true;
    const s = sim.current;
    gsap.killTweensOf(s.E);
    s.tweening = 0;
    s.dragging = true;
    setDragging(true);
    if (connectedRef.current) settle(null);
    kick();
    const move = (e: PointerEvent) => {
      const b = board.getBoundingClientRect();
      s.E.x = Math.min(Math.max(e.clientX - b.left, 8), b.width - 8);
      s.E.y = Math.min(Math.max(e.clientY - b.top, 8), b.height - 8);
      let best: string | null = null, bestD = HIT;
      for (const [id, p] of Object.entries(positions.current)) {
        const d = Math.hypot(p.x - s.E.x, p.y - s.E.y);
        if (d < bestD) { bestD = d; best = id; }
      }
      if (best !== hoverRef.current) { hoverRef.current = best; setHover(best); }
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      s.dragging = false;
      setDragging(false);
      const id = hoverRef.current;
      hoverRef.current = null;
      setHover(null);
      if (id) connect(id, "snap"); else unplug();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
  };

  const jack = JACKS.find((j) => j.id === connected) ?? null;

  return (
    <div className={`c1-bay${dragging ? " is-dragging" : ""}${connected ? " is-live" : ""}`}>
      <div className="c1-board" ref={boardRef}>
        <span className="c1-screw c1-screw--tl" aria-hidden="true" />
        <span className="c1-screw c1-screw--tr" aria-hidden="true" />
        <span className="c1-screw c1-screw--bl" aria-hidden="true" />
        <span className="c1-screw c1-screw--br" aria-hidden="true" />
        <p className="c1-silk c1-board-title" aria-hidden="true">Bus 01 · local runtime · originlm</p>

        <div className="c1-app">
          <p className="c1-silk c1-app-label">Your app <span>· out</span></p>
          <AppScreen jack={jack} reduced={reduced} />
          <span className="c1-app-socket" aria-hidden="true">
            <span className="c1-socket c1-socket--app" ref={appSocketRef} />
            <span className="c1-silk c1-socket-cap">out</span>
          </span>
        </div>

        <ul className="c1-jacks" aria-label="Provider jacks on this example machine">
          {JACKS.map((j) => (
            <li key={j.id}>
              <button
                type="button"
                className={`c1-jack c1-jack--${j.state}${hover === j.id ? " is-hover" : ""}${connected === j.id ? " is-on" : ""}`}
                aria-pressed={connected === j.id}
                aria-label={`Patch your app into ${j.name} (${j.sub})`}
                onClick={() => (connected === j.id ? unplug() : connect(j.id, "fly"))}
              >
                <span className="c1-jack-labels">
                  <span className="c1-silk c1-jack-name">{j.name}</span>
                  <span className="c1-silk c1-jack-sub"><span className="c1-led" aria-hidden="true" />{j.sub}</span>
                </span>
                <span className="c1-socket" ref={(el) => { socketRefs.current[j.id] = el; }} aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>

        <svg className="c1-cable" aria-hidden="true" width="100%" height="100%">
          <path ref={shadowRef} className="c1-cable-shadow" />
          <path ref={pathRef} className="c1-cable-body" />
          <path ref={signalRef} className="c1-cable-signal" />
          <g ref={plugRef} className="c1-plug" onPointerDown={onPlugDown}>
            <rect className="c1-plug-hit" x="-60" y="-26" width="76" height="52" rx="12" />
            <rect className="c1-plug-boot" x="-44" y="-11" width="30" height="22" rx="5" />
            <rect className="c1-plug-collar" x="-16" y="-8" width="8" height="16" rx="1.5" />
            <rect className="c1-plug-tip" x="-9" y="-4" width="10" height="8" rx="2" />
            <line className="c1-plug-grip" x1="-38" y1="-6" x2="-38" y2="6" />
            <line className="c1-plug-grip" x1="-32" y1="-6" x2="-32" y2="6" />
            <line className="c1-plug-grip" x1="-26" y1="-6" x2="-26" y2="6" />
          </g>
        </svg>
      </div>

      <p className="c1-bay-hint">
        <span aria-hidden="true">↳</span> Drag the plug onto a jack, or press one. Example machine; model ids are examples.
      </p>
      <p className="visually-hidden" aria-live="polite">
        {jack ? `Patched into ${jack.name}. ${jack.state === "refused" ? "No eligible route." : "Streaming a sample reply."}` : "Unpatched."}
      </p>

      <Console jack={jack} />
    </div>
  );
}

function Console({ jack }: { jack: Jack | null }) {
  const code = tsCall(jack?.route ?? DEMO_ROUTES[0]);
  const lines = code.split("\n");
  return (
    <figure className={`c1-console${jack ? "" : " is-idle"}`}>
      <figcaption className="c1-console-head">
        <span className="c1-silk">your-app/src/ask.ts</span>
        <span className="c1-chip">real-shaped · SDK {VERSION}</span>
      </figcaption>
      <pre tabIndex={0} aria-label={`SDK call for ${jack?.name ?? "the patched route"}`}>
        <code>
          {lines.map((line, i) => (
            <span key={i} className={`c1-line${line.includes("query:") ? " is-hot" : ""}`}>
              <span className="c1-ln" aria-hidden="true">{String(i + 1).padStart(2, " ")}</span>
              {highlight(line)}
              {"\n"}
            </span>
          ))}
        </code>
      </pre>
    </figure>
  );
}

type Line = { k: string; v: string };

function AppScreen({ jack, reduced }: { jack: Jack | null; reduced: boolean }) {
  const [events, setEvents] = useState<Line[]>([]);
  const [chars, setChars] = useState(0);
  const reply = jack?.route.reply ?? "";

  useEffect(() => {
    if (!jack) { setEvents([]); setChars(0); return; }
    const head: Line[] = [
      { k: "resolve", v: `query "${jack.route.query}"` },
    ];
    if (jack.state === "refused") {
      const all = [...head, { k: "result", v: "[] no eligible route" }];
      if (reduced) { setEvents(all); setChars(0); return; }
      setEvents(head); setChars(0);
      const t = window.setTimeout(() => setEvents(all), 260);
      return () => window.clearTimeout(t);
    }
    const run: Line[] = [...head, { k: "session", v: "read_only" }, { k: "state", v: "running" }];
    const done: Line = { k: "final", v: "completed" };
    if (reduced) { setEvents([...run, done]); setChars(reply.length); return; }
    setEvents([]); setChars(0);
    const timers: number[] = [];
    run.forEach((line, i) => timers.push(window.setTimeout(() => setEvents((prev) => [...prev, line]), 120 + i * 170)));
    const startAt = 120 + run.length * 170 + 120;
    let n = 0;
    let interval = 0;
    timers.push(window.setTimeout(() => {
      interval = window.setInterval(() => {
        n = Math.min(reply.length, n + 2);
        setChars(n);
        if (n >= reply.length) { window.clearInterval(interval); setEvents((prev) => [...prev, done]); }
      }, 26);
    }, startAt));
    return () => { timers.forEach(window.clearTimeout); window.clearInterval(interval); };
  }, [jack, reduced, reply]);

  return (
    <div className="c1-screen" role="group" aria-label="Your app's output (sample)">
      <div className="c1-screen-head">
        <span className="c1-silk">stdout</span>
        <span className="c1-chip c1-chip--amber">sample</span>
      </div>
      {jack ? (
        <>
          <p className="c1-screen-route">
            <span className="c1-k">route</span> {jack.route.providerId}{jack.route.model ? <> · {jack.route.model} <span className="c1-dim">(example)</span></> : null}
          </p>
          <ol className="c1-events">
            {events.map((e, i) => <li key={i}><span className="c1-k">{e.k}</span> {e.v}</li>)}
          </ol>
          {jack.state === "refused" ? (
            <p className="c1-screen-note">{jack.note}</p>
          ) : (
            <p className="c1-screen-text">
              {reply.slice(0, chars)}
              {chars < reply.length && !reduced ? <span className="c1-caret" aria-hidden="true" /> : null}
            </p>
          )}
          {jack.state === "partial" ? <p className="c1-screen-note c1-screen-note--amber">Conditional: {jack.note}</p> : null}
        </>
      ) : (
        <p className="c1-screen-idle">No cable. Patch a jack to call a model.</p>
      )}
    </div>
  );
}
