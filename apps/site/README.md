# OriginLM website

A standalone React, TypeScript and Vite site. It has its own pinned dependencies and is outside the root npm workspace.

```sh
cd apps/site
npm ci --ignore-scripts
npm run dev
# http://127.0.0.1:5177/
npm run build
npm run preview
```

The build emits a static, prerendered site in `dist/`; there is no hosted runtime or telemetry. Stop the development server before starting preview on the same port, or pass `-- --port 5178`.

## Direction: "the eye wakes"

The OriginLM mark is an eye. The page is one continuous WebGL scene seen from it: the eye
wakes and finds the AI clients on the machine (hero), wires them into one runtime while a
pinned story is scrubbed (Discover, Select, Grant, Run), pulls back to show that it all
sits inside one machine with no OriginLM server, sinks while the page reads as
documentation (SDK, grants, sessions, integrations, support matrix, then the control
center and usage, then security and install), and returns for the install command, where
it blinks once and looks at it. Plan, palette, type and the critique against the banned
list: [`docs/design/landing-wow-plan.md`](../../docs/design/landing-wow-plan.md).

- **Scene.** `src/scene/eye-scene.ts`, raw WebGL2 (no three.js), loaded as its own chunk
  after first paint. The mark is the real logo geometry (`brand/logo-geometry.json`)
  rasterised to masks; only its opening moves, following the pointer through a spring.
  Amber is the only light. DPR is capped (1.75, or 1.25 on the light tier); render scale
  drops on slow frames; drawing stops when the scene has sunk or the tab is hidden.
- **State.** `src/scene/state.ts` holds plain numbers (hero, story, pull, sink, closing
  progress, pointer). The director writes them, the scene reads them, the story renders
  from the same story progress.
- **Director.** `src/motion/director.ts`, a lazy chunk: Lenis (wheel smoothing),
  ScrollTrigger (scroll to phase), the cursor lens and magnetic buttons. Pinning is CSS
  `position: sticky`, so React keeps ownership of its DOM.
- **Type.** Fraunces (display, roman only), Red Hat Display (brand body), Martian Mono
  (code and data), self-hosted from `src/assets/fonts/` (licenses in
  [`LICENSES.md`](src/assets/fonts/LICENSES.md)).
- **Without motion.** `index.html` sets `.motion` before first paint unless reduced
  motion is requested or WebGL2 is missing. Without it (and without JavaScript) the page
  uses its static layout: the story is four filled-in steps, and the scene appears as
  stills in `public/scene/`, rendered from the live scene by `npm run capture:stills`.
  A failed WebGL context falls back to the same layout.

All styling is in `src/styles/page.css` over `src/styles/tokens.css`, which imports the
generated brand tokens. Colours and font families are referenced by token only.

## Checks

Run the rendered checks from the repository root against a production preview:

```sh
cd apps/site && npm run build && npm run preview -- --port 5178
# in another shell, from the repository root:
ORIGINLM_SITE_URL=http://127.0.0.1:5178/ node --test scripts/test_site.mjs
```

The test uses `@playwright/test` when the root workspace is installed, otherwise this
folder's `playwright-core`; set `CHROME_PATH` to choose a browser binary. It covers 320
to 1920 px (horizontal overflow, page errors, 44px touch targets below 960px, clickable
labels that wrap); the no-JavaScript prerender; landmarks, heading order, skip link,
meta, Open Graph, Twitter, JSON-LD, manifest, sitemap, robots and the stills; the scene
going live and the story scrub (rows fill, unknown and unavailable stay states, the grant
waits for the host, the run streams; no frames drawn while the scene is sunk); reduced
motion (static story, stills, no smooth scroll, no scene, no lens); no WebGL; SDK tabs;
the mobile menu; and every in-page anchor.

From `apps/site`:

| Script | What it does |
| --- | --- |
| `npm run check:samples` | Type-checks the quickstart chunks and `summarize.ts` against `packages/sdk-typescript/src`, with a negative control. Python and Rust samples: see the audit. |
| `npm run check:contrast` | WCAG ratios for every text pair, including translucent surfaces over a fully lit amber pixel. |
| `npm run capture:stills -- <url>` | Re-renders `public/scene/still-*.webp` from the live scene (needs Python with Pillow). |
| `npm run capture:frames -- <url> <dir> [--gpu] [--reduced]` | Review frames at every scroll stage, desktop and mobile. |
| `node scripts/measure_perf.mjs <url> [--gpu] [--cpu=4]` | FCP, LCP, CLS, TBT and rAF frame times while idle and while scrubbing. |
| `node scripts/record_scroll.mjs <url> <out.webm> [--gpu]` | A scroll-through video (CDP screencast, encoded with `FFMPEG`). |

Headless Chromium renders WebGL with SwiftShader unless `--gpu` is passed; neither is a
mid-range laptop. Say which one a number came from.

## Artwork and media

Headers, icons and the social card use the user-supplied offset-O kit; see
[provenance](../../brand/PROVENANCE.md). `public/og-poster.png` is rendered from HTML with
the brand font. The control-center screenshots (`public/screens/desktop-overview.webp`
and the 800px `desktop-overview-800.webp`) are real captures of the GPUI app in example
mode (synthetic data), taken with `scripts/capture_desktop_example.ps1` on Windows. The
scene stills in `public/scene/` are frames of this site's own renderer.

The SDK is published as an alpha: npm `originlm@alpha` (0.1.0-alpha.25; npm's `latest`
tag still points at 0.1.0-alpha.20), crates.io `originlm` 0.1.0-alpha.25 and PyPI
`originlm` 0.1.0a25, checked 2026-11-04. Signed desktop installers are not published.
`originlm.dev` in the canonical URL, Open Graph tags, sitemap and robots file is a
deployment placeholder until the domain is confirmed.
