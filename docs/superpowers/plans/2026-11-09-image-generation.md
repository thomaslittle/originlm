# Image generation implementation plan

Owner-approved specification: ../image-generation-tool.md. Execute inline, no delegates.

- [x] Commit spec before code; apply owner model correction.
- [ ] Add originlm-images: typed requests/results/errors, pluggable async provider, host-controlled canvas/reference/asset scope, exact-size PNG output.
- [ ] Use existing credential-bound Endpoint transport and EndpointConfig keyEnv machinery; OpenAI Images 2.6, Gemini 2.1/Pro, OpenRouter Gemini route; no automatic provider fallback.
- [ ] Expose authenticated loopback MCP generate_image for existing native agent tool paths; trusted result channel for host rendering.
- [ ] Attach desktop tool on explicitly paid turns, render trusted local PNG results inline. Crntly scene mutation is a separate repository follow-up.
- [ ] Add provider wire mocks and tool lifecycle/validation/error tests, plus one-shot live example.
- [ ] ONE final build/test pass; one live generation only if a credential is present. Stop after this pass, record gaps, open one draft PR.

Review focus: reference path traversal/symlinks, malformed/oversized image responses, partial multi-image failure, leaked upstream secrets, tool billing authorization and lifecycle. Bound input/output and decode allocation; stage files and atomically publish only complete results; typed fixed-text errors; no credentials in Debug or tool arguments.
