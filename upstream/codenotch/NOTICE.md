# CodeNotch

[CodeNotch](https://github.com/vinzdg/codenotch) was reviewed as a second
cross-check for macOS Claude/Codex/Gemini/Grok/Kiro session layouts and token
accounting. It is not a dependency; released builds do not download or execute
it.

- Repository: https://github.com/vinzdg/codenotch
- Pinned commit: `6522e9f3ee0fe35d46e428b441e6e80b586eacbb`
- Version noted at pin time: `1.24.0`
- License: MIT
- Copyright: Copyright (c) 2026 Vinz
- License text: [licenses/codenotch-MIT.txt](../../licenses/codenotch-MIT.txt)

No CodeNotch source file is copied. Its `CostIndexer` and session readers were
used only to corroborate field names and dedupe behavior for OriginLM's own
`originlm usage` implementation. OriginLM does not adopt its UI, submission
pipeline, or its Ollama/LM Studio activity relays.

The sibling repository `thomaslittle/codenotch-crossplatform` was not readable
with the available GitHub credentials at review time; no claim is made about a
newer revision there.
