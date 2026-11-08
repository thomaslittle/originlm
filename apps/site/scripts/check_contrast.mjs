/** WCAG contrast for every text and control pair the site uses, read from the tokens.
 *
 *   node scripts/check_contrast.mjs
 *
 * Translucent surfaces are composited over their worst case: the story's data panel
 * (--color-panel, 90% unlit) over a fully lit amber pixel of the scene.
 */
import { readFile } from 'node:fs/promises';

/* Design-exploration concepts carry their own tokens: `check_contrast.mjs c1` (… c5,
   index, all). With no argument this checks the published landing page, as before. */
const concept = process.argv[2];
if (concept) {
  const { run } = await import('./check_contrast_concepts.mjs');
  process.exit((await run(concept)) ? 1 : 0);
}

const css = await readFile(new URL('../src/styles/tokens.css', import.meta.url), 'utf8');
const brand = await readFile(new URL('../../../brand/tokens.css', import.meta.url), 'utf8');
const vars = {};
for (const source of [brand, css]) for (const [, name, value] of source.matchAll(/--([\w-]+):\s*([^;]+);/g)) vars[name] = value.trim();
const resolve = (value) => { let v = value; for (let i = 0; i < 8 && /var\(/.test(v); i++) v = v.replace(/var\(--([\w-]+)\)/g, (_, n) => vars[n]); return v; };
const parse = (value) => {
  const v = resolve(value);
  let m = v.match(/^#([0-9a-f]{6})$/i);
  if (m) return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16)).concat(1);
  m = v.match(/^rgb\((\d+) (\d+) (\d+)(?: \/ (\d+)%)?\)$/);
  if (m) return [+m[1], +m[2], +m[3], m[4] ? +m[4] / 100 : 1];
  throw new Error(`cannot parse ${value} -> ${v}`);
};
const over = (top, bottom) => top.slice(0, 3).map((c, i) => c * top[3] + bottom[i] * (1 - top[3])).concat(1);
const lum = ([r, g, b]) => { const f = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); };
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };

const ground = parse('var(--color-ground)');
const amber = parse('var(--color-accent)');
const surfaces = {
  ground,
  'paper-2': parse('var(--color-paper-2)'),
  'panel over lit amber': over(parse('var(--color-panel)'), amber),
  'scrim over lit amber': over(parse('var(--color-scrim)'), amber),
};
const text = ['ink', 'ink-2', 'muted', 'faint', 'accent', 'accent-bright', 'syntax-keyword', 'syntax-string', 'syntax-comment', 'syntax-fn', 'syntax-num'];
const rows = [];
for (const [surface, bg] of Object.entries(surfaces)) for (const name of text) {
  const key = name.startsWith('syntax') ? name : `color-${name}`;
  rows.push([key, surface, ratio(parse(`var(--${key})`), bg), 4.5]);
}
rows.push(['color-accent-ink', 'accent (primary button)', ratio(parse('var(--color-accent-ink)'), amber), 4.5]);
rows.push(['color-accent-ink', 'ink (selected tab)', ratio(parse('var(--color-accent-ink)'), parse('var(--color-ink)')), 4.5]);
rows.push(['color-focus', 'ground (focus ring, non-text)', ratio(parse('var(--color-focus)'), ground), 3]);

let failed = 0;
console.log('| Foreground | Background | Ratio | Needs | |\n|---|---|---|---|---|');
for (const [fg, bg, value, need] of rows) {
  const pass = value >= need; if (!pass) failed++;
  console.log(`| ${fg} | ${bg} | ${value.toFixed(2)} | ${need} | ${pass ? 'pass' : 'FAIL'} |`);
}
console.log(`\n${rows.length - failed}/${rows.length} pairs pass.`);
process.exit(failed ? 1 : 0);
