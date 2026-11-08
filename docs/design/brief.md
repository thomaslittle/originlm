# Design brief — OriginLM desktop (DRAFT)

Status: **DRAFT**, written for the UI council test run from what the repo
already shows. Tom is the final arbiter; do not treat this as a settled
product brief.

## Product

OriginLM is a local control centre for AI runtimes. It watches the AI
clients and subscriptions on this machine and answers, without reading
accounts it was not given:

1. What is connected, and is that connection actually verified?
2. How much allowance is left on each account, and when does it reset?
3. What has this machine spent or used over time?
4. Which apps and routes can run right now?

It is an alpha desktop app (`crates/originlm-desktop`, GPUI via
`gpui-kit 0.9`). Example mode uses synthetic records and explicitly says
so. Nothing here is a benchmark or a public claim.

## Who uses it

A developer or operator running several paid AI subscriptions at once.
They open the app for a few seconds at a time, usually to answer "can I
keep going, or do I switch accounts?" They are comfortable with dense
tables, but they are not reading the app for pleasure.

## Visual direction (existing, not a new proposal)

- **Palette:** warm graphite `#231f20` / paper `#faf9f5`, restrained amber
  `#eba31a`, and muted state colours. Dark mode is graphite-on-dark, not
  black; light mode is warm paper, not white. The brand mark is an eye.
- **Type:** Red Hat Display, bundled. Mono is the platform's first
  installed candidate (Cascadia Mono on Windows).
- **Density:** compact. The shell already defines a spacing scale
  (4/8/12/16/24/32), a type scale (11/12/13/15/22) and a 44px row.
- **Voice:** plain, factual, honest about missing data. "Unknown" and
  "stale" are different states from "zero". Never invent a number.
- **Motion:** restrained. One eye-logo easing and short panel transitions;
  reduced motion is respected.

## What this app is not

- Not a generic Zed/Linear clone. The warm graphite/amber identity and the
  instrument-panel density are the point.
- Not grayscale. Amber is a signal, not decoration, but it should not be
  removed to look "premium".
- Not a marketing page. No hero, no illustration, no oversized type.
- Not a new product surface in this run. No new screens, features or
  dependencies.

## Anti-goals for the council test run

1. Do not turn the app into a generic Zed/Linear clone.
2. Do not shift the palette grayscale or replace the typeface.
3. Do not trade honesty for polish: an unknown reading must still look
   unknown, not like a zero or a filled meter.
4. Do not block on direction questions. Log them for Tom and make only
   conservative changes.
