/** Dev-only deterministic capture entry. Never included by the site's production entry. */
import { createStudio, type StudioFrame } from './studio';
import { LAYERS } from '../data/layers';
const canvas = document.getElementById('studio') as HTMLCanvasElement;
const labels = LAYERS.map((layer) => {
  const label = document.createElement('div'); label.className = 'layer-label'; label.textContent = layer.label;
  label.style.position = 'fixed'; document.body.append(label); return label;
});
await document.fonts.ready;
const studio = await createStudio(canvas, { width: innerWidth, height: innerHeight, dpr: devicePixelRatio, light: new URLSearchParams(location.search).has('light') });
const render = (frame: StudioFrame) => {
  const points = studio.render(frame);
  points.forEach((point, index) => { labels[index].style.transform = `translate(${point.x}px, ${point.y}px) translateY(-50%)`; labels[index].style.opacity = String(point.opacity); });
};
Object.assign(window, { renderOriginFrame: render, originStudioReady: true });
render({ seconds: 4, spread: 1 });
