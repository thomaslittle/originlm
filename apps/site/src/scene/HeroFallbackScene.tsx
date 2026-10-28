import { useEffect, useEffectEvent, useRef } from 'react';
import { LAYERS, type Layer } from '../data/layers';

/** The same five image planes, animated without requiring a WebGL context. */
export default function HeroFallbackScene({ stageEl, onReady, onHover }: {
  stageEl: HTMLElement | null;
  onReady: () => void;
  onHover: (layer: Layer | null) => void;
}) {
  const scene = useRef<HTMLDivElement>(null);
  const ready = useEffectEvent(onReady);
  useEffect(() => {
    const element = scene.current;
    if (!element || !stageEl) return;
    const planes = [...element.querySelectorAll<HTMLElement>('.css-hero-plane')];
    let animation = 0, previous = performance.now(), spread = 0, velocity = 0;
    const frame = (now: number) => {
      const dt = Math.min(0.04, (now - previous) / 1000); previous = now;
      const bounds = (stageEl.closest('.hero') ?? stageEl).getBoundingClientRect();
      const track = stageEl.closest('.hero-scroll')?.getBoundingClientRect() ?? bounds;
      const pinned = innerWidth > 960 && track.height > bounds.height + 1;
      const distance = pinned ? (track.height - bounds.height) * 0.8 : bounds.height * 0.3;
      const target = Math.max(0, Math.min(1, (72 - (pinned ? track.top : bounds.top)) / Math.max(1, distance)));
      velocity += ((target - spread) * 120 - velocity * 18) * dt;
      spread = Math.max(0, Math.min(1, spread + velocity * dt));
      planes.forEach((plane, index) => {
        const offset = index - 2;
        plane.style.transform = `translate(-50%, -50%) translate3d(${offset * spread * 12}px, ${offset * spread * 48}px, ${-offset * 18}px) rotateX(42deg) rotateY(-14deg) rotateZ(-15deg)`;
      });
      element.style.setProperty('--hero-spread', String(spread));
      animation = requestAnimationFrame(frame);
    };
    animation = requestAnimationFrame(frame);
    ready();
    return () => cancelAnimationFrame(animation);
  }, [stageEl]);
  return <div className="css-hero-scene" ref={scene} aria-hidden="true">
    {LAYERS.map((layer, index) => <div key={layer.id} className="css-hero-plane"
      style={{zIndex:LAYERS.length-index}} onPointerEnter={() => onHover(layer)} onPointerLeave={() => onHover(null)}>
      <img src={`/scene/${layer.id}.webp`} alt="" draggable={false} />
      <span>{layer.label}</span>
    </div>)}
  </div>;
}
