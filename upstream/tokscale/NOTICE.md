# Tokscale

Local AI Kit learns discovery locations from [Tokscale](https://github.com/junhoyeo/tokscale). Tokscale is an independent project. It does not endorse Local AI Kit, and Local AI Kit is not a Tokscale build, wrapper, or distribution.

- Repository: https://github.com/junhoyeo/tokscale
- License: MIT
- Copyright: Copyright (c) 2025 Junho Yeo
- Pinned commit: `1d9a9395418efc6952944b794097935d7d6fa1e8`
- Latest release noted at pin time: `v4.17.0` (`dcf8d3656bbcecf05d112c6f35f9be328e4f66b7`)

The MIT license text is in [licenses/tokscale-MIT.txt](../../licenses/tokscale-MIT.txt).

## What was taken

No Tokscale source file is vendored or executed. Path and environment rules were reimplemented in `crates/lai-discovery` after reading Tokscale's client registry, path resolver, and scanner. `upstream/tokscale/snapshot.json` is a factual extract of client ids, roots, relative locations, environment variables, and path literals from the pinned revision. It is a review oracle, not a runtime database.

Adopted knowledge includes Codex, Claude Code, Gemini CLI, Antigravity CLI and extension homes, OpenCode's XDG data directory, GitHub Copilot's `~/.copilot` profile, Grok and LM Studio home overrides, Windows `HOME` handling, and native path separators. Details are in [mappings.json](mappings.json).

## What was not taken

Tokscale pricing, token aggregation, dashboards, submission infrastructure, leaderboards, and TUI code are out of scope. Tokscale's Cursor root and its Antigravity `PathRoot::Config` cache are Tokscale's own usage storage. Local AI Kit does not treat them as installations. cc-mirror `variant.json` files are not read during passive discovery, because a passive scan does not read file contents.

When an official client or a Local AI Kit qualification disagrees with Tokscale, the official client wins, then the local qualification, then Tokscale, then convention.
