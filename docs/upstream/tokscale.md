# Tokscale as an upstream discovery source

OriginLM discovers AI access that is already configured on a machine: installations, profiles, accounts, connections, routes, models, capabilities, entitlements, and quotas. [Tokscale](https://github.com/junhoyeo/tokscale) has spent a long time learning where those clients keep config, sessions, and local databases on Windows, macOS, and Linux.

Tokscale is a knowledge source and a test oracle. It is not a dependency of a released OriginLM build. The native helper, SDKs, and demo never download Tokscale, execute it, or follow its `main` branch.

```text
Tokscale upstream
      │
      │ daily watcher
      ▼
candidate upstream changes
      │
      │ normalization, parity tests, provenance
      ▼
OriginLM discovery adapters
      │
      ▼
released OriginLM
```

The same shape is reserved for later sources. See [upstream/sources.json](../../upstream/sources.json).

## What we consume

From the pinned Tokscale revision, OriginLM adopts discovery facts:

- default profile directories and fallback locations
- environment-variable homes such as `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `GEMINI_CLI_HOME`, `GROK_HOME`, and `LM_STUDIO_HOME`
- XDG data and config roots, including OpenCode's data directory
- Windows `HOME` only when it is an absolute native path
- blank environment values treated as unset
- native path separators, so a Windows profile is not stored as `C:\Users\User/.codex/sessions`
- Antigravity CLI (`antigravity-cli`) versus the home-rooted extension directory
- GitHub Copilot's `~/.copilot` profile
- session-store relatives used only to notice when an upstream layout moves

The audited pin, watched paths, and adapter coverage are in [upstream/tokscale/lock.json](../../upstream/tokscale/lock.json). The extracted client table is [snapshot.json](../../upstream/tokscale/snapshot.json). How each client maps onto a OriginLM adapter is [mappings.json](../../upstream/tokscale/mappings.json).

## What we do not consume

Tokscale is a usage and session aggregator. OriginLM is not a token-usage dashboard. We do not import Tokscale pricing, dashboards, token totals, submission infrastructure, leaderboards, or TUI code.

Two Tokscale paths are intentionally not installations:

- Cursor's Tokscale root is `~/.config/tokscale/cursor-cache`, which is Tokscale's own usage cache. OriginLM keeps the platform Cursor directory.
- Tokscale's `antigravity` client reads Tokscale's config cache. The Antigravity CLI and extension directories are the client locations.

Passive discovery still does not read file contents. cc-mirror `variant.json` files, Copilot `session-store.db`, and OpenCode SQLite files are not opened to decide that a profile exists. Finding `~/.copilot` does not start a Copilot session or claim a subscription.

Ollama is not a Tokscale client. `~/.ollama` remains OriginLM's own rule.

## Provenance

Discovery rules reuse [`EvidenceSource`](../../crates/originlm-contracts/src/evidence.rs). A rule records a basis:

1. **Official client** (`official_protocol` / `official_api`) — the client or its protocol says where configuration lives.
2. **Local qualification** (`local_config`) — OriginLM verified the location itself.
3. **Upstream OSS** (`derived`, adapter `upstream.tokscale`, reference includes the pinned commit) — Tokscale is supporting evidence.
4. **Convention** — inferred, and the weakest source.

`RuleBasis::preferred` selects the higher rank when two sources describe the same rule. A legacy fallback is a role, not a weaker rank: Tokscale can justify a fallback the official docs omit, without replacing a conflicting official path. `CLAUDE_SECURESTORAGE_CONFIG_DIR` is local knowledge and is consulted before Tokscale's `CLAUDE_CONFIG_DIR`.

Tokscale does not become the source of truth. The reference on an upstream rule is the pinned commit, not `main`.

## Pinned revisions

`lock.json` is the revision OriginLM claims to have reviewed. `snapshot.json` is the discovery extract of that revision. Runtime code does not read either file. The parity test reads them so a Tokscale path move fails in CI instead of shipping silently.

Updating the pin is a deliberate change: port the rule into `crates/originlm-discovery`, keep or adjust the provenance, refresh the snapshot from that commit, and update the lock. Do not point the lock at upstream `main` merely because it moved.

## Watcher

[`.github/workflows/upstream-discovery.yml`](../../.github/workflows/upstream-discovery.yml) runs daily and can be started manually. It compares Tokscale `main` with the pin. If the client registry, discovery environment variables, and path literals are unchanged, it exits without opening a pull request.

If they differ, it writes a report, runs the parity test against the new extract, and opens or updates a pull request from `upstream/tokscale-discovery-review`. That pull request does not change adapter behavior and must not be merged automatically. A maintainer decides which facts to adopt.

## Run the audit locally

Check the pin without network access:

```sh
python scripts/upstream_sync.py --offline
```

Compare the pin with Tokscale `main`:

```sh
python scripts/upstream_sync.py
```

Compare a local checkout, or regenerate the reviewed snapshot after an intentional pin update:

```sh
python scripts/upstream_sync.py --source-dir path/to/tokscale --emit-snapshot upstream/tokscale/snapshot.json
```

Parity against the pinned snapshot:

```sh
cargo test -p originlm-discovery --test upstream_parity
```

The sync tool runs that test against a candidate snapshot when upstream discovery changed. Containment is the parity relation: Tokscale's session directory must stay inside the OriginLM profile. A Codex home move fails like this:

```text
DISCOVERY PARITY FAILURE

Client: codex
Platform: Windows
Scenario: windows-normal

Tokscale resolves:
C:\Users\User\.codex-next\sessions

OriginLM resolves:
C:\Users\User\.codex
```

`C:\Users\User\.codex\sessions` inside `C:\Users\User\.codex` passes. A Windows path that still contains a `/` from a relative literal, such as `C:\Users\User/.codex/sessions`, fails the native-separator test. The harness does not require the two projects to share an API, and it does not require Cursor's Tokscale usage cache to equal Cursor's installation directory.

## Example: a Codex path move

1. Tokscale changes `CODEX_HOME`'s fallback from `.codex` to `.codex-next`.
2. The daily workflow diffs `clients.rs`, sees the Codex registry change, and resolves both revisions.
3. Parity fails because `~/.codex-next/sessions` is not inside OriginLM's `~/.codex` profile.
4. The review pull request shows the SHAs, the Codex diff, the `codex` adapter mapping, and the parity failure.
5. A maintainer checks the official Codex client. If the new directory is real, they add it as a profile or legacy fallback with provenance, refresh the snapshot, and update the lock. If it is Tokscale-only, they record the exclusion in `mappings.json` and leave production behavior unchanged.

## Adding OpenUsage or AgentManager

Add an entry to [upstream/sources.json](../../upstream/sources.json) with `status` moving from `planned` to `pinned`, then add `upstream/<source>/lock.json`, `mappings.json`, `NOTICE.md`, and a snapshot. Point the watcher at that source's discovery paths. Keep the parity modes (`contained`, `intentionally-different`) and the same evidence ranking. Do not teach the scanner to call the other project.
