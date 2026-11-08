# Council scope — landing page

Target: `apps/site` (Vite/React static landing page). Branch `ui-council/landing`.

## Screens in scope

One continuous page, reviewed as 12 sections at 1440 and 390 wide:

1. Hero
2. Story (Discover / Select / Grant / Run)
3. How it fits (no server + architecture)
4. SDK
5. Grants
6. Sessions and host tools
7. Integrations
8. Support matrix / providers
9. Control center and usage
10. Security
11. Install
12. Close (final CTA + footer)

## Allowed changes

Major revision is authorised for this run: section order and structure, layout, type scale,
spacing rhythm, colour usage, buttons/CTAs, responsive behaviour, copy and hierarchy. Rebuild
a weak section rather than polish it.

Kept fixed: product name, logo and brand marks, factual claims (versions, dates, support,
limits, "example data"), stack and build.

## Limits

- Up to 3 revision rounds (Round 0 baseline, then Rounds 1–3), stopping early if the panel
  plateaus.
- Each round fixes the 3 lowest-scoring categories or the owners of the biggest visible
  defects.
- No new features, no new dependencies, no unlisted screens or routes.
- No deployment or publishing. One draft PR is the only output.
- Hard gates: WCAG contrast, visible focus, 44px targets on mobile, no horizontal overflow
  at 390px, no truncated headlines/CTAs.
