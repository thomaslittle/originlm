import { Component, type ReactNode, Suspense, lazy, useEffect, useRef, useState } from "react";
import { ArrowRight } from "../components/Icon";
import { LAYERS, type Layer } from "../data/layers";
import HeroFallbackScene from "../scene/HeroFallbackScene";

const HeroScene = lazy(() => import("../scene/HeroScene"));

function webGLSupport() {
  try {
    const canvas = document.createElement("canvas");
    const context = canvas.getContext("webgl2");
    if (!context) return null;
    const debug = context.getExtension("WEBGL_debug_renderer_info");
    const renderer = debug ? String(context.getParameter(debug.UNMASKED_RENDERER_WEBGL)) : "";
    context.getExtension("WEBGL_lose_context")?.loseContext();
    return { software: /swiftshader|llvmpipe|software|basic render/i.test(renderer) };
  } catch {
    return null;
  }
}

class SceneBoundary extends Component<{ children: ReactNode; onFailure: () => void }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch() { this.props.onFailure(); }
  render() { return this.state.failed ? null : this.props.children; }
}

export function HeroStage() {
  const stageRef = useRef<HTMLDivElement>(null);
  const introPlayed = useRef(false);
  const labelRefs = useRef<(HTMLDivElement | null)[]>([]);
  const [enabled, setEnabled] = useState(false);
  const [ready, setReady] = useState(false);
  const [active, setActive] = useState<Layer | null>(null);
  const [graphicsMode, setGraphicsMode] = useState<"webgl" | "css">("webgl");
  const [quality, setQuality] = useState<"full" | "light">("full");
  const [staticMode, setStaticMode] = useState(false);
  const [paused, setPaused] = useState(false);
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    const node = stageRef.current;
    if (!node) return;
    let intersecting = true;
    const update = () => setVisible(intersecting && !document.hidden);
    const observer = new IntersectionObserver(([entry]) => {
      intersecting = entry.isIntersecting;
      update();
    });
    observer.observe(node);
    document.addEventListener("visibilitychange", update);
    return () => { observer.disconnect(); document.removeEventListener("visibilitychange", update); };
  }, []);

  useEffect(() => {
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const graphics = webGLSupport();
    setGraphicsMode(graphics ? "webgl" : "css");

    const motionChanged = () => setStaticMode(motion.matches);
    motionChanged();
    motion.addEventListener("change", motionChanged);

    setQuality(
      graphics && !graphics.software && window.innerWidth >= 900 && (navigator.hardwareConcurrency ?? 4) >= 4 && !new URLSearchParams(window.location.search).has("light")
        ? "full"
        : "light",
    );

    let cancelled = false;
    const start = () => {
      if (!cancelled) setEnabled(true);
    };
    // Let the hero copy paint before the 3D chunk is requested.
    const timer = window.setTimeout(start, 450);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      motion.removeEventListener("change", motionChanged);
    };
  }, []);

  return (
    <figure className="hero-figure">
      <div className="hero-stage" ref={stageRef} data-renderer={graphicsMode} data-fallback={staticMode || undefined} data-static={staticMode || paused || undefined} data-ready={ready && !paused && !staticMode && visible || undefined}>
        <picture>
        <source media="(max-width: 40rem)" srcSet="/scene/poster-mobile.webp" />
        <img
          className="stage-fallback"
          src="/scene/poster.webp"
          alt="The OriginLM stack, exploded: your app, SDK, transport, runtime and providers, shown with real OriginLM screens in example mode."
          width={1440}
          height={1000}
          fetchPriority="high"
        />
        </picture>

        {enabled && !staticMode && !paused && visible && (graphicsMode === "css" ? (
          <HeroFallbackScene stageEl={stageRef.current} onReady={() => setReady(true)} onHover={setActive} />
        ) : (
          <SceneBoundary onFailure={() => setGraphicsMode("css")}>
          <Suspense fallback={null}>
            <HeroScene
              stageEl={stageRef.current}
              introPlayed={introPlayed}
              labels={labelRefs}
              quality={quality}
              onHover={setActive}
              onReady={() => setReady(true)}
              onFailure={() => setGraphicsMode("css")}
            />
          </Suspense>
          </SceneBoundary>
        ))}

        <div className="stage-labels" aria-hidden="true">
          {LAYERS.map((layer, index) => (
            <div
              key={layer.id}
              ref={(node) => { labelRefs.current[index] = node; }}
              className="layer-label"
              data-active={active?.id === layer.id}
              style={{ opacity: 0 }}
            >
              {layer.label}
            </div>
          ))}
        </div>

        <div className="layer-tooltip" data-visible={active !== null} role="status" aria-live="polite">
          {active ? (
            <>
              <strong>{active.title}.</strong> {active.detail}
            </>
          ) : (
            <span>&nbsp;</span>
          )}
        </div>

        <ul className="visually-hidden">
          {LAYERS.map((layer) => (
            <li key={layer.id}>
              {layer.title}: {layer.detail}
            </li>
          ))}
        </ul>
      </div>
      {!staticMode && enabled && (
        <button className="stage-motion" type="button" aria-pressed={paused}
          onClick={() => setPaused(!paused)}>
          {paused ? "Play animation" : "Pause animation"}
        </button>
      )}
      <ol className="static-layer-names" aria-label="Layers in the OriginLM stack">
        {LAYERS.map(layer => <li key={layer.id}>{layer.label}</li>)}
      </ol>
      <figcaption>
        {staticMode
          ? "The OriginLM stack, one runtime from the providers up to your app."
          : "Scroll to separate your app, SDK, transport, runtime and providers."}
      </figcaption>
      <details className="layer-key">
        <summary>Explore the five layers</summary>
        <dl>{LAYERS.map((layer) => <div key={layer.id}><dt>{layer.title}</dt><dd>{layer.detail}</dd></div>)}</dl>
      </details>
    </figure>
  );
}

export function Hero() {
  return (
    <div className="hero-scroll">
    <header className="hero" id="top">
      <div className="container intro-grid">
        <div className="intro-copy">
          <h1>
            <span className="line">See your AI.</span>
            <span className="line line-second">Build with it.</span>
          </h1>

          <p className="lead hero-sub">
            Your AI dashboard and developer platform, powered by one local runtime.
          </p>

          <div className="hero-actions">
            <a className="btn btn-primary" href="#install">
              Get OriginLM <ArrowRight size={17} />
            </a>
            <a className="btn btn-ghost" href="#developers">
              Build with it
            </a>
          </div>

          <p className="hero-note">
            MIT-licensed source preview. No OriginLM account or telemetry.
          </p>
        </div>

        <HeroStage />
      </div>
    </header>
    </div>
  );
}
