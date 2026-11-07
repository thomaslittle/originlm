import "./c4.css";
import { useEffect, useRef, useState } from "react";
import markUrl from "../../assets/brand/originlm-mark-white.svg";
import { DEMO_ROUTES, DOCS, ONE_LINER, PROVIDERS, REPO, STATUS, TAGLINE } from "../shared/facts";
import { gsap, prefersReducedMotion, ScrollTrigger, useLenis, usePageChrome } from "../shared/motion";
import { Grants, How, Install, Problem, Providers, Sdk, Sessions, Usage } from "./Apps";
import { Hero } from "./Hero";
import { AppIcon } from "./icons";
import { useDemo, type Demo } from "./demo";
import { DOCK_OF, DOCK_SECTION, WindowManager, scrollToId, useWM, type DockId, type WinId } from "./wm";

const capture = typeof window !== "undefined" && new URLSearchParams(window.location.search).has("still");

function useNarrow() {
  const query = "(max-width: 759px)";
  const [narrow, setNarrow] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const m = window.matchMedia(query);
    const on = () => setNarrow(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  return narrow;
}

export default function Desktop() {
  usePageChrome("OriginLM: the desktop", "c4-body", "#151a21");
  const reduced = prefersReducedMotion();
  const narrow = useNarrow();
  useLenis(true);
  return (
    <WindowManager initial={{ usage: capture ? "open" : "min" }} reduced={reduced} narrow={narrow}>
      <Page reduced={reduced} />
    </WindowManager>
  );
}

function Page({ reduced }: { reduced: boolean }) {
  const demo = useDemo({ capture, reduced });
  const root = useRef<HTMLDivElement>(null);

  /* OS launch on load, then windows cascade open as each app scrolls in. */
  useEffect(() => {
    if (reduced || !root.current) return;
    const ctx = gsap.context(() => {
      gsap.from(".c4-menubar", { y: -30, duration: 0.45, ease: "power3.out" });
      gsap.from(".c4-dock-list", { y: 90, duration: 0.55, delay: 0.25, ease: "back.out(1.4)" });
      gsap.from(".c4-hero-copy > *", { y: 14, opacity: 0, duration: 0.5, stagger: 0.06, ease: "power3.out", delay: 0.1 });
      gsap.from(".c4-hero .c4-shell", { scale: 0.94, y: 18, opacity: 0, duration: 0.5, stagger: 0.12, delay: 0.3, ease: "back.out(1.3)" });
      gsap.utils.toArray<HTMLElement>(".c4-sec").forEach((sec) => {
        gsap.from(sec.querySelectorAll(".c4-sec-head > *"), {
          y: 18, opacity: 0, duration: 0.45, stagger: 0.06, ease: "power3.out",
          scrollTrigger: { trigger: sec, start: "top 82%", once: true },
        });
        gsap.from(sec.querySelectorAll(".c4-shell, .c4-copylist"), {
          y: 36, scale: 0.94, opacity: 0, duration: 0.55, stagger: 0.14, ease: "back.out(1.25)",
          scrollTrigger: { trigger: sec, start: "top 72%", once: true },
        });
      });
    }, root);
    const refresh = window.setTimeout(() => ScrollTrigger.refresh(), 400);
    return () => { window.clearTimeout(refresh); ctx.revert(); };
  }, [reduced]);

  return (
    <div className="c4" ref={root}>
      <a className="c4-skip" href="#main">Skip to content</a>
      <MenuBar demo={demo} />
      <main id="main" tabIndex={-1}>
        <Hero demo={demo} />
        <div className="c4-read-flow">
          <Problem />
          <How />
          <Sdk />
          <Providers />
          <Grants />
          <Sessions capture={capture} reduced={reduced} />
          <Usage />
          <Install capture={capture} reduced={reduced} />
        </div>
      </main>
      <Footer />
      <Dock demo={demo} />
    </div>
  );
}

/* ---------- menu bar ---------- */

function Clock() {
  const fmt = () => new Date().toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
  const [time, setTime] = useState(() => (capture ? "10:41 AM" : fmt()));
  useEffect(() => {
    if (capture) return;
    const id = window.setInterval(() => setTime(fmt()), 10000);
    return () => window.clearInterval(id);
  }, []);
  return <time className="c4-clock">{time}</time>;
}

const NAV: [string, string][] = [["demo", "Demo"], ["how", "How"], ["sdk", "SDK"], ["providers", "Providers"], ["grants", "Grants"], ["install", "Install"]];

function MenuBar({ demo }: { demo: Demo }) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const apps = demo.granted.size > 0 ? 1 : 0;
  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => { if (!menuRef.current?.contains(e.target as Node)) setOpen(false); };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    return () => { document.removeEventListener("pointerdown", onDown); document.removeEventListener("keydown", onKey); };
  }, [open]);
  return (
    <header className="c4-menubar">
      <div className="c4-menu" ref={menuRef}>
        <button type="button" className="c4-menu-btn" aria-expanded={open} aria-controls="c4-menu-list" onClick={() => setOpen((o) => !o)}>
          <img src={markUrl} alt="" width={16} height={16} />
          <span>OriginLM</span>
        </button>
        {open && (
          <ul className="c4-menu-list" id="c4-menu-list">
            <li><span className="c4-menu-about">{TAGLINE}<small>{STATUS}</small></span></li>
            <li><a href="#problem" onClick={() => setOpen(false)}>Just read instead</a></li>
            <li><a href={REPO}>Source on GitHub</a></li>
            <li><a href={DOCS.quickstarts}>Quickstarts</a></li>
            <li><a href="/concepts">All five concepts</a></li>
          </ul>
        )}
      </div>
      <nav className="c4-nav" aria-label="Sections">
        <ul>{NAV.map(([id, label]) => <li key={id}><a href={`#${id}`}>{label}</a></li>)}</ul>
      </nav>
      <div className="c4-status-items">
        <span className="c4-mb-item c4-mb-apps" aria-live="polite"><span className={`c4-led ${apps ? "is-on" : ""}`} aria-hidden="true" />{apps} {apps === 1 ? "app" : "apps"}<span className="c4-mb-word"> connected</span></span>
        <span className="c4-mb-item c4-mb-tokens" title="Sample data, not a real counter"><span className="c4-tag">sample</span><span className="c4-num">{demo.tokens.toLocaleString("en-US")}</span><span className="c4-tok-long"> tokens</span><span className="c4-tok-short" aria-hidden="true"> tok</span></span>
        <Clock />
      </div>
    </header>
  );
}

/* ---------- dock ---------- */

const DOCK_APPS: { id: DockId; label: string }[] = [
  { id: "originlm", label: "OriginLM" }, { id: "app", label: "your-new-app" }, { id: "docs", label: "Docs" },
  { id: "providers", label: "Providers" }, { id: "grants", label: "Grants" }, { id: "sessions", label: "Sessions" },
  { id: "usage", label: "Usage" }, { id: "install", label: "Install" },
];
const DOCK_PROVIDERS = PROVIDERS.filter((p) => p.kind !== "API route");
const DEMO_BY_PROVIDER: Record<string, string> = { codex: "codex", "claude-code": "claude", "gemini-cli": "gemini", ollama: "ollama" };

function Dock({ demo }: { demo: Demo }) {
  const wm = useWM();
  const [toast, setToast] = useState<{ title: string; text: string } | null>(null);
  const toastTimer = useRef(0);
  const lastY = useRef(0);

  /* Autohide: the dock slides away while you scroll down through the reading part,
     and comes back when you scroll up, reach the end, or focus it. */
  useEffect(() => {
    if (wm.reduced) return;
    const onScroll = () => {
      const y = window.scrollY;
      const nearEnd = y + window.innerHeight > document.documentElement.scrollHeight - 160;
      const down = y > lastY.current + 4;
      const up = y < lastY.current - 4;
      if (y < window.innerHeight * 0.5 || nearEnd || up) wm.setDockShown(true);
      else if (down) wm.setDockShown(false);
      lastY.current = y;
    };
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, [wm]);

  const say = (title: string, text: string) => {
    setToast({ title, text });
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 6500);
  };
  useEffect(() => () => window.clearTimeout(toastTimer.current), []);

  const openApp = (id: DockId) => {
    const wins = (Object.keys(DOCK_OF) as WinId[]).filter((w) => DOCK_OF[w] === id);
    scrollToId(DOCK_SECTION[id]);
    for (const w of wins) if (wm.states[w] !== "open") { wm.setState(w, "open"); wm.focus(w); }
    wm.bounce(id);
  };
  const openProvider = (providerId: string) => {
    const p = PROVIDERS.find((x) => x.id === providerId)!;
    const route = DEMO_BY_PROVIDER[providerId];
    if (route) {
      demo.pick(route);
      scrollToId("demo");
      const r = DEMO_ROUTES.find((x) => x.id === route)!;
      say(`${p.name} selected`, `your-new-app now resolves query "${r.query}". ${demo.granted.has(route) ? "Already granted." : "Grant execution in OriginLM to run it."}`);
    } else {
      say(p.name, p.note);
    }
  };

  return (
    <>
      <div className="c4-toasts" aria-live="polite">
        {toast && (
          <div className="c4-toast" role="status">
            <strong>{toast.title}</strong>
            <span>{toast.text}</span>
            <button type="button" className="c4-toast-x" onClick={() => setToast(null)} aria-label="Dismiss notification">×</button>
          </div>
        )}
      </div>
      <nav
        className={`c4-dock ${wm.dockShown ? "" : "is-hidden"}`}
        aria-label="Dock"
        onFocus={() => wm.setDockShown(true)}
        onPointerEnter={() => wm.setDockShown(true)}
      >
        <ul className="c4-dock-list" data-lenis-prevent>
          {DOCK_APPS.map((a) => {
            const wins = (Object.keys(DOCK_OF) as WinId[]).filter((w) => DOCK_OF[w] === a.id);
            const running = wins.some((w) => wm.states[w] === "open");
            const minimized = wins.some((w) => wm.states[w] === "min");
            return (
              <li key={a.id}>
                <button
                  type="button"
                  ref={(el) => wm.registerDock(a.id, el)}
                  className={`c4-dock-btn is-app ${wm.bounced === a.id ? "is-bounce" : ""} ${minimized ? "is-min" : ""}`}
                  data-label={a.label}
                  aria-label={`${a.label}${minimized ? ", minimized" : running ? ", open" : ""}`}
                  onClick={() => openApp(a.id)}
                >
                  <AppIcon id={a.id} />
                  {(running || minimized) && <span className={`c4-dock-dot ${minimized ? "is-min" : ""}`} aria-hidden="true" />}
                </button>
              </li>
            );
          })}
          <li className="c4-dock-sep" aria-hidden="true" />
          {DOCK_PROVIDERS.map((p) => {
            const route = DEMO_BY_PROVIDER[p.id];
            const on = route === demo.route.id;
            return (
              <li key={p.id}>
                <button
                  type="button"
                  className={`c4-dock-btn is-provider is-${p.kind === "Local runtime" ? "local" : "native"} ${on ? "is-current" : ""}`}
                  data-label={p.name}
                  aria-label={`${p.name}, ${p.kind}${route ? ", pick for the demo" : ", show details"}`}
                  aria-pressed={route ? on : undefined}
                  onClick={() => openProvider(p.id)}
                >
                  <span aria-hidden="true">{p.glyph}</span>
                </button>
              </li>
            );
          })}
        </ul>
      </nav>
    </>
  );
}

/* ---------- footer ---------- */

function Footer() {
  return (
    <footer className="c4-foot">
      <div className="c4-foot-in">
        <div className="c4-foot-brand">
          <img src={markUrl} alt="" width={28} height={28} />
          <div>
            <p className="c4-foot-tag">{TAGLINE}</p>
            <p className="c4-foot-one">{ONE_LINER}</p>
          </div>
        </div>
        <ul className="c4-foot-links">
          <li><a href={REPO}>GitHub</a></li>
          <li><a href={DOCS.quickstarts}>Quickstarts</a></li>
          <li><a href={DOCS.matrix}>Support matrix</a></li>
          <li><a href={DOCS.threat}>Threat model</a></li>
          <li><a href={DOCS.license}>MIT license</a></li>
        </ul>
        <p className="c4-foot-fine">
          {STATUS}. Provider and product names are trademarks of their owners; their use here does not imply affiliation. All counters, replies and model ids on this page are sample data.
          {" "}<a href="/concepts">← All five concepts</a>
        </p>
      </div>
    </footer>
  );
}
