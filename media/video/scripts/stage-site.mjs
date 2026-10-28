/** Stage locally rendered exports; never downloads or publishes anything. */
import { copyFile, mkdir, access } from 'node:fs/promises';
import { resolve } from 'node:path';
const root = resolve(import.meta.dirname, '../../..');
const files = [
  ['originlm-1080p.mp4', 'originlm-promo.mp4'],
  ['originlm-1080p.webm', 'originlm-promo.webm'],
  ['poster.jpg', 'originlm-promo-poster.jpg'],
];
for (const [source] of files) await access(resolve(root, 'media/video/out', source));
const destination = resolve(root, 'apps/site/public/media');
await mkdir(destination, { recursive: true });
for (const [source, target] of files) await copyFile(resolve(root, 'media/video/out', source), resolve(destination, target));
console.log('Staged the 35-second film. Rebuild the site to include it.');
