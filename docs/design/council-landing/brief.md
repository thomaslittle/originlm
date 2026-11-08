# Landing page brief — UI council, landing run

DRAFT for this review pass. Written from the repository's own site, README and docs; no new
product claims.

## Product

OriginLM is a local runtime and SDK for building on the AI clients a user already has. It
finds installed clients (Claude Code, Codex, Cursor, OpenCode, Gemini CLI, Grok, Ollama and
others), exposes one typed API (TypeScript, Rust, Python) over the same JSON-RPC protocol,
and runs turns only inside scoped, revocable grants the host approves. There is no OriginLM
server or relay. A native desktop control center shows per-account allowance windows,
connected apps and revocation. MIT-licensed alpha; packages are published on npm, crates.io
and PyPI; signed desktop installers are not published yet.

## Audience

Developers evaluating a runtime for a desktop app, IDE extension or local agent. They already
know what an SDK is. They want to see the permission model, the API shape and the evidence
that it works before they invest a weekend.

## Page job

1. In the fold: say what it is, who it is for, and give one action (install / quick start).
2. Show one real request flow (discover, select, grant, run) with the host approval step.
3. Prove the local boundary and the permission model.
4. Show the SDK, sessions and framework fit with real method names.
5. Show the support matrix as evidence, not marketing.
6. Show the user's side (control center and usage).
7. Install, security, close.

## Brand and constraints

- Keep the OriginLM name, logo and marks (`apps/site/src/assets/brand`, `brand/`).
- Dark, warm, unlit ground; amber is the only light. Brand fonts: Fraunces (display, roman),
  Red Hat Display (body), Martian Mono (code/data). Do not change the typefaces, do not shift
  the page grayscale, do not add a second accent.
- Keep every factual claim: versions, dates, limits, provider support, and "example data".
- No invented features, benchmarks, testimonials, customers or pricing.
- Static React/Vite site; no framework swap, no new build tooling.
- WCAG AA text contrast, visible keyboard focus, no horizontal overflow at 390px, no
  truncated headlines or CTAs.

## Anti-goals

- Do not turn it into a generic Zed/Linear clone.
- Do not add glassmorphism, neon gradients, bento grids, fake window chrome or stock art.
- Do not hide the product's local-first / permission story behind abstract marketing copy.

## Capture note for reviewers

The supplied captures are the page's static (reduced-motion / no-WebGL) layout at 1440 and
390 wide. That layout is a real presentation of the page and every section is present. The
live page adds a WebGL eye and a pinned scroll story; those are not scored from stills.
