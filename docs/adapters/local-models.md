# On-disk local model inventory

`originlm models --json [--home ABSOLUTE_DIRECTORY]` lists models that exist on
disk when the local runtime is not running. It is an explicit read command; it
never starts a server, loads a model, or runs inference.

## Ollama

- Root: `OLLAMA_MODELS`, otherwise `~/.ollama/models`.
- Reads `manifests/` only. A manifest path such as
  `registry.ollama.ai/library/llama3.2/latest` becomes `llama3.2:latest`.
  Non-library namespaces keep their namespace prefix.
- Sums the manifest's layer sizes. A malformed manifest keeps the model id but
  reports `sizeBytes: null` instead of inventing a size.
- Blobs, Modelfiles, and generated output are never read.

## LM Studio

- Roots: `~/.lmstudio/models` and the older `~/.cache/lm-studio/models`.
- Reads `*.gguf` files up to eight directory levels deep. The native id is the
  publisher-relative path without the extension, matching LM Studio's
  `publisher/model/file` convention.
- Reports the file name, byte size, and `format: "gguf"`.
- Does not infer architecture, quantization, context window, vision, or text
  capability from the filename. The `lms ls --json` inventory is not invoked.

## Honest limits

- A custom LM Studio models directory set in the app is not discovered yet.
- Ollama manifests do not report a model's chat/embedding role; the inventory
  does not claim one.
- `models.local` serves the same inventory to the authenticated broker, and the
  desktop Models page shows it. Use `connections.probe` with an explicit
  endpoint for a capability-bearing catalog.
