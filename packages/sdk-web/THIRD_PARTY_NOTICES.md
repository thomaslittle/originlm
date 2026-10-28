# Third-party notices and reuse ledger

## Scope

This file records third-party references and notices relevant to this project. It is not a complete inventory of every dependency; release artifacts include a generated dependency inventory and license materials as described below.

The ChatGPT managed-auth integration is original Rust integration code in this repository. Its design used the OpenAI documentation linked from [the ChatGPT managed-auth guide](docs/integration/chatgpt-managed-auth.md). T3 Code's [pull request 14290](https://github.com/pingdotgg/t3code/pull/14290) was consulted as a reference only; no code from that pull request was copied.

Reference does not imply affiliation, endorsement, inherited security guarantees, or blanket rights to use vendor services. Provider names are used for identification. Logos and trademarks are not granted by this repository's license. Provider authentication, subscription access, and endpoint use remain separate integration-policy questions.

## Alpha18 authentication dependencies

The managed-auth verifier uses `jsonwebtoken` 10.3.0 (MIT) with `aws-lc-rs`
1.18.2 and `aws-lc-sys` 0.46.0. The latter retain their combined upstream
AWS-LC/BoringSSL/OpenSSL and bundled third-party notices in their complete
`LICENSE` files, copied into the release bundle's dependency license directory.
Windows enables the upstream `prebuilt-nasm` fallback; this is the ordinary
AWS-LC backend and makes no FIPS claim. Resolved dependency versions and archive checksums are recorded in `Cargo.lock`.
Release packaging must preserve the complete dependency licenses.

## Other reference projects

| Project | Source | Intended reference |
| --- | --- | --- |
| T3 Code | https://github.com/pingdotgg/t3code | Native client integration, provider instances, dynamic model options, picker patterns. |
| shadcn-ui/chatbot-template | https://github.com/shadcn-ui/chatbot-template | Chat interaction and model-selector patterns adapted for the demo Playground (see reuse ledger below). |
| CodeNotch cross-platform | https://github.com/thomaslittle/codenotch-crossplatform | Rust/platform discovery and usage parsing. |
| CodeNotch | https://github.com/vinzdg/codenotch | Broader provider/platform discovery. |
| Prmptr | https://github.com/thomaslittle/prmptr | Vertical provider rail and grouped model selection. |

The reuse ledger below and `upstream/` record inspected source revisions. For future reuse, verify the license at the exact source revision and preserve required copyright and license text.

## Tokscale discovery knowledge

| Field | Value |
| --- | --- |
| Project | Tokscale |
| Source | https://github.com/junhoyeo/tokscale |
| Pinned commit | `1d9a9395418efc6952944b794097935d7d6fa1e8` |
| Release noted at pin time | `v4.17.0` (`dcf8d3656bbcecf05d112c6f35f9be328e4f66b7`) |
| Upstream paths reviewed | `crates/tokscale-core/src/clients.rs`, `paths.rs`, `scanner.rs`, `cc_mirror.rs`, `sessions/` |
| License | MIT |
| Copyright | Copyright (c) 2025 Junho Yeo |
| License text | [licenses/tokscale-MIT.txt](licenses/tokscale-MIT.txt) |
| Destination | `crates/lai-discovery` path rules; `upstream/tokscale/snapshot.json` factual extract |
| What was reused | Discovery locations, environment-variable names, and Windows/XDG path behavior. Reimplemented. No Tokscale source file is copied into the build. |
| What was not reused | Pricing, token aggregation, dashboards, submission infrastructure, leaderboards, and TUI code. |
| Modifications | Normalized into Local AI Kit profile roots and `EvidenceSource` provenance. Tokscale is not authoritative when an official client or a local qualification disagrees. |
| Endorsement | None. Tokscale does not endorse Local AI Kit. |

See [upstream/tokscale/NOTICE.md](upstream/tokscale/NOTICE.md).

## T3 Code provider sync (2026-10-27)

| Field | Value |
| --- | --- |
| Source | https://github.com/pingdotgg/t3code at `e9236274a3a6e0c731a7df4092db1e686f039068` |
| Nightly tag at pin time | `v0.1.4-nightly.20261027.3770` (`7c3b3b3605efa695e9948051b569937fc1482c0f`) |
| Stable tag at pin time | `v0.1.3` (`d2c440312e5f39d59827ed466838a623516454d6`) |
| Referenced paths | `packages/provider-fx/src/server/driver.ts`, `packages/provider-fx/src/server/status.ts`, `packages/provider-acp/src/server/runtimeModel.ts`, `packages/effect-acp/src/client.ts` |
| Upstream commits | [6df241d23](https://github.com/pingdotgg/t3code/commit/6df241d23a8b82ff786fe3253dfb08645933db26) (fx provider; inventory only), [323d9611](https://github.com/pingdotgg/t3code/commit/323d9611ff49521357fdc205f667614602ec46b6) (ACP session notices) |
| Copied files | None. Behavior was reimplemented in Rust. |
| License | MIT |
| Copyright | Copyright (c) 2026 T3 Tools Inc. The license text is reproduced in the Muse section below and applies to this reuse as well. |
| What was reused | The `fx` command and `~/.fx` as passive inventory, and ACP `notice` updates as warnings that are not assistant text. |
| What was not reused | fx execution and its medium effort default, shortened model labels ([56695ae3](https://github.com/pingdotgg/t3code/commit/56695ae3ec2aad931030af1e7e43852f741b9eef)), the seven-day new-model badge ([0790e3891](https://github.com/pingdotgg/t3code/commit/0790e3891b7470d5a33292dedc4d4ae62d02ff29)), the Cursor SDK 1.0.38 bump ([850d1e7a](https://github.com/pingdotgg/t3code/commit/850d1e7a6c5ffd894c48e9182169cf8e25097db5)), and T3 release CI. |

## T3 Code provider sync (2026-10-26)

| Field | Value |
| --- | --- |
| Source | https://github.com/pingdotgg/t3code at `f0dd86f16482595c6cfffe0ac64dc12bffb634f9` |
| Nightly tag at pin time | `v0.1.3-nightly.20261026.3717` (`1316d48b0c68151afd8b47eb0c11a114cf0b4daa`) |
| Stable tag at pin time | `v0.1.2` (`044f07257c9684ae4543bd1e1ebf9c394e9e3830`) |
| Referenced paths | `packages/provider-grok/src/server/status.ts`, `apps/server/src/provider/CodexInstallation.ts`, `apps/server/src/provider/CodexProvider.ts`, `apps/server/src/provider/codexUsageLimits.ts`, `apps/server/src/provider/claudeUsageLimits.ts` |
| Upstream commits | [a2157ece](https://github.com/pingdotgg/t3code/commit/a2157eced708fd25fab7b616f5a68cb65312eaed) (Grok login recheck), [348888a7](https://github.com/pingdotgg/t3code/commit/348888a767433464dd47e723ec6509dc5a8282db) (Codex code-mode host), [33f1fe5e](https://github.com/pingdotgg/t3code/commit/33f1fe5ebe53fdd8d18b6a5d63dfd29c336ba5a9) (ChatGPT plan labels), [3b844dab](https://github.com/pingdotgg/t3code/commit/3b844dab5ced7277ce4a2cf0e5e11d0f04e62241) (configured service tier), [4c18faf6](https://github.com/pingdotgg/t3code/commit/4c18faf62cd1685b0243dcacc1a94e333c2b93ff) (Codex individual credits), [fd8dd516](https://github.com/pingdotgg/t3code/commit/fd8dd51654f855dbf4e23c457701f5088c59b689) (Claude monthly spend) |
| Copied files | None. Behavior was reimplemented in Rust. |
| License | MIT |
| Copyright | Copyright (c) 2026 T3 Tools Inc. The license text is reproduced in the Muse section below and applies to this reuse as well. |
| What was reused | The six behaviors named in the upstream commits above. |
| What was not reused | Grok billing that reads `auth.json` ([79f2bae6](https://github.com/pingdotgg/t3code/commit/79f2bae6)); spawning a Muse SDK host to learn `museHome` ([c8933536](https://github.com/pingdotgg/t3code/commit/c8933536)); a forced `gpt-6-astra` reasoning effort; ChatGPT profile-key hashing; shell-profile PATH hydration; T3 release CI. Earlier non-adoptions remain: ACP Registry downloads, Codex downloads, managed Antigravity installs, and the Cursor SDK. |

## T3 Code provider sync (2026-10-16)

| Field | Value |
| --- | --- |
| Source | https://github.com/pingdotgg/t3code at `cfafb4bb54d244e4612f5681aad3832ea64cb0fb` |
| Nightly tag at pin time | `v0.1.1-nightly.20261016.3203` (`e3b35d1b2440b104f0bc2e357377c348121234dd`) |
| Stable tag at pin time | `v0.1.0` (`81493efb71e8e8244f95d3e9f71d8369654ab151`) |
| Referenced paths | `packages/provider-grok/src/server/probeEnvironment.ts`, `apps/server/src/provider/CodexProvider.ts` |
| Upstream commits | [2a7bd167](https://github.com/pingdotgg/t3code/commit/2a7bd1673c7bdad4304b7771944e73c29ecd3c01) (Grok probe environment), [cfafb4bb](https://github.com/pingdotgg/t3code/commit/cfafb4bb54d244e4612f5681aad3832ea64cb0fb) (Codex probe timeout, already matched) |
| Copied files | None. Behavior was reimplemented. |
| License | MIT |
| Copyright | Copyright (c) 2026 T3 Tools Inc. The license text is reproduced in the Muse section below and applies to this reuse as well. |
| What was reused | Metadata probes set `GROK_DISABLE_AUTOUPDATER=1`. Text sessions do not. |
| What was not reused | ACP Registry downloads, including the Windows ARM `windows-x86_64` fallback ([76cb251d](https://github.com/pingdotgg/t3code/commit/76cb251da129579d13be333ffe89a8767f7a257a)); Codex release downloads ([364f5084](https://github.com/pingdotgg/t3code/commit/364f5084d7418e1605220b09de5aaebd46305a73)); managed Antigravity installs ([895354e9](https://github.com/pingdotgg/t3code/commit/895354e9fbf5be90cda4fdcd710dac96496181c4)); the Cursor SDK and Auto-review ([2d943723](https://github.com/pingdotgg/t3code/commit/2d943723769da0b675b56168d764d1ad96fc8d28)). |

## T3 Code provider sync (2026-10-15)

| Field | Value |
| --- | --- |
| Source | https://github.com/pingdotgg/t3code at `42da166d131017335a1158d0b379dd68b8985796` |
| Nightly tag at pin time | `v0.0.46-nightly.20261015.3158` (`e6bbfdb9ffe8334bed1c9b992f828f04d30db7ea`) |
| Stable tag at pin time | `v0.0.45` (`6c8fed35dded9ff71c5b46807125457acbb76be6`) |
| Referenced paths | `packages/provider-pi/src/server/usage.ts`, `packages/provider-opencode/src/server/driver.ts`, `apps/server/src/provider/CodexChatGptAuth.ts` |
| Copied files | None for this pin. Behavior was reimplemented. |
| License | MIT |
| Copyright | Copyright (c) 2026 T3 Tools Inc. The license text is reproduced in the Muse section below and applies to this reuse as well. |
| What was reused | Pi's profile and session location, OpenCode's `~/.opencode/bin` native install path, and ChatGPT token-refresh rules (`earliest_refresh_at`, ES256 identity tokens, consent retry when plan sharing was omitted). |
| What was not reused | ACP Registry downloads, multi-account ChatGPT handoff, Codex shadow homes, the Cursor SDK, T3 Connect, and any Ollama, LM Studio, or llama.cpp integration (those providers are absent at this commit). |

See [UPSTREAM_T3CODE.md](../../upstream/t3code/BASELINE.md) and [upstream/t3code/NOTICE.md](upstream/t3code/NOTICE.md).

## Muse Code adapter (T3 Code port)

The Muse Code adapter (`crates/lai-adapters/src/muse/`) ports the behaviour of T3 Code's Muse provider to Rust. No TypeScript was copied; the protocol subset, approval-choice mapping, launcher handling, item presentation (web search hits, workflow children, subagents) and documented limits follow it. Recorded protocol transcripts were copied unchanged.

| Field | Value |
| --- | --- |
| Source | https://github.com/pingdotgg/t3code at `e9221e44dadd78126c83d2eeed0b51c25dcf0fb4` |
| Referenced paths | `packages/provider-muse/src/server/{adapter,sdk,protocol,modelCatalog,status,itemPresentation,textGeneration}.ts`, `docs/user/providers-muse.md` |
| Copied files | `apps/server/src/orchestration-v2/testkit/fixtures/{simple,multi_turn,turn_interrupt,muse_permission,muse_workflow,subagent_v2,muse_subagent_approval,muse_subagent_interrupt}/muse_transcript.ndjson` → `crates/lai-adapters/tests/fixtures/muse/t3/*.ndjson` (renamed, contents unchanged) |
| License | MIT |
| Copyright | Copyright (c) 2026 T3 Tools Inc. |

T3 Code's MIT license text, which must accompany the copied transcripts:

```text
MIT License

Copyright (c) 2026 T3 Tools Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Muse Code itself is not redistributed. Its MSP schema was read from the installed binary (`muse schema generate-json-schema`, 1.4.7-R6101.1) to check wire shapes; the `echo_*` transcripts were recorded locally from that binary's free echo provider. No new Cargo dependency was added.

## Reuse ledger

### shadcn-ui/chatbot-template (demo Playground chat interface)

- Source: https://github.com/shadcn-ui/chatbot-template (default branch, inspected 2026-09-28: `components/chat.tsx`, `chat-message.tsx`, `prompt-form.tsx`, `model-select.tsx`, `suggestions.tsx`, `components.json`, `app/globals.css`).
- License: MIT (SPDX: MIT), copyright holder `shadcn` (Copyright (c) 2026 shadcn). Full text is retained in `licenses/shadcn-chatbot-template-MIT.txt`.
- Destination: `apps/demo/src/pages/playground.tsx`. The chat primitives the template uses (`bubble`, `message`, `message-scroller`, `item`, plus `empty`, `input-group`, `alert`, `spinner`) are installed unmodified from the official shadcn registry with the shadcn CLI into `apps/demo/src/components/ui/`, not copied from the template repository.
- Adapted structure (reimplemented, not copied): the empty state with suggestion chips, the `MessageScroller` thread of `Message` + `Bubble` pairs with a "Thinking" shimmer, the destructive `Alert` for failures, and a `PromptForm` whose `InputGroup` carries the model control beside Send/Stop.
- Modifications: the model control is the provider-rail popover (`apps/demo/src/components/model-picker.tsx`) instead of a flat gateway `Select`; assistant text renders through `react-markdown` + `remark-gfm` inside Tailwind Typography styles instead of the template's `typeset.css`; every turn keeps the toolkit's explicit route selection, consent checkbox, and Run-click gate, and synthetic fixture data stays labeled. No API route, AI SDK, gateway, or tool code from the template is used.

### shadcn/ui components (demo UI primitives)

- Source: https://ui.shadcn.com (npm package `shadcn` 4.21.0, style `base-mira`, Base UI primitives, Phosphor icon library; see `apps/demo/components.json`).
- License: MIT (SPDX: MIT), copyright holder shadcn. The components are code generated into `apps/demo/src/components/ui/` by `npx shadcn@latest add`, which is the project's documented install method.
- Modifications: `Badge` gains `success`/`warning`/`info`/`brand` variants and `Alert` gains `warning`/`info` variants for evidence status; `Sidebar` no longer writes a state cookie; `Dialog` overlay opacity; `use-mobile` reads the media query through `useSyncExternalStore`. Everything else is as generated.

### Design tokens and typography (demo theme)

- Theme tokens (neutral surfaces, crimson accent hue 22.7, elevation ramp, radius) were designed by the project author for this demo and are original work.
- Font: Outfit Variable via npm package `@fontsource-variable/outfit` 5.3.0, license SIL OFL 1.1, Copyright 2021 The Outfit Project Authors. License text retained in `licenses/outfit-font-OFL-1.1.txt`. The woff2 files are bundled by Vite and served from the same origin.
- Icons: Phosphor Icons (`@phosphor-icons/react`, MIT, Copyright (c) 2020 Phosphor Icons), used as an npm dependency; license text in `licenses/phosphor-icons-MIT.txt`.

### Provider brand marks (demo provider icons)

- Source: https://github.com/lobehub/lobe-icons, npm package `@lobehub/icons-static-svg@1.95.1` (`icons/openai.svg`, `codex.svg`, `claude.svg`, `claudecode.svg`, `anthropic.svg`, `grok.svg`, `xai.svg`, `antigravity.svg`, `gemini.svg`, `opencode.svg`, `lmstudio.svg`, `ollama.svg`; monochrome variants).
- License: MIT (SPDX: MIT), copyright holder Copyright (c) 2023 LobeHub. Full license text is retained in `licenses/provider-icons-lobehub-MIT.txt` (fetched from the repository's root `LICENSE`; the npm tarball ships none).
- Destination: `apps/demo/src/assets/providers/` (`claude-code.svg` is upstream `claudecode.svg`), with per-file provenance in `apps/demo/src/assets/providers/README.md`.
- Modifications: removed the `height="1em"`, `width="1em"` and inline `style` attributes; the 24x24 `viewBox` and all path data are unchanged.
- The marks are trademarks of their respective owners and are used only to identify the provider. This repository's MIT license does not grant trademark rights.

### Neutral "local runtime" glyph (demo provider icons)

- Source: https://github.com/lucide-icons/lucide, npm package `lucide-static@1.48.0` (`icons/cpu.svg`).
- License: ISC (SPDX: ISC), copyright holder Copyright (c) 2026 Lucide Icons and Contributors. Full license text is retained in `licenses/provider-icons-lucide-ISC.txt`.
- Destination: `apps/demo/src/assets/providers/local.svg`.
- Modifications: removed the `@license` comment, `class`, `width` and `height` attributes; `viewBox` and geometry unchanged.

## New Cargo dependencies in the ChatGPT managed-auth change

The following registry packages and versions were added to `Cargo.lock` in the managed-auth change. The SPDX expressions below are taken from each package's Cargo manifest metadata.

| Package | Version | SPDX license expression |
| --- | --- | --- |
| `byteorder` | `1.5.0` | `Unlicense OR MIT` |
| `deranged` | `0.5.8` | `MIT OR Apache-2.0` |
| `fs4` | `0.13.1` | `MIT OR Apache-2.0` |
| `jsonwebtoken` | `9.3.1` | `MIT` |
| `keyring` | `3.6.3` | `MIT OR Apache-2.0` |
| `num-conv` | `0.2.2` | `MIT OR Apache-2.0` |
| `pem` | `3.0.6` | `MIT` |
| `powerfmt` | `0.2.0` | `MIT OR Apache-2.0` |
| `simple_asn1` | `0.6.4` | `ISC` |
| `time` | `0.3.55` | `MIT OR Apache-2.0` |
| `time-core` | `0.1.9` | `MIT OR Apache-2.0` |
| `time-macros` | `0.2.32` | `MIT OR Apache-2.0` |
| `uuid` | `1.26.1` | `Apache-2.0 OR MIT` |
| `windows-sys` | `0.59.0` | `MIT OR Apache-2.0` |

## Release artifact license materials

Standalone Windows helper releases include a generated Cargo dependency license appendix with the helper's `THIRD_PARTY_NOTICES.md`, and a CycloneDX SBOM. The full SDK bundle also carries per-crate license files under `licenses/`, alongside its SBOM. These materials inventory the dependencies represented by that build; they do not certify legal compliance or cover unrelated code provenance.

## Required ledger for future reuse

Record the source URL, exact commit/blob, original path, copied or adapted symbols, destination path, license/SPDX identifier, copyright holder, retained notices, modifications, and applicable asset/model/dependency conditions. Add required license text under the release's dependency license materials when applicable. Do not mark the ledger complete merely because the root repository is MIT-licensed.
