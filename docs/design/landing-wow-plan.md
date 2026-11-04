# Landing page: "The eye wakes" (design plan, 2026-11-04)

The previous pass was rejected as underwhelming. This plan starts the visual
direction over. Copy, product facts, quickstart code and the brand amber stay.

## Concept

The OriginLM mark is an eye: an amber ovoid with an offset, rounded opening.
The page is one continuous shot from that eye's point of view. It wakes, looks
around the user's machine, finds the AI clients already there, and wires them
into one runtime. There is one WebGL scene for the whole page; every section
moves that scene instead of opening a new card. When the page reads as
documentation (SDK, grants, matrix), the scene sinks into the dark and the eye
waits in the nav logo. At the end it comes back, blinks once and looks at the
install command.

## Palette (amber is the only light)

| Name | Hex | Role |
| --- | --- | --- |
| Unlit | `#120f0f` | Page ground and the scene's falloff. Not a flat fill: in the scene it is where amber light runs out. |
| Graphite | `#231f20` | Brand graphite; lit field near the eye, raised surfaces. |
| Amber | `#eba31a` | The light source. The eye, threads, focus, primary button. |
| Hot amber | `#f7bd4b` | Thread cores, hover, the runtime core. |
| Filament | `#fff2dc` | White-hot centre of a thread pulse. Never a fill. |
| Paper | `#faf9f5` / muted `#b0a8a1` | Text. |

No second hue. Evidence states (known, unknown, unavailable) are told by
weight, glyph and amber intensity, not by green/red.

## Type

- Display: **Fraunces** (OFL), variable, roman only, `opsz 144`, light weight
  (~320), `SOFT 100`. Its diagonal stress and soft terminals echo the mark,
  which is itself a stressed, soft O. Set enormous: the hero headline is
  ~8vw, section titles ~5vw. Line masks reveal it; it never fades up.
- Body: **Red Hat Display** (brand face, already self-hosted).
- Code and data: **Martian Mono** (OFL), condensed (`wdth 87.5`). Only for real
  code, method names and snapshot fields.
- Scale contrast: 8vw display against 15 to 17px body. Nothing in between
  except the step words.

## Section choreography

1. **Hero (100svh).** First paint is HTML: dark ground, headline, lead, install
   command. The headline lines rise out of masks (expo out, 90ms stagger). The
   WebGL chunk loads after first paint. The eye's opening unfolds from a closed
   slit (1.4s), the body brightens like a filament, then the eye tracks the
   cursor through a spring with weight. It scans: five nodes (Claude Code,
   Codex, Cursor, Gemini CLI, Ollama) ignite at different depths, and a thread
   draws from each into a runtime core that sits behind the eye, seen through
   its opening. Far nodes are out of focus. Asymmetric: headline bottom left,
   eye upper right; on phones the eye sits above the type.
2. **The story, pinned (Discover, Select, Grant, Run).** One pinned viewport,
   scrubbed over ~4 screens. A huge step word rolls through a slot mask. The
   quickstart types itself chunk by chunk with the scrub. One snapshot table
   morphs with the scene:
   - Discover: camera moves in; each node pings and its table row fills
     (`quotaPools[].windows[].reading.state`: known, unknown, unavailable,
     with `resetsAt`). Unknown rows stay unknown, never zero.
   - Select: one node (Claude Code, Personal) is pulled toward the camera;
     ineligible rows dim with their reason.
   - Grant: a six-blade consent shutter closes over that thread; the host
     approval sheet (an example of the app's own UI) appears; the shutter
     opens.
   - Run: packets of light travel the thread into the core; the reply streams;
     typed events fill.
3. **No server, because there isn't one.** The camera pulls back until the
   whole scene sits inside one machine outline, which draws itself. Lines from
   the cloud clients cross the outline toward their own provider ("under its
   terms"); Ollama's stays inside. No cloud icon anywhere and no OriginLM
   server, because there isn't one. The sentence is set as the largest type on
   the page, word by word against the pull-back.
4. **The documentation stretch.** SDK, grants, sessions, integrations, support
   matrix, then the control center and usage, then security. The scene sinks
   to an ember; sections sit on the unlit ground with grain. Each section is
   composed differently (index column, full-bleed table, split code). One
   recurring motif: a thin amber thread in the left margin that ignites a node
   when a section's title arrives. Only titles mask-reveal; body copy is just
   there.
5. **Install.** The scene returns. The eye settles, blinks once, and its
   opening turns to the install command. The command button is magnetic.

Motion language: one family, `expo.out` / `cubic-bezier(0.16,1,0.3,1)` for
entrances and `power4.inOut` for camera moves; overlapping staggers. Nothing
linear except packets on a thread (constant speed is what signal looks like).

## Tech

- Raw WebGL2, one scene module (~20 KB), lazy-loaded after first paint. three.js
  was considered: this scene is billboards, ribbons and a mask-textured mark,
  and three would add ~130 KB gzip without being used for anything we draw.
  Passes: field and dust, threads (screen-space ribbons, chromatic
  per-channel falloff, packets), nodes (depth-of-field by distance from the
  focal plane), the mark (the real logo paths rasterised to masks, opening
  offset by gaze, lid by aperture), shutter, then a composite pass with
  vignette and film grain. DPR capped at 1.75 (1.25 on the low tier);
  render stops when the canvas is hidden, the tab is hidden, or the scene has
  sunk.
- GSAP + ScrollTrigger for pin and scrub; Lenis for smooth scroll (off for
  reduced motion and touch). Loaded as their own chunk after first paint.
- Reduced motion: no smooth scroll, no pin, no scrub, no cursor follower. The
  story renders as four stacked, fully populated steps; the scene shows
  pre-rendered stills captured from the real renderer. The same stills cover
  WebGL being unavailable and JavaScript being off.

## Critique against the banned list, and what changed

| Risk in the first draft | Verdict | Revision |
| --- | --- | --- |
| A centred hero with the eye in the middle and the headline under it | Default AI-landing look. | Asymmetric: headline bottom left at 8vw, eye upper right, nodes spill behind the type. |
| A gradient haze behind the eye | Aurora-blob tell. | Light is computed from the eye and threads only; the dark is their falloff. |
| Nodes drawn as brand logos in tiles | Generic icon grid, and trademark use. | Nodes are points of light with HTML labels in the mono face. |
| Four story cards that swap | Exactly what the brief bans. | One table morphs with the scene; only the step word rolls. |
| Fade-up on every section | Banned. | Titles use the same line-mask as the hero; body copy is static; the margin thread is the only scroll motif. |
| A cloud icon for providers in the pull-back | Banned, and dishonest about the architecture. | Provider-bound lines simply leave the outline, labelled. |
| Glass panels for code | Banned. | Code sits on the unlit ground with a hairline and a filename caption, no fake window chrome. |
| A second accent for "unknown" | Breaks "amber is the only light". | Unknown is an open ring and dim text; known is a filled amber cell. |
| Cursor dot follower (Hallmark bans it) | The brief asks for it. | It is the eye's lens: a ring that the system cursor sits in, which wraps interactive elements. The system cursor stays visible; fine pointers only. |
| Scroll scrub and parallax (Hallmark bans them) | The brief asks for them. | Scrub is the narrative, not decoration, and every scrubbed state also exists as static content for reduced motion. |

Skills used: frontend-design, Hallmark (atmospheric genre, motion, anti-patterns,
typography), design-superpowers:design, hyperframes-animation (easing language).
`taste-skill` and `landing-page-design` are not installed on this machine.

## Build log: self-review passes

Frames for each pass are in [`screens/wow/`](screens/wow/). Review was done on real
WebGL frames (SwiftShader for passes 1 to 4, this machine's GPU for the final pass).

| Pass | What was wrong in the frames | What changed |
| --- | --- | --- |
| 1, first build | The whole field was muddy brown (the haze term lit the full screen). The eye was a soft blur. Billboard quads left visible rectangles. Edge aberration threw blue-green fringes. A node sat off-screen. The solid nav bar cut the scene. Story code ran off the bottom; the table clipped its route column. Scene labels stayed on screen over the documentation after the canvas sank. | Amber light now pools near the eye only. Billboards and ribbons are windowed to zero at their edges. Labels sink with the canvas and hide when unplaced. Nav scrim is a gradient. Code box is bounded. Node framing moved. |
| 2 | The eye was still soft: the box blur wrote back into its source, so the sharp mask channels were blurred too. Row headers were `display: grid`, which broke table rows. The hidden route column still took height and the table swallowed the eye. Phone code collided with the table; scene notes ran off the viewport. | Fixed the blur bug (the mark became crisp and recognisable). Fixed table rows. Route column appears only while selecting. Labels flip side; notes clamp to the viewport. |
| 3 | The eye read as flat vector yellow. Red-only aberration made green fringes. Past code wasted the box. The closing shot did not feel settled. Phones crowded. | Gaze-driven specular bevel on the mark (it now reads as cast amber). No channel-shift aberration. Finished calls fold to one line. Nodes quiet down in the closing shot. Phone stage compacted. |
| 4 | The run pane covered the one row left in the table; route column overflowed; the static (reduced-motion) no-server put the statement on top of the still. | The table morphs further: resetsAt gives way to route while selecting; other rows collapse during the run; phones swap the table for the grant sheet. Static no-server shows the still as a band above the type. |
| 5, GPU | Dead space under the step word; the selected node's label sat on its own thread; a specular wash over the flat face when the eye looked down-right (the closing shot). At 1920 the eye slid under the headline; at 320 the menu button was clipped. | Step word at 14vw. Labels sit away from the eye. Specular limited to the bevel. The scene measures the headlines and keeps the eye clear of them. Compact nav actions on tiny screens. |

Performance fix found by measurement, not by eye: with a 4x CPU throttle the scrub fell
to 60 fps with a 48 ms p95, because the story re-rendered and re-tokenised its code on
every scroll frame and the director wrote custom properties on large subtrees. Tokens are
now cached, the table and code are memoised on derived primitives, and variables are
written only on the elements that read them: 148 fps, 12 ms p95.

## What is still weak

- The scene has only been seen on SwiftShader and an RTX 3080. Neither is a mid-range
  laptop, Safari or Firefox; the light tier and adaptive render scale are untested on
  real low-power hardware.
- At 4x CPU throttle TBT is 257 ms, mostly hydration plus building the mark's masks on
  the main thread. Moving the mask build to a worker or shipping the masks as an image
  would cut it.
- The hero's line reveal staggers by word index, not by measured line, because lines
  are only known after hydration and the reveal starts at first paint.
- The documentation stretch is well set but quiet: the margin thread is the only link
  back to the scene. A second scene beat between the support matrix and the control
  center would help the long middle.
- Stills are 1x for desktop (2x for the phone hero); on high-density screens the reduced
  motion hero is slightly soft.
