/* The scroll director, a lazy chunk loaded after first paint, only when motion is on.

   Pinning is CSS `position: sticky` (React keeps ownership of its DOM; ScrollTrigger's pin
   would wrap nodes React renders). ScrollTrigger turns scroll into the scene's phase
   numbers; Lenis smooths wheel input. One easing family everywhere: expo / quint out. */
import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";
import { sceneState, storyStore } from "../scene/state";

const EXPO = "expo.out";

export function startDirector() {
  gsap.registerPlugin(ScrollTrigger);
  gsap.defaults({ ease: EXPO, duration: 0.9 });
  const cleanups: (() => void)[] = [];
  const fine = window.matchMedia("(pointer: fine)").matches;

  /* ---- smooth scroll (wheel only; touch keeps native momentum) ---- */
  const lenis = new Lenis({ lerp: 0.09, smoothWheel: true, wheelMultiplier: 0.95 });
  lenis.on("scroll", ScrollTrigger.update);
  const raf = (time: number) => lenis.raf(time * 1000);
  gsap.ticker.add(raf);
  gsap.ticker.lagSmoothing(0);
  cleanups.push(() => { gsap.ticker.remove(raf); lenis.destroy(); });
  // In-page links keep native jumps: gliding through four screens of scrub to reach the
  // matrix would be slower, not smoother.

  /* ---- scene phases ---- */
  const progress = (trigger: string, start: string, end: string, set: (value: number) => void) => {
    const el = document.querySelector(trigger);
    if (!el) return;
    const st = ScrollTrigger.create({ trigger: el, start, end, onUpdate: (self) => set(self.progress), onRefresh: (self) => set(self.progress) });
    cleanups.push(() => st.kill());
  };
  const noserver = document.querySelector<HTMLElement>("#how .noserver-track");
  progress("#top", "top top", "bottom top", (v) => { sceneState.heroOut = v; });
  // Custom properties are written on the smallest element that reads them: a variable on a
  // large subtree would restyle every descendant on every scroll frame.
  progress("#story", "top top", "bottom bottom", (v) => { storyStore.set(v); });
  progress("#how .noserver-track", "top top", "bottom bottom", (v) => { sceneState.pull = v; noserver?.style.setProperty("--pull", v.toFixed(4)); });
  progress("#how .arch", "top 85%", "bottom 30%", (v) => { sceneState.sink = Math.min(1, v * 1.6); });

  /* ---- closing: the eye returns, settles, blinks once and looks at the install command ---- */
  const button = () => document.querySelector<HTMLElement>(".final-cta .cmd .copy-btn") ?? document.querySelector<HTMLElement>(".final-cta .cmd");
  let blinked = false;
  progress(".final-cta", "top bottom", "top 25%", (v) => {
    sceneState.cta = v;
    if (v > 0.82 && !blinked) { blinked = true; sceneState.blinkAt = performance.now() + 380; }
    if (v < 0.2) blinked = false;
  });
  const track = () => {
    if (sceneState.cta <= 0) { sceneState.lookAt = null; return; }
    const box = button()?.getBoundingClientRect();
    sceneState.lookAt = box ? { x: box.left + box.width / 2, y: box.top + box.height / 2 } : null;
  };
  gsap.ticker.add(track);
  cleanups.push(() => gsap.ticker.remove(track));

  /* ---- the margin thread through the documentation stretch ---- */
  const docs = document.querySelector<HTMLElement>(".docs");
  const thread = document.querySelector<HTMLElement>(".docs-thread");
  if (docs && thread) {
    progress(".docs", "top 75%", "bottom bottom", (v) => thread.style.setProperty("--thread", v.toFixed(4)));
    const heads = docs.querySelectorAll<HTMLElement>(".section");
    heads.forEach((section) => {
      const st = ScrollTrigger.create({ trigger: section, start: "top 62%", onEnter: () => section.setAttribute("data-lit", ""), onLeaveBack: () => section.removeAttribute("data-lit") });
      cleanups.push(() => st.kill());
    });
  }

  /* ---- the lens: a ring the system cursor sits in; it wraps whatever you can press ---- */
  if (fine) {
    const lens = document.createElement("div");
    lens.className = "lens";
    lens.setAttribute("aria-hidden", "true");
    document.body.appendChild(lens);
    const x = gsap.quickTo(lens, "x", { duration: 0.55, ease: "power4.out" });
    const y = gsap.quickTo(lens, "y", { duration: 0.55, ease: "power4.out" });
    const w = gsap.quickTo(lens, "width", { duration: 0.45, ease: EXPO });
    const h = gsap.quickTo(lens, "height", { duration: 0.45, ease: EXPO });
    let locked: HTMLElement | null = null;
    const interactive = 'a, button, [role="tab"], summary, [tabindex="0"]';
    const move = (event: PointerEvent) => {
      if (event.pointerType !== "mouse") return;
      lens.dataset.on = "";
      const hit = (event.target as HTMLElement).closest<HTMLElement>(interactive);
      const target = hit && !hit.closest(".stage") ? hit : null;
      if (target !== locked) { locked = target; lens.toggleAttribute("data-locked", !!target); }
      if (target) {
        const box = target.getBoundingClientRect();
        // Lock around the control, nudged a little toward the pointer.
        const nx = (event.clientX - (box.left + box.width / 2)) * 0.12, ny = (event.clientY - (box.top + box.height / 2)) * 0.12;
        x(box.left + box.width / 2 + nx); y(box.top + box.height / 2 + ny); w(box.width + 14); h(box.height + 14);
      } else {
        x(event.clientX); y(event.clientY); w(34); h(34);
      }
    };
    const leave = (event: PointerEvent) => { if (!event.relatedTarget) delete lens.dataset.on; };
    window.addEventListener("pointermove", move, { passive: true });
    window.addEventListener("pointerout", leave);
    cleanups.push(() => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerout", leave); lens.remove(); });

    /* Magnetic buttons: pulled toward the pointer inside a small radius, released with expo. */
    document.querySelectorAll<HTMLElement>("[data-magnetic]").forEach((el) => {
      const mx = gsap.quickTo(el, "x", { duration: 0.6, ease: EXPO });
      const my = gsap.quickTo(el, "y", { duration: 0.6, ease: EXPO });
      const onMove = (event: PointerEvent) => {
        const box = el.getBoundingClientRect();
        mx((event.clientX - (box.left + box.width / 2)) * 0.22); my((event.clientY - (box.top + box.height / 2)) * 0.32);
      };
      const onLeave = () => { mx(0); my(0); };
      el.addEventListener("pointermove", onMove);
      el.addEventListener("pointerleave", onLeave);
      cleanups.push(() => { el.removeEventListener("pointermove", onMove); el.removeEventListener("pointerleave", onLeave); gsap.set(el, { x: 0, y: 0 }); });
    });
  }

  /* Layout can move after fonts arrive; re-measure the triggers once they have. */
  document.fonts?.ready.then(() => ScrollTrigger.refresh());
  const onStatic = () => stop();
  window.addEventListener("originlm:static", onStatic);
  cleanups.push(() => window.removeEventListener("originlm:static", onStatic));

  function stop() {
    cleanups.splice(0).forEach((fn) => fn());
  }
  return stop;
}
