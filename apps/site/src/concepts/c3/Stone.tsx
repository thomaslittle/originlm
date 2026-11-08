import "./c3.css";
import { useEffect, useMemo, useRef, useState } from "react";
import { highlight } from "../../lib/highlight";
import { gsap, ScrollTrigger, prefersReducedMotion, useLenis, usePageChrome } from "../shared/motion";
import {
  CONTROL_CENTER, DEMO_ROUTES, DISCOVERED, DISCOVERED_TAIL, DOCS, HOW, INSTALL, OP_LABELS, PITCH, PROBLEM,
  PROVIDERS, QUICKSTART, REPO, SAFETY, STATUS, STATUS_LONG, TAGLINE, VERSION, tsCall,
} from "../shared/facts";
import {
  CAMZ, FOCAL, LIGHTS, createStone, frame, lerpPose, orbit, project, renderStills, toUv,
  type Pose, type Uniforms,
} from "./stone-gl";

/* ---------------------------------------------------------------- beats */

const P = (p: Partial<Pose>): Pose => ({ cx: 0.5, cy: 0.5, size: 0.5, lights: 0, labels: 0, facet: 0, beam: 0, dial: 0, dialAmt: 0, dim: 0, spin: 1, ...p });

type BeatId = "hero" | "problem" | "discover" | "grant" | "call" | "build" | "providers" | "safety" | "control" | "install";
const BEATS: BeatId[] = ["hero", "problem", "discover", "grant", "call", "build", "providers", "safety", "control", "install"];

const DESKTOP: Record<BeatId, Pose> = {
  hero: P({ cx: 0.5, cy: 0.4, size: 0.6, lights: 0.4 }),
  problem: P({ cx: 0.73, cy: 0.5, size: 0.46, lights: 0.12 }),
  discover: P({ cx: 0.68, cy: 0.5, size: 0.5, lights: 1, labels: 1 }),
  grant: P({ cx: 0.66, cy: 0.5, size: 0.7, lights: 0.45, facet: 1, spin: 0.2 }),
  call: P({ cx: 0.2, cy: 0.52, size: 0.44, lights: 0.5, beam: 1 }),
  build: P({ cx: 0.5, cy: 0.36, size: 0.3, lights: 0.6 }),
  providers: P({ cx: 0.62, cy: 0.24, size: 0.24, lights: 1 }),
  safety: P({ cx: 0.75, cy: 0.5, size: 0.48, lights: 0.18, facet: 0.35, spin: 0.5 }),
  control: P({ cx: 0.7, cy: 0.5, size: 0.36, lights: 0.15, dial: 0.62, dialAmt: 1, spin: 0.6 }),
  install: P({ cx: 0.5, cy: 0.26, size: 0.17, lights: 0.12, dim: 0.85, spin: 0.4 }),
};
/* Mobile: after the hero the stone lives in an 84px header band. */
const EMBLEM = P({ cx: 0.5, cy: 0.5, size: 0.8, lights: 0.4 });
const MOBILE: Record<BeatId, Pose> = {
  hero: P({ cx: 0.5, cy: 0.36, size: 0.46, lights: 0.4 }),
  problem: EMBLEM, discover: { ...EMBLEM, lights: 1 }, grant: { ...EMBLEM, facet: 1, spin: 0.2 },
  call: { ...EMBLEM, lights: 0.5 }, build: EMBLEM, providers: { ...EMBLEM, lights: 1 }, safety: EMBLEM,
  control: { ...EMBLEM, dial: 0.62, dialAmt: 1 }, install: { ...EMBLEM, dim: 0.6 },
};

const LIGHT_NAMES = ["Codex", "Claude Code", "Gemini CLI", "Ollama", "Grok", "OpenCode", "Cursor", "LM Studio"];
const ROUTE_LIGHT: Record<string, number> = { codex: 0, claude: 1, gemini: 2, ollama: 3 };
const EMBLEM_H = 84;
const smooth = (t: number) => t * t * (3 - 2 * t);
const facingStep = (Math.PI * 2) / 7;

/* ---------------------------------------------------------------- small parts */

function Copy({ value }: { value: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      className="c3-copy"
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
  return (
    <pre className="c3-code" tabIndex={0} aria-label={label}><code>{highlight(code)}</code></pre>
  );
}

/** Vector stand-in for the stone when WebGL is unavailable. */
function Emblem({ className = "" }: { className?: string }) {
  return (
    <svg className={`c3-emblem ${className}`} viewBox="0 0 120 200" aria-hidden="true">
      <defs>
        <linearGradient id="c3-ob" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#2a3140" /><stop offset="0.45" stopColor="#0b0d12" /><stop offset="1" stopColor="#05060a" />
        </linearGradient>
      </defs>
      <polygon points="60,4 104,58 104,142 60,196 16,142 16,58" fill="url(#c3-ob)" stroke="#6f86ad" strokeWidth="0.8" />
      <polyline points="16,58 60,74 104,58" fill="none" stroke="#9cbcff" strokeWidth="0.7" opacity="0.8" />
      <polyline points="16,142 60,126 104,142" fill="none" stroke="#6f86ad" strokeWidth="0.6" />
      <line x1="60" y1="4" x2="60" y2="74" stroke="#6f86ad" strokeWidth="0.5" />
      <line x1="60" y1="74" x2="60" y2="126" stroke="#cfe0ff" strokeWidth="1.2" opacity="0.9" />
      <line x1="60" y1="126" x2="60" y2="196" stroke="#6f86ad" strokeWidth="0.5" />
    </svg>
  );
}

/* ---------------------------------------------------------------- page */

type Mode = "live" | "still";

export default function Stone() {
  usePageChrome("OriginLM: The Origin Stone", "c3-body", "#050506");
  const [mode, setMode] = useState<Mode>(() => (prefersReducedMotion() ? "still" : "live"));
  const [stills, setStills] = useState<Record<string, string> | null>(null);
  const [noGl, setNoGl] = useState(false);
  const [route, setRoute] = useState(DEMO_ROUTES[0]);
  const [typed, setTyped] = useState(mode === "still" ? Infinity : 0);
  const rootRef = useRef<HTMLDivElement>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const labelsRef = useRef<HTMLDivElement>(null);
  const codeRef = useRef<HTMLDivElement>(null);
  const routeRef = useRef(route);
  routeRef.current = route;
  useLenis(mode === "live");

  const code = useMemo(() => tsCall(route), [route]);

  /* Stills: one pass of the same shader, rendered to images. */
  useEffect(() => {
    if (mode !== "still") return;
    const w = Math.min(window.innerWidth, 1600);
    const h = Math.max(560, Math.round(window.innerHeight * 0.92));
    const sq = 560;
    const urls = renderStills([
      { pose: { ...(window.innerWidth < 760 ? MOBILE.hero : DESKTOP.hero) }, width: w, height: h },
      { pose: P({ size: 0.5, lights: 1 }), width: sq, height: sq },
      { pose: P({ size: 0.72, lights: 0.45, facet: 1 }), width: sq, height: sq, rot: Math.PI / 2 },
      { pose: P({ cx: 0.3, size: 0.5, lights: 0.5, beam: 1 }), width: sq, height: sq, beamTo: [0.52, 0] },
      { pose: P({ size: 0.42, lights: 0.15, dial: 0.62, dialAmt: 1 }), width: sq, height: sq },
      { pose: P({ size: 0.34, lights: 0.12, dim: 0.85 }), width: sq, height: Math.round(sq * 0.6) },
    ]);
    if (!urls) { setNoGl(true); return; }
    const [hero, discover, grant, call, control, install] = urls;
    setStills({ hero, discover, grant, call, control, install });
  }, [mode]);

  /* Live: the fixed canvas and the scroll choreography. */
  useEffect(() => {
    if (mode !== "live") return;
    const canvas = canvasRef.current!;
    const stage = stageRef.current!;
    const stone = createStone(canvas);
    if (!stone) { setMode("still"); return; }

    const sections = BEATS.map((id) => document.getElementById(`c3-${id}`)!);
    const labels = Array.from(labelsRef.current!.children) as HTMLElement[];
    let anchors: number[] = [];
    let mobile = window.innerWidth < 760;
    let emblem = false;
    let W = 0, H = 0;
    const measure = () => {
      mobile = window.innerWidth < 760;
      anchors = sections.map((el) => {
        const top = el.getBoundingClientRect().top + window.scrollY;
        return Math.max(0, top + el.offsetHeight / 2 - window.innerHeight / 2);
      });
      anchors[0] = 0;
      setSize();
    };
    const setSize = () => {
      W = window.innerWidth;
      H = mobile && emblem ? EMBLEM_H : window.innerHeight;
      stone.resize(W, H, Math.min(window.devicePixelRatio || 1, mobile ? 1.5 : 1.75));
    };
    measure();

    const target = (): Pose => {
      const set = mobile ? MOBILE : DESKTOP;
      const y = window.scrollY;
      if (y <= anchors[0]) return set[BEATS[0]];
      for (let i = 0; i < anchors.length - 1; i++) {
        if (y < anchors[i + 1]) {
          const span = Math.max(1, anchors[i + 1] - anchors[i]);
          // Hold each pose around its anchor, travel in between.
          const t = Math.min(1, Math.max(0, ((y - anchors[i]) / span - 0.2) / 0.6));
          return lerpPose(set[BEATS[i]], set[BEATS[i + 1]], smooth(t));
        }
      }
      return set[BEATS[BEATS.length - 1]];
    };

    let pose = target();
    let rotY = 0.6;
    let mouse: [number, number] = [0, 0];
    let mouseGoal: [number, number] = [0, 0];
    let mousePx: [number, number] | null = null;
    const lights = new Float32Array(LIGHTS * 4);
    const onMove = (e: PointerEvent) => {
      mousePx = [e.clientX, e.clientY];
      mouseGoal = [(e.clientX / window.innerWidth) * 2 - 1, 1 - (e.clientY / window.innerHeight) * 2];
    };
    const onLeave = () => { mousePx = null; mouseGoal = [0, 0]; };
    window.addEventListener("pointermove", onMove, { passive: true });
    document.addEventListener("pointerleave", onLeave);
    const onResize = () => { measure(); };
    window.addEventListener("resize", onResize);
    ScrollTrigger.addEventListener("refresh", measure);

    let last = performance.now();
    let clock = 8;
    const tick = () => {
      if (document.hidden) return;
      const now = performance.now();
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      clock += dt;

      const wantEmblem = mobile && window.scrollY > window.innerHeight * 0.55;
      if (wantEmblem !== emblem) {
        emblem = wantEmblem;
        stage.classList.toggle("is-emblem", emblem);
        setSize();
      }
      const goal = mobile && !emblem ? MOBILE.hero : target();
      const k = 1 - Math.exp(-dt * 5);
      pose = lerpPose(pose, goal, k);
      mouse = [mouse[0] + (mouseGoal[0] - mouse[0]) * k, mouse[1] + (mouseGoal[1] - mouse[1]) * k];
      rotY += dt * 0.2 * pose.spin;

      // Grant: settle on the facet that faces you (the stone is 7-fold symmetric).
      let diff = (Math.PI / 2 - rotY) % facingStep;
      if (diff > facingStep / 2) diff -= facingStep;
      if (diff < -facingStep / 2) diff += facingStep;
      const rot = rotY + diff * Math.min(1, pose.facet) + mouse[0] * 0.18 * (1 - pose.facet);

      const { shift, scale } = frame(pose, W / H);
      const mouseWorld: [number, number] | null = mousePx && !emblem
        ? (() => { const [ux, uy] = toUv(mousePx[0], mousePx[1], W, H); return [ux * CAMZ / FOCAL, uy * CAMZ / FOCAL]; })()
        : null;
      orbit(lights, clock, pose, shift, scale, mouseWorld);

      // Call: the chosen route's light burns brighter; the beam runs from the runtime to your code.
      const chosen = ROUTE_LIGHT[routeRef.current.id] ?? 0;
      lights[chosen * 4 + 3] *= 1 + pose.beam * 1.6;
      const sc: [number, number] = [shift[0] * FOCAL / CAMZ, shift[1] * FOCAL / CAMZ];
      let beamB: [number, number] = [sc[0] + 0.6, sc[1]];
      const panel = codeRef.current;
      if (panel && pose.beam > 0.01 && !emblem) {
        const r = panel.getBoundingClientRect();
        beamB = mobile ? toUv(r.left + r.width / 2, r.top, W, H) : toUv(r.left, r.top + Math.min(r.height / 2, 160), W, H);
      }
      const u: Uniforms = {
        shift, scale, rotY: rot, tilt: 0.12 + mouse[1] * 0.06, mouse, lights,
        facet: pose.facet, beam: emblem ? 0 : pose.beam, beamA: sc, beamB,
        dial: pose.dial, dialAmt: pose.dialAmt, dim: pose.dim, time: clock,
      };
      stone.draw(u);

      // DOM labels ride the same orbit maths.
      for (let i = 0; i < labels.length; i++) {
        const el = labels[i];
        const a = lights[i * 4 + 3] * pose.labels * (mobile ? 0 : 1);
        if (a < 0.02) { if (el.style.opacity !== "0") el.style.opacity = "0"; continue; }
        const [px, py] = project(lights[i * 4], lights[i * 4 + 1], lights[i * 4 + 2], W, H);
        const behind = lights[i * 4 + 2] < -0.15 * scale;
        el.style.opacity = String(Math.min(1, a) * (behind ? 0.62 : 1));
        el.style.transform = `translate3d(${px.toFixed(1)}px, ${py.toFixed(1)}px, 0)`;
      }
    };
    gsap.ticker.add(tick);

    /* Copy settles in like a postcard turning over. */
    const ctx = gsap.context(() => {
      gsap.utils.toArray<HTMLElement>(".c3-rv").forEach((el) => {
        gsap.fromTo(el, { autoAlpha: 0, y: 28 }, {
          autoAlpha: 1, y: 0, duration: 0.9, ease: "power3.out",
          scrollTrigger: { trigger: el, start: "top 86%", toggleActions: "play none none reverse" },
        });
      });
      // SDK cards orbit the stone as you scroll through Build.
      const cards = gsap.utils.toArray<HTMLElement>(".c3-card");
      const build = document.getElementById("c3-build");
      if (build && cards.length && window.innerWidth >= 760) {
        ScrollTrigger.create({
          trigger: build, start: "top bottom", end: "bottom top", scrub: true,
          onUpdate: (self) => {
            // An ellipse around the stone: the front of the orbit is below it.
            const base = (self.progress - 0.5) * 0.8;
            cards.forEach((card, i) => {
              const a = base + (i - 1) * 1.2;
              const x = Math.sin(a) * 33;
              const y = Math.cos(a) * 24 - 10;
              const s = 0.86 + 0.14 * Math.cos(a);
              card.style.transform = `translate(-50%, -50%) translate(${x}vw, ${y}vh) scale(${s.toFixed(3)})`;
              card.style.zIndex = String(10 + Math.round(Math.cos(a) * 10));
            });
          },
        });
      }
      // Typed reply when the call beat arrives.
      const call = document.getElementById("c3-call");
      if (call) ScrollTrigger.create({ trigger: call, start: "top 55%", onEnter: () => setTyped(0.0001), onEnterBack: () => setTyped(0.0001) });
    }, rootRef);
    requestAnimationFrame(() => ScrollTrigger.refresh());

    return () => {
      gsap.ticker.remove(tick);
      ctx.revert();
      window.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerleave", onLeave);
      window.removeEventListener("resize", onResize);
      ScrollTrigger.removeEventListener("refresh", measure);
      stone.destroy();
    };
  }, [mode]);

  /* Streams the sample reply, character by character (live mode only). */
  useEffect(() => {
    if (!Number.isFinite(typed) || typed <= 0) return;
    const text = route.reply;
    if (typed >= text.length) return;
    const id = window.setTimeout(() => setTyped((n) => Math.floor(n) + 2), 28);
    return () => window.clearTimeout(id);
  }, [typed, route]);

  const pickRoute = (r: typeof route) => { setRoute(r); setTyped(mode === "still" ? Infinity : 0.0001); };
  const still = mode === "still";
  const replyShown = still || !Number.isFinite(typed) ? route.reply : route.reply.slice(0, Math.floor(typed));
  const fig = (key: string, alt: string) =>
    still ? (
      <figure className="c3-fig" aria-hidden="true">
        {stills?.[key] ? <img src={stills[key]} alt={alt} /> : noGl ? <Emblem /> : <span className="c3-fig-wait" />}
      </figure>
    ) : null;

  return (
    <div ref={rootRef} className={`c3 ${still ? "c3--still" : "c3--live"}`}>
      <a className="c3-skip" href="#main">Skip to content</a>

      {!still && (
        <div ref={stageRef} className="c3-stage" aria-hidden="true">
          <canvas ref={canvasRef} className="c3-canvas" />
          <div ref={labelsRef} className="c3-labels">
            {LIGHT_NAMES.map((name) => <span key={name} className="c3-label">{name}</span>)}
          </div>
        </div>
      )}

      <header className="c3-top">
        <a className="c3-brand" href="#main" aria-label="OriginLM, top of page">
          <span className="c3-brand-mark" aria-hidden="true" />ORIGINLM
        </a>
        <nav aria-label="Primary" className="c3-nav">
          <a href="#c3-discover">How</a>
          <a href="#c3-build">SDK</a>
          <a href="#c3-install">Install</a>
          <a href={REPO}>GitHub</a>
        </nav>
      </header>

      <main id="main">
        {/* 00 Hero */}
        <section id="c3-hero" className="c3-beat c3-hero" aria-labelledby="c3-h1">
          {still && (
            <div className="c3-hero-still" aria-hidden="true">
              {stills?.hero ? <img src={stills.hero} alt="" /> : noGl ? <Emblem className="c3-emblem--hero" /> : null}
            </div>
          )}
          <div className="c3-hero-copy">
            <p className="c3-eyebrow">Local runtime · {VERSION}</p>
            <h1 id="c3-h1" className="c3-h1">
              <span>Every model you own.</span>
              <span>Callable from your app.</span>
            </h1>
            <p className="c3-hero-sub">One local runtime between the software you write and the AI already on your machine.</p>
            <p className="c3-hero-cta">
              <a className="c3-btn" href="#c3-install">Install the alpha</a>
              <a className="c3-link" href={DOCS.quickstarts}>Read the quickstarts</a>
            </p>
          </div>
          {!still && <p className="c3-cue" aria-hidden="true"><span />Scroll</p>}
        </section>

        {/* 01 Problem */}
        <section id="c3-problem" className="c3-beat c3-left" aria-labelledby="c3-problem-h">
          <div className="c3-postcard c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">01</span> The problem</p>
            <h2 id="c3-problem-h" className="c3-h2">{PROBLEM.line}</h2>
            <ul className="c3-burdens" aria-label="What every integration repeats">
              {PROBLEM.burdens.map((b) => <li key={b}><span aria-hidden="true">×</span> {b}</li>)}
            </ul>
            <p className="c3-small">{PROBLEM.fix}</p>
          </div>
        </section>

        {/* 02 Discover */}
        <section id="c3-discover" className="c3-beat c3-left c3-with-fig" aria-labelledby="c3-discover-h">
          {fig("discover", "")}
          <div className="c3-postcard c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">02</span> {HOW[0].verb} · <code>{HOW[0].method}</code></p>
            <h2 id="c3-discover-h" className="c3-h2">Lights appear for what is already installed.</h2>
            <p className="c3-text">{HOW[0].body}</p>
            <p className="c3-readout">{DISCOVERED.join(" · ")} · {DISCOVERED_TAIL}</p>
          </div>
        </section>

        {/* 03 Grant */}
        <section id="c3-grant" className="c3-beat c3-left c3-with-fig" aria-labelledby="c3-grant-h">
          {fig("grant", "")}
          <div className="c3-postcard c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">03</span> {HOW[1].verb} · <code>{HOW[1].method}</code></p>
            <h2 id="c3-grant-h" className="c3-h2">Discovery is not permission.</h2>
            <p className="c3-text">{HOW[1].body}</p>
            <p className="c3-readout">→ requires_host_approval</p>
          </div>
        </section>

        {/* 04 Call */}
        <section id="c3-call" className="c3-beat c3-call c3-with-fig" aria-labelledby="c3-call-h">
          {fig("call", "")}
          <div className="c3-call-copy c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">04</span> {HOW[2].verb} · <code>{HOW[2].method}</code></p>
            <h2 id="c3-call-h" className="c3-h2">Call it from your app.</h2>
            <p className="c3-text">{HOW[2].body}</p>
          </div>
          <div ref={codeRef} className="c3-panel c3-rv">
            <div className="c3-panel-head">
              <span className="c3-panel-file">your-app.ts</span>
              <div className="c3-routes" role="group" aria-label="Route to call">
                {DEMO_ROUTES.map((r) => (
                  <button key={r.id} type="button" className="c3-route" aria-pressed={route.id === r.id} onClick={() => pickRoute(r)}>
                    {r.name}
                  </button>
                ))}
              </div>
            </div>
            <Code code={code} label={`TypeScript: call ${route.name} from your app`} />
            <div className="c3-out" aria-live="polite">
              <span className="c3-out-k">text_delta · example output · {route.name} / {route.model}</span>
              <p className="c3-out-text">{replyShown}<span className="c3-caret" aria-hidden="true" /></p>
            </div>
            <p className="c3-panel-foot">{HOW[2].detail}</p>
          </div>
        </section>

        {/* 05 Build */}
        <section id="c3-build" className="c3-beat c3-build" aria-labelledby="c3-build-h">
          <div className="c3-build-head c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">05</span> Build</p>
            <h2 id="c3-build-h" className="c3-h2">Three SDKs, one protocol.</h2>
            <p className="c3-small">TypeScript, Rust and Python clients speak the same JSON-RPC protocol to the same runtime.</p>
          </div>
          <div className="c3-orbit">
            {([
              ["TypeScript", INSTALL[0].cmd, QUICKSTART.node],
              ["Python", INSTALL[1].cmd, QUICKSTART.python],
              ["Rust", INSTALL[2].cmd, QUICKSTART.rust],
            ] as const).map(([lang, cmd, snippet]) => (
              <article key={lang} className="c3-card" aria-label={`${lang} quickstart`}>
                <h3 className="c3-card-h">{lang}</h3>
                <p className="c3-card-cmd"><code>{cmd}</code><Copy value={cmd} /></p>
                <Code code={snippet} label={`${lang} quickstart code`} />
              </article>
            ))}
          </div>
        </section>

        {/* 06 Providers */}
        <section id="c3-providers" className="c3-beat c3-providers" aria-labelledby="c3-providers-h">
          <div className="c3-prov-head c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">06</span> Providers</p>
            <h2 id="c3-providers-h" className="c3-h2">Evidence-based. Operation-specific.</h2>
          </div>
          <ul className="c3-prov c3-rv" aria-label="Providers and the operations each supports">
            {PROVIDERS.map((p) => (
              <li key={p.id} className="c3-prov-item">
                <span className="c3-prov-name">{p.name}</span>
                <span className="c3-prov-kind">{p.kind}</span>
                <span className="c3-ops">
                  {OP_LABELS.map(({ key, label }) => (
                    <span key={key} className={`c3-op c3-op--${p.ops[key]}`} title={`${label}: ${p.ops[key]}`}>
                      <span className="c3-vh">{label}: {p.ops[key] === "none" ? "no" : p.ops[key]}. </span>
                    </span>
                  ))}
                </span>
              </li>
            ))}
          </ul>
          <p className="c3-prov-key c3-rv">
            <span><i className="c3-op c3-op--yes" aria-hidden="true" /> yes</span>
            <span><i className="c3-op c3-op--partial" aria-hidden="true" /> partial</span>
            <span><i className="c3-op c3-op--none" aria-hidden="true" /> no</span>
            <span className="c3-prov-ops">{OP_LABELS.map((o) => o.label).join(" · ")}</span>
            <a className="c3-link" href={DOCS.matrix}>Support matrix</a>
          </p>
        </section>

        {/* 07 Grants & safety */}
        <section id="c3-safety" className="c3-beat c3-left" aria-labelledby="c3-safety-h">
          <div className="c3-postcard c3-postcard--wide c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">07</span> Grants &amp; safety</p>
            <h2 id="c3-safety-h" className="c3-h2">The installed client keeps its keys.</h2>
            <dl className="c3-safety">
              {SAFETY.map(([term, text]) => (
                <div key={term}><dt>{term}</dt><dd>{text}</dd></div>
              ))}
            </dl>
            <p className="c3-hero-cta">
              <a className="c3-link" href={DOCS.threat}>Threat model</a>
              <a className="c3-link" href={DOCS.privacy}>Privacy boundary</a>
            </p>
          </div>
        </section>

        {/* 08 Control center: brief, late */}
        <section id="c3-control" className="c3-beat c3-left c3-control c3-with-fig" aria-labelledby="c3-control-h">
          {fig("control", "")}
          <div className="c3-postcard c3-postcard--small c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">08</span> Control</p>
            <h2 id="c3-control-h" className="c3-h3">{TAGLINE.split(".")[0]}, too.</h2>
            <p className="c3-small">{CONTROL_CENTER.line}</p>
            <p className="c3-readout">{CONTROL_CENTER.views.join(" · ")}</p>
            <p className="c3-small">{CONTROL_CENTER.note} The dial reads an example: 62% of a 5-hour window.</p>
            <p className="c3-cmd"><code>{CONTROL_CENTER.run}</code><Copy value={CONTROL_CENTER.run} /></p>
          </div>
        </section>

        {/* 09 Install */}
        <section id="c3-install" className="c3-beat c3-install" aria-labelledby="c3-install-h">
          {still && <figure className="c3-fig c3-fig--mark" aria-hidden="true">{stills?.install ? <img src={stills.install} alt="" /> : noGl ? <Emblem /> : null}</figure>}
          <div className="c3-install-in c3-rv">
            <p className="c3-eyebrow"><span className="c3-n">09</span> Install</p>
            <h2 id="c3-install-h" className="c3-h2">{PITCH.headline}</h2>
            <ul className="c3-cmds" aria-label="Install commands">
              {INSTALL.map((c) => (
                <li key={c.id} className="c3-cmd">
                  <span className="c3-cmd-k">{c.label}</span>
                  <code>{c.cmd}</code>
                  <Copy value={c.cmd} />
                </li>
              ))}
            </ul>
            <p className="c3-small">{STATUS_LONG}</p>
            <p className="c3-hero-cta">
              <a className="c3-btn" href={DOCS.quickstarts}>SDK quickstarts</a>
              <a className="c3-link" href={REPO}>Source on GitHub</a>
            </p>
          </div>
        </section>
      </main>

      <footer className="c3-foot">
        <p className="c3-foot-brand"><span className="c3-brand-mark" aria-hidden="true" />ORIGINLM <span>{STATUS}</span></p>
        <nav aria-label="Footer" className="c3-foot-nav">
          <a href={REPO}>GitHub</a>
          <a href={DOCS.security}>Security</a>
          <a href={DOCS.privacy}>Privacy</a>
          <a href={DOCS.license}>License</a>
          <a href="/concepts">← All concepts</a>
        </nav>
        <p className="c3-foot-note">Concept 03, The Origin Stone. A design exploration, not the published site. Provider names are trademarks of their owners.</p>
      </footer>
    </div>
  );
}
