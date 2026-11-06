/** Deterministic review captures for the UI council.
 *
 *   node scripts/capture_council.mjs <url> <outDir> [--only=desktop|mobile] [--scale=1]
 *
 * Full page plus one image per section, at 1440x900 and 390x844. Runs the static
 * (reduced-motion / no-WebGL) layout: no canvas, no timers, no caret, animations and
 * transitions forced off, so two runs of the same build are byte-identical. That layout
 * is also the page's real no-JavaScript and reduced-motion presentation; every section
 * and all copy is present. Tall sections are also cut into <=1400px chunks so a vision
 * model can read them. A manifest records the viewport, page height, section boxes and
 * a SHA-256 per file.
 */
import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright-core");

const args = process.argv.slice(2);
const url = args.find((a) => !a.startsWith("--")) ?? "http://127.0.0.1:5178/";
const out = args.find((a, i) => i > 0 && !a.startsWith("--")) ?? "council-captures";
const only = args.find((a) => a.startsWith("--only="))?.split("=")[1];
const scale = Number(args.find((a) => a.startsWith("--scale="))?.split("=")[1] ?? 1);
await mkdir(out, { recursive: true });

const VIEWPORTS = [
  ["desktop", { width: 1440, height: 900 }],
  ["mobile", { width: 390, height: 844 }],
];

const SECTIONS = [
  ["01-hero", "header.hero"],
  ["02-story", "#story"],
  ["03-how", "#how"],
  ["04-sdk", "#sdk"],
  ["05-grants", "#grants"],
  ["06-sessions", "#sessions"],
  ["07-integrations", "#integrations"],
  ["08-providers", "#providers"],
  ["09-usage", "#usage"],
  ["10-security", "#security"],
  ["11-install", "#install"],
  ["12-close", "footer.footer"],
];

const FREEZE = `
  html { scroll-behavior: auto !important; }
  *, *::before, *::after {
    animation: none !important;
    transition: none !important;
    caret-color: transparent !important;
    scroll-behavior: auto !important;
  }
  html { scrollbar-width: none; }
  body { overflow-anchor: none; }
`;

/** Fixed elements would repeat when a tall element is stitched into one image. */
const CHROME_OFF = `
  .nav, .skip-link, .lens, .hero-scroll, body::after { display: none !important; }
`;

const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH,
  headless: true,
  args: [
    "--use-angle=swiftshader",
    "--enable-unsafe-swiftshader",
    "--ignore-gpu-blocklist",
    "--force-color-profile=srgb",
    "--font-render-hinting=none",
    "--disable-lcd-text",
    "--disable-gpu",
    "--hide-scrollbars",
  ],
});

const manifest = { url, layout: "static (reduced motion)", scale, viewports: {} };

for (const [name, viewport] of VIEWPORTS) {
  if (only && only !== name) continue;
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: scale,
    reducedMotion: "reduce",
    colorScheme: "dark",
    locale: "en-US",
    timezoneId: "UTC",
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });

  await page.goto(`${url}${url.includes("?") ? "&" : "?"}tier=low`, { waitUntil: "networkidle" });
  await page.addStyleTag({ content: FREEZE });
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => {
    for (const image of document.images) image.loading = "eager";
  });
  await page.waitForFunction(() =>
    [...document.images].every((image) => image.complete),
    null, { timeout: 20000 });
  await page.evaluate(() =>
    Promise.all([...document.images].map((image) => image.decode().catch(() => {}))));
  await page.waitForTimeout(600);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.waitForTimeout(200);

  const height = await page.evaluate(() => document.documentElement.scrollHeight);
  await page.screenshot({ path: `${out}/${name}-full.png`, fullPage: true });

  const sections = {};
  for (const [id, selector] of SECTIONS) {
    const element = page.locator(selector).first();
    const box = await element.evaluate((node) => {
      const rect = node.getBoundingClientRect();
      return { y: rect.top + window.scrollY, width: rect.width, height: rect.height };
    }).catch(() => null);
    if (!box) { console.log(name, id, "missing", selector); continue; }
    const file = `${out}/${name}-${id}.png`;
    if (id === "01-hero") {
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.waitForTimeout(120);
      await page.screenshot({ path: file });
      await page.addStyleTag({ content: CHROME_OFF });
    } else {
      await element.screenshot({ path: file });
    }
    sections[id] = {
      selector,
      y: Math.round(box.y),
      width: Math.round(box.width),
      height: Math.round(box.height),
      file: `${name}-${id}.png`,
    };
  }

  manifest.viewports[name] = {
    viewport, scale, pageHeight: height, full: `${name}-full.png`, sections, errors,
  };
  await context.close();
}

await browser.close();

// Split tall section images into <=1400px chunks for readable model input.
const here = fileURLToPath(new URL(".", import.meta.url));
execFileSync("python", ["-c", `
import sys, os, re
from PIL import Image
out = sys.argv[1]
limit, chunk = 1500, 1400
for name in sorted(os.listdir(out)):
    if not name.endswith('.png') or name.endswith('-full.png'): continue
    if re.search(r'-p\\d+\\.png$', name): continue
    path = os.path.join(out, name)
    image = Image.open(path)
    if image.height <= limit: continue
    stem = name[:-4]
    index = 1
    top = 0
    while top < image.height:
        piece = image.crop((0, top, image.width, min(image.height, top + chunk)))
        piece.save(os.path.join(out, f'{stem}-p{index}.png'))
        index += 1
        top += chunk
`, out], { stdio: "inherit" });

for (const viewport of Object.values(manifest.viewports)) {
  const dir = out;
  viewport.fullSha256 = createHash("sha256").update(await readFile(`${dir}/${viewport.full}`)).digest("hex");
  for (const section of Object.values(viewport.sections)) {
    section.sha256 = createHash("sha256").update(await readFile(`${dir}/${section.file}`)).digest("hex");
  }
}
await writeFile(`${out}/manifest.json`, JSON.stringify(manifest, null, 2));
console.log(JSON.stringify(manifest, null, 2));
