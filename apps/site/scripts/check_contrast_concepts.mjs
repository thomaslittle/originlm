/** WCAG contrast for a landing concept, read from its own stylesheet.
 *
 *   node scripts/check_contrast.mjs c1        (or c2 … c5, index, all)
 *
 * Each concept's CSS (src/concepts/<id>/*.css) declares its colour tokens as custom
 * properties and lists the pairs it actually uses in an @contrast comment:
 *
 *   / * @contrast
 *      ink on bg 4.5
 *      muted on panel/bg 4.5      panel composited over bg (translucent surfaces)
 *      focus on bg 3              non-text (focus ring, control boundary)
 *   * /
 */
import { readdir, readFile } from 'node:fs/promises';

const root = new URL('../src/concepts/', import.meta.url);
const parse = (vars, value, depth = 0) => {
  let v = value.trim();
  if (depth > 8) throw new Error(`token loop at ${value}`);
  const ref = v.match(/^var\(--([\w-]+)\)$/);
  if (ref) { if (!(ref[1] in vars)) throw new Error(`unknown token --${ref[1]}`); return parse(vars, vars[ref[1]], depth + 1); }
  let m = v.match(/^#([0-9a-f]{3})$/i);
  if (m) return [...m[1]].map((c) => parseInt(c + c, 16)).concat(1);
  m = v.match(/^#([0-9a-f]{6})([0-9a-f]{2})?$/i);
  if (m) return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16)).concat(m[2] ? parseInt(m[2], 16) / 255 : 1);
  m = v.match(/^rgba?\(\s*(\d+)[ ,]+(\d+)[ ,]+(\d+)(?:\s*[/,]\s*([\d.]+)(%?))?\s*\)$/);
  if (m) return [+m[1], +m[2], +m[3], m[4] ? (m[5] ? +m[4] / 100 : +m[4]) : 1];
  throw new Error(`cannot parse colour ${value}`);
};
const over = (top, bottom) => top.slice(0, 3).map((c, i) => c * top[3] + bottom[i] * (1 - top[3])).concat(1);
const lum = ([r, g, b]) => { const f = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); };
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };

async function check(id) {
  const dir = new URL(`${id}/`, root);
  const files = (await readdir(dir)).filter((f) => f.endsWith('.css'));
  const css = (await Promise.all(files.map((f) => readFile(new URL(f, dir), 'utf8')))).join('\n');
  const vars = {};
  for (const [, name, value] of css.matchAll(/--([\w-]+):\s*([^;}]+)[;}]/g)) if (!(name in vars)) vars[name] = value.trim();
  const block = css.match(/@contrast([\s\S]*?)\*\//);
  if (!block) throw new Error(`${id}: no @contrast block`);
  const rows = [];
  for (const line of block[1].split('\n')) {
    const m = line.trim().match(/^([\w-]+)\s+on\s+([\w/-]+)\s+([\d.]+)/);
    if (!m) continue;
    const [, fgName, bgSpec, need] = m;
    const layers = bgSpec.split('/');
    let bg = parse(vars, `var(--${layers[layers.length - 1]})`);
    for (let i = layers.length - 2; i >= 0; i--) bg = over(parse(vars, `var(--${layers[i]})`), bg);
    const fg = over(parse(vars, `var(--${fgName})`), bg);
    rows.push([fgName, bgSpec, ratio(fg, bg), +need]);
  }
  let failed = 0;
  console.log(`\n## ${id}\n\n| Foreground | Background | Ratio | Needs | |\n|---|---|---|---|---|`);
  for (const [fg, bg, value, need] of rows) {
    const pass = value >= need; if (!pass) failed++;
    console.log(`| ${fg} | ${bg} | ${value.toFixed(2)} | ${need} | ${pass ? 'pass' : 'FAIL'} |`);
  }
  console.log(`\n${id}: ${rows.length - failed}/${rows.length} pairs pass.`);
  return failed;
}

export async function run(target) {
  const ids = target === 'all' ? ['index', 'c1', 'c2', 'c3', 'c4', 'c5'] : [target];
  let failed = 0;
  for (const id of ids) failed += await check(id);
  return failed;
}
