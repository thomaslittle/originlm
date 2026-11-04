# Detection lab

Status: proposal. Nothing here is built yet. Names, paths and thresholds are starting points to argue with.

## The problem

OriginLM's value is that it reads what a user's own clients report: which account is signed in, which plan, which models and options, how much allowance is left and when it resets, and what a session is allowed to do. Every one of those facts comes from a vendor client we do not control, and every one of those clients ships often.

Our recent history is mostly this class of bug:

- `fix(opencode): resolve the native executable before execution`
- `fix(cursor): launch pinned runtime behind Windows wrappers`
- `fix(claude): label undocumented usage buckets`
- `fix(desktop): offer OpenCode models in the Playground`

Each was found by a person noticing something wrong on one machine, on one version, on one OS. We learn about drift late, from a single data point, and we cannot say which client versions a fix applies to. The support matrix already separates five axes of evidence (code implemented, synthetic contract-tested, OS-executed, installed-client metadata checked, live inference checked; see [VERSIONING](../adapters/VERSIONING.md)). Today the last three are manual and undated.

A dedicated, always-on machine fleet closes that gap. It installs every supported client the way its vendor recommends, keeps each one current, runs our real detection and metadata code against it on a schedule, and turns any change in what clients report into a finding with a version range attached.

Other tools make the same failure visible. OpenUsage pins a `claude-cli/<version>` User-Agent because the usage endpoint rejects unknown or outdated client versions. A version string quietly going stale is exactly what a lab should catch before a user does.

## Goals and non-goals

Goals:

1. Detect, within a day of release, that a new client version changed something we read (paths, wrappers, auth state, plan, models, options, usage windows, approvals).
2. Know the first bad version and the last good one for each regression.
3. Keep the support matrix's evidence axes truthful and dated, per client version and OS.
4. Produce regression fixtures automatically, so a field bug becomes a permanent test.
5. Cover Windows, WSL2 Ubuntu, and later macOS, since path layout, shims and credential stores differ.

Non-goals:

- Not a benchmark of model quality or speed.
- Not a place to use real users' accounts or data.
- No inference by default. Generation is an explicit, budgeted tier (see Probe tiers).
- Not a replacement for unit and fixture tests. The lab feeds them.

## Architecture

```
                  upstream release feeds (npm dist-tags, GitHub releases,
                  installer endpoints, package registries)
                                   |
                         version watcher (cheap, hourly)
                                   |  new version or changed installer
                                   v
        orchestrator (GitHub Actions on self-hosted runners, or lab/ harness)
                 |                 |                     |
          Windows 11 host     WSL2 Ubuntu lane      macOS lane (later)
          lanes per client    lanes per client      lanes per client
                 |                 |                     |
        install -> probe ladder -> redacted snapshot + raw-shape capture
                                   |
                             drift engine (diff)
                                   |
        findings: issue + artifacts + proposed fixture PR + matrix update
```

### Lanes

A lane is one client at one version on one OS in one auth state, in isolation:

- Own user profile or home directory, own `PATH`, own config roots (`CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `GEMINI_CLI_HOME`, `XDG_*`, `%APPDATA%`), so lanes cannot contaminate each other or the host.
- Disposable. A lane is rebuilt from a snapshot (Hyper-V checkpoint, WSL distro export, or a fresh user profile) before each run, with a persistent, separately stored credential bundle for lanes that need to stay signed in.
- Named by `client / version / os / install-channel / auth-state`, for example `claude-code/2.x.y/win11/native-installer/signed-in`.

### Client manifest

One file lists every client and how to track it, so adding a client is a data change:

```toml
# lab/clients.toml (sketch)
[claude_code]
display = "Claude Code"
channels = ["native-installer", "npm"]          # each is an official, vendor-documented route
version_source = { kind = "npm", package = "@anthropic-ai/claude-code" }
track = ["latest", "previous-minor", "oldest-supported"]
config_roots = ["CLAUDE_CONFIG_DIR", "~/.claude"]
adapter = "crates/originlm-adapters/src/claude"

[codex]
channels = ["npm", "native-release"]
version_source = { kind = "github-release", repo = "openai/codex" }
...
```

The manifest records, per client: which official install channels exist, where its versions are published, which version set to keep (latest, previous minor, oldest we claim to support, and any pre-release channel as a canary), where it keeps config and credentials per OS, and which adapter owns it. Install commands live in the manifest and are copied from each vendor's own documentation. They must be re-verified when the vendor changes its recommended route, which the installer-change check below detects.

Initial clients: Claude Code, Codex, Cursor (`cursor-agent`), OpenCode, Gemini CLI, Antigravity, Grok, Muse Code, GitHub Copilot CLI, Pi, Ollama and LM Studio (local runtimes). Agent libraries and SDKs that apps embed can join later as a second class of lane.

### Version watcher

Runs hourly and is deliberately dumb: poll each `version_source`, compare to the last seen value, and also hash each vendor install script or manifest. A new version, a new pre-release, or a changed installer hash enqueues the affected lanes. Installer changes matter on their own: a changed `curl | sh` script is both a compatibility signal and a supply-chain signal.

## Probe tiers

Each lane runs as many tiers as its auth state allows. Higher tiers are rarer and cost more.

| Tier | What it does | Credentials | Runs |
|---|---|---|---|
| T0 Install | Install via the official channel, capture the version string, binary kind (native, script, wrapper, shim), install path and its resolved target. Uninstall and reinstall once a week to prove the channel still works. | none | every version change |
| T1 Passive detection | Run OriginLM's passive discovery against the lane. Assert the installation, profile and wrapper resolution we expect, including Windows `.cmd` shims and symlinked executables. | none | every version change |
| T2 Metadata | Run the real `originlm` binary with metadata allowed (the model of `scripts/check_native_metadata.py`): auth state, account identity, plan, model catalog and option descriptors, usage windows and reset times. Zero generation requests. | test account, signed in | nightly, and on version change |
| T3 Permissions | With a canned, near-zero-cost prompt on opt-in lanes: confirm read-only mode is honored, approvals reach the host, cancel stops the turn, host MCP tools attach, and a denied grant blocks execution. | test account | weekly, and on version change for approval-affecting clients |
| T4 Live inference | One tiny real generation per lane to confirm the account can actually run a turn and that streaming and images behave. | test account with a spend cap | weekly at most, hard budget |

Auth states per client include signed out, signed in, expired or revoked token, API-key mode, and a profile with several accounts, since our identity logic depends on all of them.

## Drift detection

The lab's main product is a diff, not a pass or fail.

1. **Raw-shape capture.** For every client output we parse (JSON from app-server calls, `status` and `about` text, usage endpoints reached through the client, on-disk config), record a *shape fingerprint*: the set of key paths, types, enum-like value sets and units, with all values dropped. This is what makes the capture safe to store and diff.
2. **Normalized snapshot.** Store our own redacted `Snapshot` output (providers, connections, models, quota pools, issues) for each run. Redaction uses the same secret-redaction path the product uses, applied again before upload.
3. **Two comparisons per run.** Against the previous run of the same version (flaky or environmental), and against the previous version of the same client (real drift).
4. **Classify.**

| Class | Example |
|---|---|
| New or removed field | a usage bucket appears, or a plan field vanishes |
| Renamed or re-scoped | `seven_day_*` buckets replaced by a `limits[]` entry kind |
| Unit or scale change | seconds to milliseconds, percent 0 to 1 versus 0 to 100 |
| Model or option change | catalog entry added, option enum gained a value |
| Auth flow change | new login method, new "signed out" error text, token store moved |
| Layout change | binary moved, wrapper added, config root renamed |
| Behavior change | approvals no longer requested, read-only not honored |
| Environmental | network failure, rate limit, vendor outage; never opens a finding on its own |

5. **Bisect.** When a regression appears, replay the relevant tier across the stored version list to pin the first bad version. Install is cheap if versions are cached, so bisect is automatic.

## Findings

A finding is opened when a classified drift touches something an adapter reads, or when any tier fails twice in a row on a clean lane.

Each finding carries: client, version range (last good to first bad), OS and install channel, drift class, severity, the redacted before and after, the owning adapter, and a fingerprint so the same drift is not filed twice.

Outputs:

- A GitHub issue from a template, labeled by client, OS and class.
- A proposed fixture PR: the new shape as a synthetic fixture under `scripts/fixtures`, plus a failing test where we can express one. A human reviews and merges.
- A dated update to the support matrix evidence for that client, OS and version, so the "installed-client metadata checked" axis is a measured claim ("Claude Code 2.1.x, Windows 11, 2026-11-04") and never a vague one.
- A notification for high-severity classes (auth change, behavior change, permission regression).

A finding is never auto-merged into adapter logic. The lab proposes; people decide.

## Safety and security

This fleet holds credentials and runs vendor installers, so it is the most sensitive thing in the project.

- **Dedicated test accounts only.** One per vendor per lane group, never a personal or maintainer account. Check each vendor's terms for automated and multi-machine use of a subscription before enabling T2 and above; if a vendor prohibits it, that lane stays at T0 and T1.
- **Spend caps and budgets.** T3 and T4 prompts are fixed and tiny. A hard monthly budget per vendor; the runner stops when it is hit.
- **Secrets stay on the runner.** Credential bundles live in a vault or OS credential store on the lab hosts, are mounted into a lane only when needed, and are never uploaded. Artifacts are redacted twice and contain shapes, not values.
- **Installer hygiene.** Run official installers only inside a lane. Pin and hash each install script, alert on change, and never feed an installer's output to anything with credentials.
- **No untrusted code on the runner.** Self-hosted runners on a public repository must not run workflows triggered by forks or pull requests. The lab runs only on `schedule` and `workflow_dispatch` from the default branch, on ephemeral lanes. The existing `upstream-discovery.yml` already uses a report-only posture when workflow permissions are narrow; the lab follows the same default.
- **Network.** Egress is logged per lane. Anything unexpected in a T0 or T1 run (those should need almost none) is itself a finding.
- **Isolation of the host.** Lanes cannot read the host profile or each other. Rebuild from snapshot before every run.

## Infrastructure

- **Windows 11 host** (the current dev box class). Lanes as separate user profiles for T0 to T2, Hyper-V checkpoints when a clean OS image matters (shims, PATH, wrapper behavior).
- **WSL2 Ubuntu.** One exported distro per lane, imported fresh each run. Also exercises the Windows-to-WSL boundary, where users hit path and wrapper problems.
- **macOS (later).** A Mac mini with per-lane user accounts. macOS is in the CI matrix but has never been platform-qualified, so this unlocks the missing evidence axis.
- **Orchestration.** GitHub Actions workflows targeting self-hosted runner labels (`lab-win`, `lab-wsl`, `lab-mac`) that call a small harness in a new `lab/` directory. Keep the harness a plain script runner so it also works from a terminal.
- **Storage.** Redacted snapshots, shape fingerprints and logs as workflow artifacts, with a long-lived index (a git branch or object store) for version history and bisect.
- **Public surface.** A generated matrix page of client by version by OS with the five evidence axes and dates, built from the lab's results. That page is the honest answer to "does it work with my version", and it is a strong credibility piece for the site.

## Schedule

| Cadence | Work |
|---|---|
| Hourly | Version watcher, installer-hash check |
| On new version | T0 to T2 on affected lanes, plus T3 where approvals could change |
| Nightly | T2 on the latest and previous-minor of every client |
| Weekly | Full matrix including oldest-supported, clean reinstall, T3 on opt-in lanes, T4 within budget |
| Monthly | Review: stale lanes, manifest accuracy, budget, account health |

## Rollout

1. **Phase 0, a few days.** `lab/clients.toml`, version watcher, T0 and T1 for Claude Code, Codex, Cursor and OpenCode on the existing Windows machine. Output: a table of versions and detection results.
2. **Phase 1.** T2 with test accounts, snapshot and shape capture, the diff engine, issue creation. Reuse `check_native_metadata.py` and `adapter_conformance.py` as the first probes.
3. **Phase 2.** WSL2 lanes, the oldest-supported and previous-minor matrix, bisect, fixture-PR proposals.
4. **Phase 3.** The rest of the clients, T3 permission checks, the public matrix page, notifications.
5. **Phase 4.** macOS lanes, T4 inference under budget, agent-library lanes.

## Open questions

- Terms of service per vendor for automated, multi-machine use of subscription test accounts. This decides how far each client can climb the tiers.
- How to keep lanes signed in across rebuilds when vendors use device-code or browser login, and what to do on 2FA or periodic re-auth.
- Who owns the hardware, the accounts and the monthly spend, and what happens when a vendor locks an account.
- Whether agent libraries that apps embed belong in the same lanes or a second harness.
- How much of the lab can be hosted publicly, and whether the matrix page should be opt-in per vendor.

## Success measures

- Median time from an upstream release to a detected drift finding is under 24 hours.
- For every supported client, the latest version has a T2 result under 48 hours old on every platform we claim.
- Every regression fix links a finding with a last-good and first-bad version.
- The false-positive rate stays low enough that findings are read, tracked as the share of findings closed as environmental.

## Related

- [Compatibility and versioning](../adapters/VERSIONING.md)
- [Support matrix](../adapters/SUPPORT-MATRIX.md)
- [Adapter authoring](../adapters/AUTHORING.md)
- `.github/workflows/upstream-discovery.yml`, the existing upstream watch
- `scripts/check_native_metadata.py` and `scripts/adapter_conformance.py`, the probes to start from
