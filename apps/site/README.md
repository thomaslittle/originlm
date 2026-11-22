# OriginLM website

A standalone React, TypeScript and Vite landing page for OriginLM. It has its own pinned dependencies and builds to a static, prerendered `dist/` bundle with no hosted runtime or telemetry.

```sh
cd apps/site
npm ci --ignore-scripts
npm run dev       # http://127.0.0.1:5177/
npm run build     # typecheck + static build + SSR prerender into dist/
npm run preview   # serve dist/ locally
npm run check:contrast
```

## Structure

- `src/App.tsx`: Six-section landing page (Hero → Problem & Architecture → How it works → SDKs → Control center → Install).
- `src/RouteWorkbench.tsx`: Interactive 3-stage flow (`01 Installed agents → 02 OriginLM runtime & SDK → 03 Your app output`) showing route resolution, explicit execution grants (`requires_host_approval`), live SDK code, and streamed events.
- `src/styles/site.css`: Flat matte surfaces, 1px hairline borders, signal-only accents, 35/35 WCAG AA verified contrast pairs.
- `src/data/facts.ts` & `src/data/providers.ts`: Copy and provider metadata (Antigravity covers Gemini models).

## Deploying to Cloudflare Pages

- **Root directory:** `apps/site`
- **Build command:** `npm ci --ignore-scripts && npm run build`
- **Build output directory:** `dist`

Or with Wrangler once authenticated (`npx wrangler login`):

```sh
cd apps/site && npm run build && npx wrangler pages deploy dist --project-name originlm
```
