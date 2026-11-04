# Detection accuracy ledger

This ledger records what OriginLM detects, how it is proven, and what remains
unproven. It is deliberately conservative: a fixture test proves the parser and
the path rules, not a real installation. A real installation is only marked
proven when a run on that OS recorded the result.

Baseline for this change (Linux, `2026-11-04`):

- `cargo test -j 4 --workspace --all-targets --locked --no-fail-fast` — 730 passed, 0 failed before the change; 776 passed, 0 failed after it.
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
| Kiro | **Real CI** (official installer, `~/.local/bin`) + PATH + version managers | **Real CI** (MSI, `%ProgramFiles%\Kiro-Cli`) + `%LOCALAPPDATA%\Kiro-Cli` | **Real CI** (official installer, `~/.local/bin`) + Homebrew/npm prefixes | Real on all four CI runners; fixture (metadata on Unix) | `kiro-cli whoami` and `chat --list-models` are parsed; signed-out sessions never request models. |

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
- **CI.** `macos-15` (arm64) joins the main verification matrix. A new
  `Detection real install` workflow installs npm CLIs and Kiro with official
  installers, then scans with both the inherited PATH and a GUI-like bare PATH.

## Known gaps

1. Real-install CI is green on all four runners for the npm-installed CLIs and
   Kiro. It does not yet cover Homebrew installs, Ollama, LM Studio, or every
   alternate installer for each tool.
2. macOS Keychain: Claude Code keeps its login in the Keychain. OriginLM
   never reads the credential file or the Keychain; the native `claude` client
   owns that store, so metadata and usage must go through the CLI. An
   existence-only Keychain indicator in the UI is not implemented.
3. `CLAUDE_SECURESTORAGE_CONFIG_DIR` is real but undocumented. It is present
   in the Claude Code binary since roughly 2.1.144 and moves only the
   credential store (on macOS, a distinct Keychain service); see
   [anthropics/claude-code#79223](https://github.com/anthropics/claude-code/issues/79223).
   The variable stays ahead of `CLAUDE_CONFIG_DIR`, and OriginLM does not read
   either credential file.
4. Session-history token totals are implemented for Claude Code and Codex
   through `originlm usage --json` (`history.rs`), including streaming
   duplicate merge, split-aware cache fields, Codex resumed-copy dedupe,
   mixed-model totals, and bounded malformed/oversized input reporting.
   Gemini, Cursor, Grok, Copilot, Antigravity, and Kiro history remain
   unaggregated; day-boundary rollups, a public API/desktop surface, and a
   shared-fixture cross-check against Tokscale/t3code are still missing.
5. Ollama/LM Studio on-disk model discovery now exists through
   `originlm models --json` (Ollama manifests, LM Studio GGUF files, missing and
   malformed roots reported). A custom LM Studio models directory, the
   `lms ls --json` inventory, capability claims, and service/desktop wiring are
   still missing.
6. The detection workflow records each installed CLI's `--version` output and
   asserts provider and executable identity, but it does not yet verify
   OriginLM's parsed version or home/config directory. Those need a metadata
   probe with an explicit grant, which the workflow deliberately does not grant
   to a real account.
7. macOS execution is now recorded by CI on both arm64 and Intel runners.
   There is still no local macOS host qualification of an interactive session.
8. `scan --include-locations` now reports a `checkedProviders` status for every
   known provider (`found` or `not_found`), but it does not yet distinguish
   "not installed" from "installed but blocked" or record every path tried.
   The desktop still has no per-tool not-installed list.
