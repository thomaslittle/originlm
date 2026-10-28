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

## Scroll hero

`src/scene/studio.ts` builds the five-layer WebGL scene from synthetic example screenshots in `public/scene/`. `HeroScene.tsx` ties separation to scroll position and reverses when scrolling back. Hardware WebGL uses the full renderer; software WebGL uses the existing lightweight renderer. If WebGL is unavailable or fails, `HeroFallbackScene.tsx` animates the same image planes through CSS transforms. Reduced motion, pause and no-JavaScript views retain a static poster. Hidden/offscreen animation stops.

Run the rendered production checks from the repository root with a production preview running:

```sh
ORIGINLM_SITE_URL=http://127.0.0.1:5178/ CHROME_PATH=/usr/bin/google-chrome node --test scripts/test_site.mjs
```

Omit `CHROME_PATH` to use an installed Playwright Chromium. Checks cover GPU and no-WebGL forward/reverse scrolling, pause/play, reduced motion, mobile overflow and no-JavaScript content.

## Artwork and media

Headers, icons and the social card use the user-supplied replacement offset-O kit; see [provenance](../../brand/PROVENANCE.md). The existing font and UI theme remain unchanged. Public screenshots use labeled synthetic data. `scripts/capture_example_ui.mjs` refreshes scene inputs against the isolated example server; `media/video/scripts/render-posters.mjs` regenerates poster images.

The 35-second film source is in `media/video`. Rendered media is staged explicitly; the source checkout shows a poster unless the required film files are present. Registry packages and signed installers are not yet published. `originlm.dev` remains a deployment placeholder.
