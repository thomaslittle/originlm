/** Pointer-driven logo motion. One shared listener; no React state or idle frame loop. */
type Eye = { svg: SVGSVGElement; counter: SVGPathElement; x: number; y: number; tx: number; ty: number };
const eyes = new Set<Eye>();
let frame = 0, previous = 0;
let motion: MediaQueryList | undefined;
const atRest = (eye: Eye) => eye.x === eye.tx && eye.y === eye.ty;
function paint(eye: Eye) { eye.counter.setAttribute('transform', `translate(${eye.x.toFixed(3)} ${eye.y.toFixed(3)})`); }
function tick(now: number) {
  const dt = previous ? Math.min(50, now - previous) : 16; previous = now;
  const alpha = 1 - Math.exp(-dt / 55);
  let moving = false;
  for (const eye of eyes) {
    if (atRest(eye)) continue;
    eye.x += (eye.tx - eye.x) * alpha; eye.y += (eye.ty - eye.y) * alpha;
    if (Math.hypot(eye.tx - eye.x, eye.ty - eye.y) < .02) { eye.x = eye.tx; eye.y = eye.ty; }
    paint(eye); moving ||= !atRest(eye);
  }
  frame = moving ? requestAnimationFrame(tick) : 0;
  if (!moving) previous = 0;
}
function schedule() { if (!frame) frame = requestAnimationFrame(tick); }
function rest() {
  for (const eye of eyes) { eye.tx = 0; eye.ty = 0; }
  schedule();
}
function resetImmediately() {
  if (frame) cancelAnimationFrame(frame); frame = 0; previous = 0;
  for (const eye of eyes) { eye.x = eye.y = eye.tx = eye.ty = 0; paint(eye); }
}
function move(event: PointerEvent) {
  if (motion?.matches || (event.pointerType && event.pointerType !== 'mouse' && event.pointerType !== 'pen')) return;
  for (const eye of eyes) {
    const box = eye.svg.getBoundingClientRect();
    if (box.bottom < 0 || box.top > innerHeight || box.right < 0 || box.left > innerWidth || !box.width) continue;
    const scale = box.width / 906;
    const dx = event.clientX - (box.left + 119 * scale), dy = event.clientY - (box.top + 87 * scale);
    const distance = Math.hypot(dx, dy);
    const strength = Math.min(1, distance / (205 * scale * 3));
    eye.tx = distance ? dx / distance * 12 * strength : 0;
    eye.ty = distance ? dy / distance * 10 * strength : 0;
  }
  schedule();
}
function leave(event: PointerEvent) { if (!event.relatedTarget) rest(); }
function preferenceChanged() { if (motion?.matches) resetImmediately(); }
function visibilityChanged() { if (document.hidden) resetImmediately(); }
function listen() {
  motion = matchMedia('(prefers-reduced-motion: reduce)');
  motion.addEventListener('change', preferenceChanged);
  window.addEventListener('pointermove', move, {passive:true});
  window.addEventListener('pointerout', leave);
  window.addEventListener('blur', rest);
  document.addEventListener('visibilitychange', visibilityChanged);
}
function stop() {
  if (frame) cancelAnimationFrame(frame); frame = 0; previous = 0;
  motion?.removeEventListener('change', preferenceChanged); motion = undefined;
  window.removeEventListener('pointermove', move); window.removeEventListener('pointerout', leave);
  window.removeEventListener('blur', rest); document.removeEventListener('visibilitychange', visibilityChanged);
}
export function trackBrandEye(svg: SVGSVGElement, counter: SVGPathElement) {
  const eye: Eye = {svg, counter, x:0, y:0, tx:0, ty:0};
  if (!eyes.size) listen(); eyes.add(eye);
  return () => { eyes.delete(eye); if (!eyes.size) stop(); };
}
