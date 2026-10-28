import { useEffect, useRef } from 'react';
/** Actual example-mode captures; scroll only changes their physical separation. */
export function LayeredScreens() {
  const figure = useRef<HTMLElement>(null);
  useEffect(() => {
    const node = figure.current;
    if (!node) return;
    const motion = matchMedia('(prefers-reduced-motion: reduce)');
    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const bounds = node.getBoundingClientRect();
        const progress = motion.matches ? 1 : Math.max(0, Math.min(1, (innerHeight - bounds.top) / (innerHeight * 0.8)));
        node.style.setProperty('--screen-spread', String(0.4 + progress * 0.6));
      });
    };
    update(); addEventListener('scroll', update, { passive: true }); addEventListener('resize', update); motion.addEventListener('change', update);
    return () => { cancelAnimationFrame(frame); removeEventListener('scroll', update); removeEventListener('resize', update); motion.removeEventListener('change', update); };
  }, []);
  return <figure ref={figure} className="layered-screens" data-kind="product">
    <div className="layered-screens-plane">
      <img src="/screens/control-center-connections.webp" width="2880" height="2000" loading="lazy" alt="OriginLM connections with labeled example accounts." />
      <img src="/screens/control-center-usage.webp" width="2880" height="1800" loading="lazy" alt="Provider usage windows in the example control center." />
      <img src="/scene/app.webp" width="2880" height="2000" loading="lazy" alt="The overview unites quota rings, reset countdowns and an explicitly estimated cost trend." />
    </div>
    <figcaption>Real control-center screens, using example data.</figcaption>
  </figure>;
}
