# Provider brand marks

Monochrome SVG marks used by the demo to identify providers. They are tinted in the UI with CSS `mask-image` (only the alpha channel matters), so they carry `fill="currentColor"` / `stroke="currentColor"` and no fixed colours. The lookup lives in `../../lib/provider-icons.ts`.

Brand marks are trademarks of their respective owners; used only to identify the provider. This repository's MIT license does not grant trademark rights.

## Modifications (all files)

Only mechanical normalisation: removed the `height="1em"`, `width="1em"` and `style="flex:none;line-height:1"` attributes so the file sizes from its container; the 24x24 `viewBox` was already present upstream and is unchanged. Path data, `<title>` and fill attributes are untouched. Files were not passed through svgo. `local.svg` additionally dropped the upstream `class`, `width="24"`, `height="24"` and the `@license` comment.

## Brand marks: `@lobehub/icons-static-svg` 1.95.1

- Source: https://github.com/lobehub/lobe-icons (npm package `@lobehub/icons-static-svg@1.95.1`, `icons/<name>.svg`, monochrome variants, not `-color` / `-text`).
- License: MIT (SPDX: MIT), copyright holder Copyright (c) 2023 LobeHub. Text in `licenses/provider-icons-lobehub-MIT.txt` (the npm tarball ships no LICENSE file; the text was fetched from the repository's root `LICENSE` and the package.json declares `"license": "MIT"`).
- Trademarks in the marks belong to their owners (OpenAI, Anthropic, xAI, Google, Ollama, LM Studio, OpenCode); the MIT license covers only the SVG artwork files as distributed by LobeHub.

| File here | Upstream path | Provider ids that use it |
| --- | --- | --- |
| `openai.svg` | `icons/openai.svg` | `openai` |
| `codex.svg` | `icons/codex.svg` | `codex` |
| `claude.svg` | `icons/claude.svg` | `claude` |
| `claude-code.svg` | `icons/claudecode.svg` | `claude_code` |
| `anthropic.svg` | `icons/anthropic.svg` | `anthropic` |
| `grok.svg` | `icons/grok.svg` | `grok` |
| `xai.svg` | `icons/xai.svg` | `xai` |
| `antigravity.svg` | `icons/antigravity.svg` | `antigravity` |
| `gemini.svg` | `icons/gemini.svg` | `gemini`, `gemini_cli` |
| `opencode.svg` | `icons/opencode.svg` | `opencode` |
| `lmstudio.svg` | `icons/lmstudio.svg` | `lmstudio`, `lm_studio` |
| `ollama.svg` | `icons/ollama.svg` | `ollama` |

## Neutral glyph: `lucide-static` 1.48.0

- Source: https://github.com/lucide-icons/lucide (npm package `lucide-static@1.48.0`, `icons/cpu.svg`).
- License: ISC (SPDX: ISC), copyright holder Copyright (c) 2026 Lucide Icons and Contributors. Text in `licenses/provider-icons-lucide-ISC.txt`. The `cpu` icon is not in the list of Feather-derived icons, so the additional Feather MIT notice in that file does not apply to it (it is retained anyway with the full upstream text).
- This is a generic chip glyph, not a brand mark.

| File here | Upstream path | Provider ids that use it |
| --- | --- | --- |
| `local.svg` | `icons/cpu.svg` | `local` |
