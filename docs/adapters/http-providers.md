# HTTP provider adapters

The native helper now supports explicit Ollama, LM Studio, OpenAI, Anthropic, Gemini, xAI, and compatible API endpoints. Discovery only registers them; `connections.probe` contacts the configured endpoint after a separate grant. Text turns require an execution grant and, for metered routes, explicit billing consent. No discovery call loads a local model or starts generation.

## Configure a local endpoint

```json
{"endpoints":[{"id":"local-ollama","label":"My Ollama","kind":"ollama","baseUrl":"http://127.0.0.1:11434/"}]}
```

Pass `--endpoints ABSOLUTE_JSON` to scan or serve. Detected Ollama and LM Studio installations also use the known literal-loopback defaults when explicitly probed; there is no port or network scanning. Local cloud-routed Ollama models are excluded rather than mislabeled local.

For remote endpoints use `kind` `openai`, `anthropic`, `gemini`, `xai`, or `compatible`; the base includes the API version path (for example `/v1/` or `/v1beta/`). `keyEnv` names a credential variable supplied to the owning helper. The adapter never exports it; the trusted CLI resolves only that named variable after permission checks. Library hosts can directly pass a credential through `endpoint_with_credential`. Inline keys, URL credentials, unsafe redirects, and foreign origins are rejected. Registry parsing does not resolve secrets or perform network requests.

`--allow-metadata PROVIDER` accepts `ollama`, `lmstudio`, `openai_api`, `anthropic_api`, `gemini_api`, `xai_api`, and `compatible_api`. Add the corresponding `--allow-execution` and `--workspace ABSOLUTE_DIRECTORY` only to enable explicit turns. API turns also need host `--allow-paid-options` and per-session `allowPaid: true`. No silent subscription-to-API fallback exists.

## Capability evidence

Ollama `/api/show`, LM Studio's native model inventory, and Anthropic capability fields supply reported capability/context/effort data where available. Generic API model IDs alone do not establish text capability. An explicit `textModels` array may assert text compatibility for *already-listed* models; this is labeled `user_supplied`, not live-provider verification. Unsupported and unknown remain separate. No global Low/Medium/High selector is invented.

## Execution and limits

All HTTP adapters are text-only. The common runtime retains conversation history only in memory and scopes it to the session owner. They do not claim native agent resumption, terminal access, or tool execution. Provider tool calls fail closed. Cancellation drops the in-flight request without retry; a remote service may already have consumed compute. Native final markers are required; a truncated socket cannot become a successful answer.

The transport uses TLS for non-loopback addresses, no proxy or redirect following, origin-bound authentication, bounded JSON/line/stream bytes, and connect/read/overall deadlines. SSE and NDJSON are decoded into common text/usage events. Provider errors are sanitized. Normal tests use local synthetic servers; fixture tests are not live account qualification.

## References

- https://docs.ollama.com/api/tags
- https://docs.ollama.com/api/chat
- https://lmstudio.ai/docs/developer/rest/list
- https://platform.claude.com/docs/en/api/models/list
- https://ai.google.dev/api/models
- https://developers.openai.com/api/reference/resources/models/methods/list
- https://docs.x.ai/developers/model-capabilities/text/generate-text

Reviewed 2026-09-24; native provider/API behavior must be qualified separately for each installed runtime/account. Exact API dialects are covered by `http_catalogs`, `http_frames`, `http_execution`, and `endpoint_config` tests; not all remote dialects have live tests.
