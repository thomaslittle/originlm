# Contributor and PR automation

This is a **soft start**. The universal-product guard is the only hard block. Everything else is advisory until real pull requests show what is useful.

## What runs

| Automation | Files | Behavior |
| --- | --- | --- |
| Label sync | `.github/workflows/labels.yml`, `.github/labels.json` | Creates or updates the repository labels. |
| Path labels | `.github/labeler.yml`, `.github/workflows/labeler.yml` | Applies `rust`, `typescript`, `python`, `documentation`, `ci`, `release`, `policy`, `demo`, `adapters` from changed paths. |
| Release Drafter | `.github/release-drafter.yml`, `.github/workflows/release-drafter.yml` | Keeps a draft release from merged PRs, groups changes, and autolabels PRs by title/files. It does not publish packages or create a tag until a maintainer publishes the draft. |
| Stale | `.github/workflows/stale.yml` | Warns after 60 inactive days, closes after 21 more. Exempts `pinned`, `security`, `good first issue`, `help wanted`. |
| First interaction | `.github/workflows/first-interaction.yml` | Welcomes first-time issue and PR authors. |
| PR title | `.github/workflows/pr-title.yml` | Nudges toward conventional titles (`feat:`, `fix:`, `docs:`, ...). Non-blocking for now. |
| Universal product | `.github/workflows/policy.yml`, `.github/scripts/check_universal_policy.py` | Hard block on known consumer/business names. Fails with the reason and posts it on the PR. |

## Release and tag flow

1. Merge PRs. Release Drafter updates the draft release and its notes.
2. Review the draft notes and set the exact version/tag (for example `v0.1.0-alpha.21`).
3. Run the preparation workflows for that source commit.
4. Review the generated artifacts and evidence.
5. Publish the draft release. This creates the tag.
6. Run the gated registry publishers with the release version.

A draft release or tag is not a registry publication. The publish workflows remain the authority for artifacts and credentials.

## Tuning as real PRs arrive

- Add path labels when a new area appears.
- Add release categories or labels when a category is missing.
- Adjust stale timings if real contributors need more time.
- Promote advisory checks (PR title, size) to required only after false positives are low.
- Keep the universal-product policy strict. It is the product boundary, not a style rule.
- Review labels and release notes monthly. The configs are the feedback loop.

## Deliberately not enabled yet

Auto-merge, auto-approve, required conventional-title checks, automatic version bumps, and tag-triggered publication are off until the contribution flow is proven. Add them only after real PRs show the need and the false-positive rate is understood.
