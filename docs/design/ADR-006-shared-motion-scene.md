# ADR-006: One physical UI scene for the site and film

Accepted 2026-10-28, following Thomas's revised motion direction.

The v1 branch owns `apps/site` and `media/video`; no further landing-site branch merges are permitted. The scene has five opaque layers: app, SDK, transport, runtime and providers. Real example-mode app captures supply textures. There are no account captures, translucent placeholder panels, or particle packets.

`apps/site/src/scene/studio.ts` is the shared renderer. The React wrapper supplies scroll and pointer inputs; the dev-only `scene-render.html` entry supplies deterministic timestamps for offline capture. A perspective camera, Z separation, studio lighting, edge highlights and line glints are common to both outputs. The film composites rendered shots with fframes. Hero copy stays in the DOM.

Mobile uses a lower DPR and bypasses bloom, depth of field and grain. Reduced motion, a paused animation, or unavailable WebGL shows the poster generated from the same scene. The lower page uses CSS perspective over actual app screenshots, with a static no-script composition.

The film target is 35 seconds: problem (3), see your AI (7), build with it (9), connect (4), install (5), then a 7-second brand ending to be tightened during visual review. No audio is required. Large rendered outputs belong in `media/video/out/`, never in source control.

Implementation and visual iteration are in progress. No frame-rate, Lighthouse, bundle-budget, or final-render qualification is claimed yet.


## Site scroll behavior, 2026-10-28

The site owns its scroll controller separately from the shared scene renderer.
On desktop, a sticky hero keeps the real UI in view through the complete separation
and reverse-separation range. Mobile scrubs across a shorter ordinary flow. Static,
no-script and reduced-motion states use the existing poster and native layer key.
The introductory macro runs once per page visit; leaving and returning to the hero
does not restart it. A headless software-WebGL test covers separation, reversal and
viewport containment. It does not qualify hardware frame rates.

Thomas stopped all video work during final verification. Existing media is retained;
no further video qualification is implied by this site behavior change.
