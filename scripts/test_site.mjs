/** Rendered landing-page contracts. Run against a production preview:
 *
 *   cd apps/site && npm run build && npm run preview -- --port 5178
 *   ORIGINLM_SITE_URL=http://127.0.0.1:5178/ node --test scripts/test_site.mjs
 *
 * Uses @playwright/test when the root workspace is installed, otherwise the
 * playwright-core copy in apps/site. CHROME_PATH overrides the browser binary.
 */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
function loadChromium() {
  for (const id of ['@playwright/test', '../apps/site/node_modules/playwright-core']) {
    try { return require(id).chromium; } catch { /* try the next one */ }
  }
  throw new Error('Install @playwright/test at the root, or run npm ci in apps/site');
}
const chromium = loadChromium();
const url = process.env.ORIGINLM_SITE_URL ?? 'http://127.0.0.1:5178/';
const launch = () => chromium.launch({ executablePath: process.env.CHROME_PATH, headless: true });
const WIDTHS = [320, 375, 414, 768, 1024, 1440, 1920];

test('no horizontal scroll, no page errors, 44px touch targets and one-line controls from 320 to 1920', async () => {
  const browser = await launch();
  try {
    for (const width of WIDTHS) {
      const page = await browser.newPage({ viewport: { width, height: 900 } });
      const errors = [];
      page.on('pageerror', (error) => errors.push(error.message));
      await page.goto(url, { waitUntil: 'networkidle' });
      const result = await page.evaluate((minimum) => {
        const controls = [...document.querySelectorAll('a, button, [role="tab"], summary')].filter((el) => {
          const box = el.getBoundingClientRect();
          // Inline links inside prose are exempt (WCAG 2.5.8 inline exception).
          return box.width > 0 && !el.closest('p, dd, td, figcaption, .visually-hidden, .skip-link, [hidden]');
        });
        const lineHeight = (el) => parseFloat(getComputedStyle(el).lineHeight) || parseFloat(getComputedStyle(el).fontSize) * 1.3;
        return {
          overflow: document.documentElement.scrollWidth > window.innerWidth,
          small: controls.filter((el) => el.getBoundingClientRect().height < minimum).map((el) => el.textContent.trim().slice(0, 40) || el.getAttribute('aria-label')),
          // Hallmark gate 49: no clickable label wraps to two lines.
          wrapped: controls
            .filter((el) => !el.matches('.track-card'))
            .filter((el) => {
              // Lines of text only: icons and badges at other offsets are not text lines.
              const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
              const tops = [];
              for (let node = walker.nextNode(); node; node = walker.nextNode()) {
                if (!node.textContent.trim()) continue;
                const range = document.createRange(); range.selectNodeContents(node);
                for (const rect of range.getClientRects()) if (rect.width > 0) tops.push(rect.top + rect.height / 2);
              }
              tops.sort((a, b) => a - b);
              return tops.length > 1 && tops[tops.length - 1] - tops[0] > lineHeight(el) * 0.75;
            })
            .map((el) => el.textContent.trim().slice(0, 40)),
        };
      }, width < 960 ? 44 : 32);
      assert.equal(result.overflow, false, `horizontal overflow at ${width}px`);
      assert.deepEqual(result.small, [], `targets under ${width < 960 ? 44 : 32}px at ${width}px`);
      assert.deepEqual(result.wrapped, [], `clickable text on two lines at ${width}px`);
      assert.deepEqual(errors, [], `page errors at ${width}px`);
      await page.close();
    }
  } finally { await browser.close(); }
});

test('prerendered page leads with the developer story and works without JavaScript', async () => {
  const browser = await launch();
  try {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto(url);
    assert.match(await page.locator('h1').innerText(), /users.{1,3}own subscriptions/);
    assert.equal(await page.locator('h1').count(), 1);
    assert.match(await page.locator('#sdk').innerText(), /createLocalAI/);
    assert.match(await page.locator('#grants').innerText(), /requires_host_approval/);
    assert.ok(await page.locator('#install').innerText());
    assert.equal(await page.locator('img:not([alt])').count(), 0);
    const order = await page.evaluate(() => ['sdk', 'grants', 'usage'].map((id) => document.getElementById(id).offsetTop));
    assert.ok(order[0] < order[1] && order[1] < order[2], 'SDK and grants come before usage');
    await context.close();
  } finally { await browser.close(); }
});

test('landmarks, heading outline, skip link, metadata and structured data', async () => {
  const browser = await launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await page.goto(url, { waitUntil: 'networkidle' });
    const facts = await page.evaluate(() => {
      const meta = (selector) => document.querySelector(selector)?.getAttribute('content') ?? null;
      const levels = [...document.querySelectorAll('h1, h2, h3, h4')].map((h) => Number(h.tagName[1]));
      let skipped = false;
      levels.reduce((previous, level) => { if (level > previous + 1) skipped = true; return level; }, 1);
      return {
        main: document.querySelectorAll('main').length,
        nav: document.querySelectorAll('nav[aria-label]').length,
        footer: document.querySelectorAll('footer').length,
        skipped,
        skipTarget: !!document.querySelector(document.querySelector('.skip-link')?.getAttribute('href') ?? '#none'),
        lang: document.documentElement.lang,
        description: meta('meta[name="description"]'),
        canonical: document.querySelector('link[rel="canonical"]')?.href ?? null,
        og: ['og:title', 'og:description', 'og:image', 'og:url', 'og:type'].map((p) => meta(`meta[property="${p}"]`)),
        twitter: ['twitter:card', 'twitter:title', 'twitter:description', 'twitter:image'].map((n) => meta(`meta[name="${n}"]`)),
        jsonld: [...document.querySelectorAll('script[type="application/ld+json"]')].map((s) => s.textContent),
        manifest: document.querySelector('link[rel="manifest"]')?.href ?? null,
        lazyImages: [...document.images].map((img) => ({ lazy: img.loading, w: img.getAttribute('width'), h: img.getAttribute('height') })),
      };
    });
    assert.equal(facts.main, 1); assert.equal(facts.nav >= 1, true); assert.equal(facts.footer, 1);
    assert.equal(facts.skipped, false, 'heading levels never skip');
    assert.ok(facts.skipTarget, 'skip link points at an element');
    assert.equal(facts.lang, 'en');
    assert.ok(facts.description && facts.description.length <= 300);
    assert.ok(facts.canonical);
    assert.ok(facts.og.every(Boolean), 'Open Graph tags present');
    assert.ok(facts.twitter.every(Boolean), 'Twitter tags present');
    for (const block of facts.jsonld) {
      const data = JSON.parse(block);
      assert.equal(data['@context'], 'https://schema.org');
      assert.equal(data['@type'], 'SoftwareApplication');
    }
    for (const image of facts.lazyImages) {
      assert.equal(image.lazy, 'lazy'); assert.ok(image.w && image.h, 'images carry dimensions');
    }
    const manifest = await (await page.request.get(facts.manifest)).json();
    assert.ok(manifest.icons.some((icon) => icon.purpose === 'maskable'));
    for (const path of ['robots.txt', 'sitemap.xml', 'og-poster.png', 'favicon.svg', 'screens/desktop-overview.webp', 'screens/desktop-overview-800.webp',
      ...['hero', 'hero-narrow', 'discover', 'select', 'grant', 'run', 'noserver', 'cta'].map((name) => `scene/still-${name}.webp`)]) {
      assert.equal((await page.request.get(new URL(path, url).href)).status(), 200, path);
    }
    // Skip link becomes visible on keyboard focus and moves focus to main.
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement?.className), 'skip-link');
  } finally { await browser.close(); }
});

/** Scroll to a point inside the pinned story (0..1) and let the frame settle. */
async function storyAt(page, progress) {
  await page.evaluate((p) => {
    const el = document.getElementById('story');
    const top = el.getBoundingClientRect().top + scrollY;
    window.scrollTo(0, top + (el.offsetHeight - innerHeight) * p);
  }, progress);
  await page.waitForTimeout(700);
}

test('the scene loads after first paint, the story scrubs and its table stays honest', async () => {
  const browser = await launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    page.on('console', (message) => { if (message.type() === 'error') errors.push(message.text()); });
    await page.goto(url, { waitUntil: 'networkidle' });
    // The headline is real HTML before any WebGL.
    assert.match(await page.locator('h1').innerText(), /users.{1,3}own subscriptions/);
    assert.ok(await page.evaluate(() => document.documentElement.classList.contains('motion')), 'motion layout is on');
    await page.waitForSelector('.stage[data-live]', { timeout: 15000 });
    const stage = page.locator('.story-stage');
    assert.equal(await page.locator('.story-pin').evaluate((el) => getComputedStyle(el).position), 'sticky');

    await storyAt(page, 0.02);
    assert.equal(await stage.getAttribute('data-step'), 'discover');
    const foundEarly = await stage.locator('.snap tbody tr[data-found]').count();
    await storyAt(page, 0.24);
    assert.equal(await stage.locator('.snap tbody tr[data-found]').count(), 5, 'every client found by the end of Discover');
    assert.ok(foundEarly < 5, 'rows fill as the scrub advances');
    assert.match(await stage.locator('.story-code-pre').innerText(), /snapshot\.get/);
    // Unknown and unavailable are states, never zero.
    assert.equal(await stage.locator('.reading[data-state="unknown"]').count(), 1);
    assert.equal(await stage.locator('.reading[data-state="unavailable"]').count(), 1);
    assert.match(await stage.locator('.data-head').innerText(), /example data/);

    await storyAt(page, 0.42);
    assert.equal(await stage.getAttribute('data-step'), 'select');
    assert.equal(await stage.locator('tr[data-picked]').count(), 1);
    assert.match(await stage.locator('tr[data-ineligible]').innerText(), /not eligible: sign-in not verified/);

    await storyAt(page, 0.6);
    assert.equal(await stage.getAttribute('data-step'), 'grant');
    assert.match(await stage.locator('.grant-sheet').innerText(), /requires_host_approval/);
    assert.match(await stage.locator('.approval').getAttribute('aria-label'), /your app's own UI/);

    await storyAt(page, 0.995);
    assert.equal(await stage.getAttribute('data-step'), 'run');
    assert.match(await stage.locator('.reply-text').innerText(), /Thursday/);
    assert.equal(await stage.locator('.events li[data-shown]').count(), 4);

    // The scene stops drawing while it is sunk under the documentation sections.
    await page.evaluate(() => document.getElementById('providers').scrollIntoView());
    await page.waitForTimeout(800);
    assert.equal(await page.locator('.stage-canvas').evaluate((el) => getComputedStyle(el).opacity), '0');
    const frames = await page.evaluate(() => window.__originlmScene.stats().frames);
    await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => window.__originlmScene.stats().frames), frames, 'no frames while sunk');
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('reduced motion gets the static story, stills, native scroll and no scene', async () => {
  const browser = await launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
    await page.goto(url, { waitUntil: 'networkidle' });
    const facts = await page.evaluate(() => ({
      motion: document.documentElement.classList.contains('motion'),
      lenis: document.documentElement.classList.contains('lenis'),
      stage: getComputedStyle(document.querySelector('.stage')).display,
      scene: !!window.__originlmScene,
      staticSteps: document.querySelectorAll('.story-static .step').length,
      stageShown: getComputedStyle(document.querySelector('.story-stage')).display,
      still: getComputedStyle(document.querySelector('.hero-still')).backgroundImage,
      lens: !!document.querySelector('.lens'),
    }));
    assert.equal(facts.motion, false);
    assert.equal(facts.lenis, false, 'no smooth scroll');
    assert.equal(facts.stage, 'none');
    assert.equal(facts.scene, false, 'the WebGL chunk is never loaded');
    assert.equal(facts.staticSteps, 4);
    assert.equal(facts.stageShown, 'none');
    assert.match(facts.still, /still-hero/);
    assert.equal(facts.lens, false, 'no cursor lens');
    // Each static step is filled in: the grant waits for the host, the run has its reply.
    const steps = page.locator('.story-static .step');
    assert.match(await steps.nth(2).innerText(), /requires_host_approval/);
    assert.match(await steps.nth(3).innerText(), /Thursday/);
    await steps.nth(0).locator('img').scrollIntoViewIfNeeded();
    await page.waitForFunction(() => { const img = document.querySelector('.story-static img'); return img.complete && img.naturalWidth > 0; });
    // The statement is fully visible without the pull-back.
    assert.equal(await page.locator('.nw').first().evaluate((el) => getComputedStyle(el).opacity), '1');
  } finally { await browser.close(); }
});

test('without WebGL the page falls back to the static layout', async () => {
  const browser = await launch();
  try {
    const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    await context.addInitScript(() => {
      const original = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function (type, ...rest) { return /webgl/.test(type) ? null : original.call(this, type, ...rest); };
    });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(url, { waitUntil: 'networkidle' });
    await page.waitForTimeout(600);
    assert.equal(await page.evaluate(() => document.documentElement.classList.contains('motion')), false);
    assert.equal(await page.evaluate(() => document.documentElement.dataset.sceneFallback), 'webgl2 unavailable');
    assert.equal(await page.locator('.story-static .step').count(), 4);
    assert.ok(await page.locator('.story-static h3').first().isVisible());
    assert.deepEqual(errors, []);
    await context.close();
  } finally { await browser.close(); }
});

test('SDK tabs switch language by click and keyboard and show matching install commands', async () => {
  const browser = await launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    await page.goto(url, { waitUntil: 'networkidle' });
    const panel = page.locator('#sdk [role="tabpanel"]');
    const tab = page.locator('#sdk [role="tab"]');
    assert.equal(await tab.count(), 3);
    await tab.first().focus();
    await page.keyboard.press('ArrowRight');
    assert.match(await panel.innerText(), /LocalAi::start/);
    assert.match(await panel.innerText(), /cargo add originlm@=0\.1\.0-alpha\.25/);
    await page.keyboard.press('ArrowRight');
    assert.match(await panel.innerText(), /Client\.auto/);
    assert.match(await panel.innerText(), /pip install originlm==0\.1\.0a25/);
    await page.keyboard.press('ArrowRight');
    assert.match(await panel.innerText(), /createLocalAI/);
    assert.match(await panel.innerText(), /npm install originlm@alpha/);
    await tab.nth(2).click();
    assert.equal(await tab.nth(2).getAttribute('aria-selected'), 'true');
    assert.equal(await tab.nth(2).getAttribute('tabindex'), '0');
    assert.equal(await tab.nth(0).getAttribute('tabindex'), '-1');
  } finally { await browser.close(); }
});

test('mobile menu opens, closes on Escape and outside click, and its links navigate', async () => {
  const browser = await launch();
  try {
    const page = await browser.newPage({ viewport: { width: 375, height: 800 } });
    await page.goto(url, { waitUntil: 'networkidle' });
    const toggle = page.locator('.nav-toggle');
    const menu = page.locator('#mobile-menu');
    assert.equal(await toggle.getAttribute('aria-expanded'), 'false');
    assert.equal(await menu.count(), 1, 'aria-controls target always exists');
    assert.equal(await menu.isVisible(), false);
    await toggle.click();
    assert.equal(await toggle.getAttribute('aria-expanded'), 'true');
    assert.ok(await menu.isVisible());
    await page.keyboard.press('Escape');
    assert.equal(await menu.isVisible(), false);
    assert.equal(await page.evaluate(() => document.activeElement?.className), 'nav-toggle', 'focus returns to the toggle');
    await toggle.click();
    await page.mouse.click(200, 700);
    assert.equal(await menu.isVisible(), false, 'outside click closes');
    await toggle.click();
    await menu.getByRole('link', { name: 'Grants' }).click();
    await page.waitForFunction(() => location.hash === '#grants');
    assert.equal(await menu.isVisible(), false);
    const top = await page.evaluate(() => document.getElementById('grants').getBoundingClientRect().top);
    assert.ok(top >= 0 && top < 200, `#grants scrolled into view below the sticky nav (top ${top})`);
  } finally { await browser.close(); }
});

test('every in-page anchor resolves and lands below the sticky nav', async () => {
  const browser = await launch();
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    await page.goto(url, { waitUntil: 'networkidle' });
    const hashes = await page.evaluate(() => [...new Set([...document.querySelectorAll('a[href^="#"]')].map((a) => a.getAttribute('href')))].filter((h) => h.length > 1));
    assert.ok(hashes.length >= 8);
    for (const hash of hashes) {
      assert.ok(await page.locator(hash).count(), `${hash} exists`);
      await page.evaluate((h) => { location.hash = ''; location.hash = h; }, hash);
      await page.waitForTimeout(100);
      const { top, nav } = await page.evaluate((h) => ({
        top: document.querySelector(h).getBoundingClientRect().top,
        nav: document.querySelector('.nav').getBoundingClientRect().bottom,
      }), hash);
      if (hash !== '#top' && hash !== '#main') assert.ok(top >= nav - 1, `${hash} is not hidden under the nav (top ${top}, nav ${nav})`);
    }
  } finally { await browser.close(); }
});
