import { useEffect, useLayoutEffect, useRef, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { Draggable } from "gsap/Draggable";
import { gsap } from "../shared/motion";
import { DOCK_OF, useWM, type WinId } from "./wm";

gsap.registerPlugin(Draggable);

type WinProps = {
  id: WinId;
  title: string;
  /** Visible heading inside the title bar; defaults to the title. */
  heading?: ReactNode;
  level?: 2 | 3;
  className?: string;
  children: ReactNode;
  /** Mono text on the right of the title bar. */
  meta?: ReactNode;
  resizable?: boolean;
  style?: CSSProperties;
  /** Copy for the minimized placeholder. */
  minimizedNote?: string;
};

/** One window: chrome, drag (GSAP Draggable, bounded by the nearest [data-bounds]),
 *  arrow-key moving from the title bar, minimize-to-dock, close and reopen. */
export function Win({ id, title, heading, level = 3, className = "", children, meta, resizable, style, minimizedNote }: WinProps) {
  const wm = useWM();
  const state = wm.states[id];
  const ref = useRef<HTMLElement>(null);
  const pos = useRef({ x: 0, y: 0 });
  const drag = useRef<Draggable | null>(null);
  const prev = useRef(state);
  const headingId = `c4-win-${id}`;
  const H = level === 2 ? "h2" : "h3";

  const target = () => {
    const dock = wm.dockEl(DOCK_OF[id]);
    return dock?.getBoundingClientRect() ?? null;
  };

  useEffect(() => {
    const el = ref.current;
    if (state !== "open" || wm.narrow || !el) return;
    const bounds = el.closest("[data-bounds]") as HTMLElement | null;
    const [instance] = Draggable.create(el, {
      type: "x,y",
      trigger: el.querySelector(".c4-titlebar"),
      bounds: bounds ?? undefined,
      edgeResistance: 0.9,
      zIndexBoost: false,
      onPress() { wm.focus(id); },
      onDragEnd() { pos.current = { x: this.x, y: this.y }; },
    });
    drag.current = instance;
    return () => { instance.kill(); drag.current = null; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state, wm.narrow]);

  /* Restore: fly back out of the dock with a little overshoot. */
  useLayoutEffect(() => {
    const before = prev.current;
    prev.current = state;
    const el = ref.current;
    if (state !== "open" || !el || before === "open") return;
    gsap.set(el, { x: pos.current.x, y: pos.current.y });
    if (wm.reduced) return;
    const t = before === "min" ? target() : null;
    const r = el.getBoundingClientRect();
    const onScreen = r.bottom > 0 && r.top < window.innerHeight;
    if (t && onScreen) {
      const dx = t.left + t.width / 2 - (r.left + r.width / 2);
      const dy = t.top + t.height / 2 - (r.top + r.height / 2);
      gsap.fromTo(el, { x: pos.current.x + dx, y: pos.current.y + dy, scale: 0.08, opacity: 0 },
        { x: pos.current.x, y: pos.current.y, scale: 1, opacity: 1, duration: 0.5, ease: "back.out(1.35)", clearProps: "opacity,scale" });
    } else {
      gsap.fromTo(el, { scale: 0.94, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.36, delay: onScreen ? 0 : 0.45, ease: "back.out(1.6)", clearProps: "opacity,scale" });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state]);

  function minimize() {
    const el = ref.current;
    const dock = DOCK_OF[id];
    if (!el || wm.reduced) { wm.setState(id, "min"); wm.bounce(dock); return; }
    wm.showDock();
    const t = target();
    const r = el.getBoundingClientRect();
    if (!t) { wm.setState(id, "min"); return; }
    const dx = t.left + t.width / 2 - (r.left + r.width / 2);
    const dy = t.top + t.height / 2 - (r.top + r.height / 2);
    drag.current?.disable();
    gsap.to(el, {
      x: `+=${dx}`, y: `+=${dy}`, scaleX: 0.06, scaleY: 0.04, opacity: 0.2, duration: 0.42, ease: "power3.in",
      onComplete: () => { wm.setState(id, "min"); wm.bounce(dock); },
    });
  }

  function close() {
    const el = ref.current;
    if (!el || wm.reduced) { wm.setState(id, "closed"); return; }
    drag.current?.disable();
    gsap.to(el, { scale: 0.96, opacity: 0, duration: 0.18, ease: "power2.in", onComplete: () => wm.setState(id, "closed") });
  }

  function resetPosition() {
    const el = ref.current;
    if (!el) return;
    pos.current = { x: 0, y: 0 };
    if (wm.reduced) gsap.set(el, { x: 0, y: 0 });
    else gsap.to(el, { x: 0, y: 0, duration: 0.35, ease: "power3.out" });
  }

  function onTitleKey(event: KeyboardEvent<HTMLDivElement>) {
    if (event.target !== event.currentTarget || wm.narrow) return;
    const step = event.shiftKey ? 64 : 16;
    const delta = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[event.key];
    if (!delta) return;
    event.preventDefault();
    const el = ref.current;
    const bounds = el?.closest("[data-bounds]") as HTMLElement | null;
    if (!el || !bounds) return;
    const r = el.getBoundingClientRect();
    const b = bounds.getBoundingClientRect();
    const dx = Math.max(b.left - r.left, Math.min(b.right - r.right, delta[0]));
    const dy = Math.max(b.top - r.top, Math.min(b.bottom - r.bottom, delta[1]));
    pos.current = { x: pos.current.x + dx, y: pos.current.y + dy };
    gsap.set(el, pos.current);
    drag.current?.update();
  }

  if (state !== "open") {
    return (
      <div className={`c4-ghost ${className}`} data-win={id}>
        <span className="c4-ghost-title">{title}</span>
        <span className="c4-ghost-note">{state === "min" ? (minimizedNote ?? "Minimized to the dock.") : "Closed."}</span>
        <button type="button" className="c4-btn c4-btn-ghost" onClick={() => { wm.setState(id, "open"); wm.focus(id); }}>
          {state === "min" ? "Restore" : "Reopen"} <span className="visually-hidden">{title}</span>
        </button>
      </div>
    );
  }

  return (
    <section
      ref={ref}
      className={`c4-win ${resizable ? "is-resizable" : ""} ${className}`}
      aria-labelledby={headingId}
      data-win={id}
      style={{ ...style, zIndex: wm.z[id] }}
      onPointerDown={() => wm.focus(id)}
      onFocus={() => wm.focus(id)}
    >
      <div
        className="c4-titlebar"
        role={wm.narrow ? undefined : "group"}
        tabIndex={wm.narrow ? -1 : 0}
        aria-label={wm.narrow ? undefined : `${title} title bar. Arrow keys move the window.`}
        onKeyDown={onTitleKey}
        onDoubleClick={resetPosition}
      >
        <div className="c4-lights">
          <button type="button" className="c4-light is-close" onClick={close} aria-label={`Close ${title}`}><span aria-hidden="true">×</span></button>
          <button type="button" className="c4-light is-min" onClick={minimize} aria-label={`Minimize ${title} to the dock`}><span aria-hidden="true">−</span></button>
          <button type="button" className="c4-light is-zoom" onClick={resetPosition} aria-label={`Put ${title} back`}><span aria-hidden="true">+</span></button>
        </div>
        <H className="c4-win-title" id={headingId}>{heading ?? title}</H>
        <span className="c4-win-meta">{meta}</span>
      </div>
      <div className="c4-win-body">{children}</div>
    </section>
  );
}
