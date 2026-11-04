# Detection accuracy ledger

This ledger records what OriginLM detects, how it is proven, and what remains
unproven. It is deliberately conservative: a fixture test proves the parser and
the path rules, not a real installation. A real installation is only marked
proven when a run on that OS recorded the result.

Baseline for this change (Linux, `2026-11-04`):

- `cargo test -j 4 --workspace --all-targets --locked --no-fail-fast` — 730 passed, 0 failed before the change; 787 passed, 0 failed after it.
- `npm run build` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — passed.
- `cargo fmt --all --check` — passed.
- `scripts/verify.py --profile javascript` — passed; `npm run test:react` — passed; cached-usage Node integration tests — passed; `scripts/check_project.py` — passed; `scripts/check_generated.py` — passed.
- `scripts/verify.py --profile all` stops at the Python SDK/generation tests on this host because it has Python 3.10. The SDK uses `asyncio.timeout` and the generated bindings import `typing.Never`, both Python 3.11+. That is an environment limit, not a repository failure; CI pins Python 3.12. Everything before and after that point that does not need Python 3.11 was run separately.
- Linux real-install simulation: installed `@anthropic-ai/claude-code`, `@openai/codex`, `@google/gemini-cli`, and `opencode-ai` into `$HOME/.local` with npm, then ran `env -i HOME=... PATH=/usr/bin:/bin originlm scan --json --include-locations`. All four were found through `$HOME/.local/bin`; npm launcher symlinks kept their `claude`/`codex`/`gemini`/`opencode` names instead of resolving to `cli.js`/`codex.js`.
- CI real-install proof on the merged head: [run 41825120577](https://github.com/thomaslittle/originlm/actions/runs/41825120577) passed on Ubuntu 24.04, Windows 2025, macOS 15 (arm64), and macOS 15 Intel. Each job installed the npm CLIs plus Kiro with the official installer, then found all five tools with both the inherited PATH and a GUI-like bare PATH.

Legend:

- **Real** — a real installation was found and its executable path was recorded.
- **Fixture** — a deterministic fixture proves the parser/path rule.
- **Partial** — some operations are proven, others are not.
- **Unproven** — known path/parser exists but no run proves it.
- **Unsupported** — intentionally not implemented.

## Tool × OS × install method

| Tool | Linux | Windows | macOS | Proof today | Notes |
| --- | --- | --- | --- | --- | --- |
| Codex | **Real** (npm prefix, bare PATH) + PATH + version managers | **Real CI** (npm prefix, bare PATH) + `%APPDATA%\npm` + WinGet/scoop/Programs | **Real CI** (npm prefix, bare PATH) + Homebrew/npm prefixes | Real on all four CI runners; fixture (metadata/execution) | Version comes from the client `userAgent`; models from `model/list`. |
| Claude Code | **Real** (npm prefix, bare PATH) + PATH + `~/.claude/local` | **Real CI** (npm prefix, bare PATH) + `%APPDATA%\npm` + WinGet/scoop | **Real CI** (npm prefix, bare PATH) + Homebrew/npm prefixes | Real on all four CI runners; fixture (metadata/process) | Models come from Claude Code's own `init.models`, not a fixed catalog. The native client owns its Keychain entry; OriginLM never reads it, and no existence-only UI indicator is implemented. |
| Gemini CLI | **Real** (npm prefix, bare PATH) + PATH + T3 managed tools | **Real CI** (npm prefix, bare PATH) + `%APPDATA%\npm` + Programs | **Real CI** (npm prefix, bare PATH) + Homebrew/npm prefixes | Real on all four CI runners; fixture (ACP metadata/execution) | — |
| Antigravity | PATH + `~/.gemini/antigravity-*` | PATH + per-user Programs | PATH | Fixture (passive + ACP) | Not live-qualified. |
| Grok | PATH | PATH + Programs | PATH + Homebrew | Fixture (passive + ACP) | Not live-qualified. |
| OpenCode | **Real** (npm prefix + local symlink dedupe) | **Real CI** (npm prefix, bare PATH) + `~/.opencode/bin` + Programs | **Real CI** (npm prefix, bare PATH) + `~/.opencode/bin` | Real on all four CI runners; v1/v2 fixtures | Metadata/usage are fixture-tested; no live account probe in this change. |
| Muse Code | PATH, `LOCAL_AI_KIT_MUSE_BINARY` | PATH, `%LOCALAPPDATA%\Programs\muse` | PATH | Fixture (MSP) | No live Meta turn. |
| Cursor | PATH, `~/.config/Cursor` | PATH, `%APPDATA%\Cursor` | PATH, `~/Library/Application Support/Cursor` | Fixture (cache, model list) | Usage requires the separate `--allow-usage cursor` grant. |
| GitHub Copilot | `~/.copilot` | `%USERPROFILE%\.copilot` | `~/.copilot` | Fixture (passive) | Session database is not opened. |
| Pi | PATH, `PI_CODING_AGENT_DIR`, `~/.pi/agent` | same | same | Fixture (passive) | Session files are not opened. |
| fx | PATH, `~/.fx` | same | same | Fixture (passive) | Binary is not launched. |
| Ollama | explicit endpoint / `~/.ollama` + `originlm models` | explicit endpoint + `originlm models` | explicit endpoint + `originlm models` | Fixture (manifest inventory) | No network scan. On-disk ids/names/sizes are read only through the explicit CLI; no capability claims. |
| LM Studio | explicit endpoint / `~/.lmstudio` + `originlm models` | explicit endpoint + `originlm models` | explicit endpoint + `originlm models` | Fixture (GGUF inventory) | `~/.lmstudio/models` and `~/.cache/lm-studio/models`; a custom model directory is not read yet. |
| Kiro | **Real CI** (official installer, `~/.local/bin`) + PATH + version managers | **Real CI** (MSI, `%LOCALAPPDATA%\Kiro-Cli`) + `%ProgramFiles%\Kiro-Cli` | **Real CI** (official installer, `~/.local/bin`) + Homebrew/npm prefixes | Real on all four CI runners; fixture (metadata on Unix) | `kiro-cli whoami` and `chat --list-models` are parsed; signed-out sessions never request models. |

## What changed in this pass

- **PATH-independent executable search.** `ScanEnvironment` now carries a
  per-OS fallback list (Homebrew, `/usr/local/bin`, `/snap/bin`, flatpak, Nix,
  npm prefixes, nvm/fnm/mise/asdf/volta shims, WinGet, scoop, Chocolatey,
  per-user Programs). A GUI launch with `/usr/bin:/bin` still finds installed
  clients. Diagnostics record `*_EXECUTABLE_FROM_FALLBACK_LOCATION` when a
  fallback directory supplied the hit.
- **Canonical dedupe.** A command reached through both `PATH` and its real
  install directory is one installation. Unix symlinks resolve to the real
  target; the real path is retained. This fixes OpenCode appearing twice when
  `/usr/local/bin/opencode` points at `~/.opencode/bin/opencode`.
- **Kiro.** Passive provider + `~/.kiro` profile, `sessions/cli` as a known but
  unopened store, and a CLI metadata adapter (`--version`, `whoami`,
  `chat --list-models`). No credential file is read; `KIRO_API_KEY` is passed
  through but never serialized.
- **Claude token units.** Result usage now emits `cache_read_tokens`,
  `cache_creation_tokens`, and `reasoning_tokens` separately from
  `input_tokens`/`output_tokens`.
- **Codex per-turn token delta.** `thread/tokenUsage/updated` total counters
  are turned into this turn's delta, with `last` as the reset fallback.
  Cache reads/writes and reasoning tokens are separate units. The old
  thread-wide `session_tokens` fallback remains for older app-servers.
- **Session-history totals.** `originlm usage --json` reads Claude Code and
  Codex JSONL history with bounds and explicit diagnostics. It never reads
  prompts, tool output, or credentials. The public service/desktop surface is
  not wired yet.
- **On-disk local models.** `originlm models --json` lists Ollama manifests and
  LM Studio GGUF files without a running server and without capability guesses.
- **Upstream pins.** T3 Code is pinned at `8b44f42b` (nightly
  `v0.1.8-nightly.20261104.4165`, stable `v0.1.7`); Tokscale is pinned at
  `b26c433d` (`v4.21.0`). Tokscale parity passes against the new snapshot.
- **Scan location evidence.** `originlm scan --json --include-locations` adds
  the private executable/profile paths for local debugging and CI. The default
  public snapshot still omits host paths.
- **Usage service.** `usage.history` and `costs.read.historical` serve the same
  bounded totals to the authenticated broker; the desktop Costs page shows
  provider totals. Gemini CLI, Grok, and Kiro readers join Claude Code and
  Codex.
- **Model service.** `models.local` serves Ollama/LM Studio on-disk inventories
  to the desktop Models page. An LM Studio `modelDirectory` in
  `LMStudio/config.json` is honored.
- **Detection status.** `detection.status` reports found / configuration-only /
  not-found for every known provider with diagnostics and locations checked;
  the desktop Diagnostics page renders it. `scan --probe-versions` records raw
  `--version` output for real-install evidence.
- **CI.** `macos-15` (arm64) joins the main verification matrix. `Detection
  real install` covers npm CLIs and Kiro on all four runners, plus Ollama on
  Linux and macOS (official installer/Homebrew) with a pulled `smollm2:135m`
  model read from disk after the server stops.
- **Upstream cross-check.** CodeNotch `6522e9f` (`1.24.0`, MIT) is pinned as a
  session-layout/token-accounting oracle; no source is copied.

## Token-history coverage

| Provider | Source | Status |
| --- | --- | --- |
| Claude Code | `projects/`, `transcripts/` JSONL | Fixture; service/API/desktop; cross-checked |
| Codex | `sessions/`, `archived_sessions/` JSONL | Fixture; service/API/desktop; cross-checked |
| Gemini CLI | `~/.gemini/tmp` JSON/JSONL | Fixture; service/API/desktop |
| Grok | `~/.grok/sessions/**/updates.jsonl` | Fixture; service/API/desktop |
| Kiro | `~/.kiro/sessions/cli/*.json` | Fixture; service/API/desktop |
| Cursor | dashboard/cache only | Separate `--allow-usage cursor` grant; no transcript totals |
| GitHub Copilot | session database | Not opened by policy |
| Antigravity | conversation database | Not opened by policy |

## Remaining boundaries

The detection work is complete for the scope below. These are deliberate
boundaries, not silent misses:

1. **Install-method coverage.** CI proves npm-installed Codex, Claude Code,
   Gemini CLI, OpenCode, and Kiro on Ubuntu, Windows, macOS arm64, and macOS
   Intel, plus Ollama via the official installer on Linux and Homebrew on
   macOS. Alternate npm prefixes, Homebrew-installed Claude/Codex/Gemini,
   LM Studio's app/CLI, Copilot, and every vendor-specific installer are not
   individually exercised yet.
2. **Token-history boundaries.** Cursor's usage is a dashboard/cache surface,
   not a transcript token feed, and remains the separate `--allow-usage cursor`
   grant. Copilot and Antigravity store conversation data in databases that
   OriginLM does not open during history reads. `usage.history` omits providers
   with no readable tokens rather than reporting a silent zero.
3. **Quota-only provider work.** Grok Legacy/Privacy-mode quota and Antigravity
   rate-limit reset extraction were not adopted: they require credential-file
   or provider-billing endpoint access outside OriginLM's policy. Native quota
   surfaces that are already published remain in `usage.read`.
4. **Claude update status.** OriginLM does not report client update status, so
   no native-release update check is performed.
5. **macOS interactive qualification.** CI runs detection, the full verify
   matrix, and real installs on both macOS runners, but this Linux host cannot
   run a human interactive macOS session.

Everything else in the original detection brief has a passing fixture or CI
test. The status matrix above is the evidence ledger.
