/* The fixed stage behind the page. The canvas and its labels are decorative (aria-hidden):
   everything the scene shows also exists as text in the sections. The WebGL chunk is
   requested only after the HTML has painted; until it arrives the page is plain dark
   ground with real type on it. Reduced motion, no WebGL, or a failed context fall back to
   the static layout with pre-rendered stills (html:not(.motion)). */
import { useEffect, useRef } from "react";
import { anchorSink, sceneState, type Anchor } from "./state";
import { STORY_NODES } from "../data/story";

type SceneHandle = { dispose: () => void; stats: () => unknown };
declare global {
  interface Window { __originlmScene?: SceneHandle & { state: typeof sceneState } }
}

/** Probe WebGL2 once; report software renderers so they get the light tier. From the
    2026-11 HeroStage, kept because it held up. */
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

export function fallBackToStatic(reason: string) {
  const root = document.documentElement;
  if (!root.classList.contains("motion")) return;
  root.classList.remove("motion");
  root.classList.add("scene-fallback");
  root.dataset.sceneFallback = reason;
  window.dispatchEvent(new CustomEvent("originlm:static"));
}

export function SceneLayer() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const labels = useRef<Record<string, HTMLElement | null>>({});

  useEffect(() => {
    if (!document.documentElement.classList.contains("motion")) return;
    const support = webGLSupport();
    if (!support) { fallBackToStatic("webgl2 unavailable"); return; }
    const params = new URLSearchParams(window.location.search);
    const narrow = window.innerWidth < 760;
    const lowHardware = (navigator.hardwareConcurrency ?? 8) <= 4 || ((navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? 8) <= 4;
    const tier = params.get("tier") === "high" ? "high" : params.get("tier") === "low" || support.software || narrow || lowHardware ? "low" : "high";

    anchorSink.write = (anchors: Record<string, Anchor>) => {
      // The labels sink with the canvas; a label the scene did not place this frame is hidden.
      const layer = stage.current?.querySelector<HTMLElement>(".stage-labels");
      if (layer) layer.style.opacity = (anchors.stage?.alpha ?? 1).toFixed(3);
      for (const [key, el] of Object.entries(labels.current)) {
        if (!el) continue;
        // In the hero the display type owns the left half; the node labels would print
        // through the headline, so they wait until the story begins.
        if (key.startsWith("node") && sceneState.heroOut < 0.1) { el.style.opacity = "0"; continue; }
        const anchor = anchors[key];
        if (!anchor || anchor.alpha < 0.02) { el.style.opacity = "0"; continue; }
        let x = anchor.x;
        // Notes are aligned to their anchor, then kept inside the viewport.
        const align = el.dataset.align;
        if (align) {
          const width = el.offsetWidth;
          x = align === "end" ? x - width - 22 : align === "center" ? x - width / 2 : x;
          x = Math.max(12, Math.min(window.innerWidth - width - 12, x));
        }
        el.style.transform = `translate3d(${x.toFixed(1)}px, ${anchor.y.toFixed(1)}px, 0)`;
        el.style.opacity = anchor.alpha.toFixed(3);
        if (key.startsWith("node")) el.dataset.side = anchor.scale < 0 ? "left" : "right";
      }
    };

    let scene: SceneHandle | undefined;
    let cancelled = false;
    const boot = () => import("./eye-scene").then(({ createEyeScene }) => {
      if (cancelled || !canvas.current) return;
      try {
        scene = createEyeScene(canvas.current, {
          tier,
          capture: params.has("capture"),
          onFirstFrame: () => { performance.mark("scene-live"); stage.current?.setAttribute("data-live", ""); },
          onFailure: (reason) => fallBackToStatic(reason),
        });
        window.__originlmScene = Object.assign(scene, { state: sceneState });
      } catch (error) {
        fallBackToStatic(error instanceof Error ? error.message : "scene failed");
      }
    }).catch(() => fallBackToStatic("scene chunk failed to load"));

    // Where the two big headlines end, measured from their words (not their boxes).
    const measure = () => {
      const edge = (selector: string) => Math.max(0, ...[...document.querySelectorAll<HTMLElement>(`${selector} .w`)].map((w) => w.getBoundingClientRect().right));
      sceneState.clearOf = { hero: edge(".hero-title"), cta: edge(".final-title") };
    };
    measure();
    document.fonts?.ready.then(measure);
    window.addEventListener("resize", measure);

    // After the first paint, so the headline and copy never wait on WebGL.
    let timer = 0;
    const frame = requestAnimationFrame(() => { timer = window.setTimeout(boot, 60); });
    return () => {
      cancelled = true;
      cancelAnimationFrame(frame); window.clearTimeout(timer);
      window.removeEventListener("resize", measure);
      anchorSink.write = null;
      scene?.dispose();
    };
  }, []);

  const ref = (key: string) => (node: HTMLElement | null) => { labels.current[key] = node; };
  return (
    <div className="stage" ref={stage} aria-hidden="true">
      <canvas ref={canvas} className="stage-canvas" />
      <div className="stage-labels">
        {STORY_NODES.map((node, index) => (
          <span key={node.id} ref={ref(`node${index}`)} className="node-label">
            <b>{node.name}</b>
            <i>{node.auth}</i>
          </span>
        ))}
        <span ref={ref("machine")} className="scene-note scene-note--machine" data-align="start">this machine</span>
        <span ref={ref("provider")} className="scene-note scene-note--provider" data-align="center">to each route’s own provider, under its terms</span>
        <span ref={ref("local")} className="scene-note scene-note--local" data-align="end">local route: stays on this device</span>
      </div>
    </div>
  );
}
