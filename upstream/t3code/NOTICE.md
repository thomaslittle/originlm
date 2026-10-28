# T3 Code

Local AI Kit reviewed [T3 Code](https://github.com/pingdotgg/t3code) as an upstream source for provider detection, local and cloud model access, and Sign in with ChatGPT. Released builds do not clone, download, or execute T3 Code, and they do not follow its `main` branch.

| Field | Value |
| --- | --- |
| Pinned commit | `e9236274a3a6e0c731a7df4092db1e686f039068` (`main`) |
| Latest nightly at pin time | `v0.1.4-nightly.20261027.3770` (`7c3b3b3605efa695e9948051b569937fc1482c0f`) |
| Latest stable at pin time | `v0.1.3` (`d2c440312e5f39d59827ed466838a623516454d6`) |
| License | MIT |
| Copyright | Copyright (c) 2026 T3 Tools Inc. |

There is no `nightly` branch. Nightly builds are scheduled GitHub prereleases cut from `main` by `.github/workflows/release.yml`. A manual stable release builds the latest nightly's commit.

No T3 Code source file is copied into this build. The Pi profile, the OpenCode native binary directory, and the ChatGPT refresh behavior below were reimplemented in Rust from the pinned revision. The earlier Muse port and its copied transcripts are recorded separately in [THIRD_PARTY_NOTICES.md](../../THIRD_PARTY_NOTICES.md).

Adopted at this pin:

- Pi coding agent command `pi`, profile `PI_CODING_AGENT_DIR` or `~/.pi/agent`, session directory `sessions/` under that profile. Passive inventory only. First adopted at `42da166d131017335a1158d0b379dd68b8985796`.
- OpenCode installer binary directory `~/.opencode/bin`. First adopted at that same commit.
- ChatGPT OAuth: `earliest_refresh_at` is honored, ID tokens may be RS256 or ES256 with a five-second clock skew, and a sign-in that omitted plan sharing asks for consent on the next attempt. First adopted at that same commit.
- Grok metadata probes set `GROK_DISABLE_AUTOUPDATER=1`. Text sessions do not. Reimplemented from [2a7bd167](https://github.com/pingdotgg/t3code/commit/2a7bd1673c7bdad4304b7771944e73c29ecd3c01).
- A logged-out `grok models` probe is repeated once. A definite recheck replaces the first result ([a2157ece](https://github.com/pingdotgg/t3code/commit/a2157eced708fd25fab7b616f5a68cb65312eaed)).
- Launch resolution rejects a native Codex binary that is missing the adjacent `codex-code-mode-host` executable ([348888a7](https://github.com/pingdotgg/t3code/commit/348888a767433464dd47e723ec6509dc5a8282db)).
- Known ChatGPT `planType` values keep the native id and use the current display labels ([33f1fe5e](https://github.com/pingdotgg/t3code/commit/33f1fe5ebe53fdd8d18b6a5d63dfd29c336ba5a9)).
- Codex `config/read` `service_tier` selects an advertised choice, with `fast` aliasing `priority` ([3b844dab](https://github.com/pingdotgg/t3code/commit/3b844dab5ced7277ce4a2cf0e5e11d0f04e62241)).
- Codex `individualLimit.remainingPercent` is a "Monthly credit limit" window ([4c18faf6](https://github.com/pingdotgg/t3code/commit/4c18faf62cd1685b0243dcacc1a94e333c2b93ff)).
- Claude monthly spend prefers an enabled matching-currency budget, then enabled extra usage ([fd8dd516](https://github.com/pingdotgg/t3code/commit/fd8dd51654f855dbf4e23c457701f5088c59b689)).
- Passive `fx` inventory: command `fx`, profile `~/.fx`. Reimplemented from [6df241d23](https://github.com/pingdotgg/t3code/commit/6df241d23a8b82ff786fe3253dfb08645933db26). The binary is not launched.
- ACP `sessionUpdate` `notice` is a warning, not assistant text. The client advertises `session.notices`. Reimplemented from [323d9611](https://github.com/pingdotgg/t3code/commit/323d9611ff49521357fdc205f667614602ec46b6).

Reviewed and not adopted: Ollama, LM Studio, and llama.cpp (absent upstream), direct cloud HTTP APIs (absent upstream; Local AI Kit already has explicit API routes), ACP Registry downloads including the Windows ARM `windows-x86_64` fallback and Gemini CLI, multi-account ChatGPT handoff, Codex shadow homes, Codex release downloads (including the Windows system-tar extract), Windows Claude launcher-shim rewriting, managed Antigravity installs, and the bundled Cursor SDK (including Auto-review). Also not adopted from this range: running fx (status, models, ACP, and a medium effort default), shortened model labels ([56695ae3](https://github.com/pingdotgg/t3code/commit/56695ae3ec2aad931030af1e7e43852f741b9eef)), a seven-day new-model badge ([0790e3891](https://github.com/pingdotgg/t3code/commit/0790e3891b7470d5a33292dedc4d4ae62d02ff29)), the Cursor SDK bump to 1.0.38 ([850d1e7a](https://github.com/pingdotgg/t3code/commit/850d1e7a6c5ffd894c48e9182169cf8e25097db5)), Grok billing that reads `auth.json` ([79f2bae6](https://github.com/pingdotgg/t3code/commit/79f2bae6)), spawning Muse to learn `museHome` ([c8933536](https://github.com/pingdotgg/t3code/commit/c8933536)), a forced `gpt-6-astra` effort of medium, ChatGPT profile-key hashing, shell-profile PATH hydration, and T3 release CI. Codex's 20-second status probe already matched Local AI Kit. ChatGPT OAuth endpoints and algorithms did not change.

The daily diff list is [UPSTREAM_T3CODE.md](BASELINE.md). The factual extract is [snapshot.json](snapshot.json).
