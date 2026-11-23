# Observability and attribution implementation

Owner decisions: Apache-2.0 with NOTICE; headless kit-owned credits metadata and a reusable GPUI renderer; desktop consumes the same renderer; configurable read-only feed client; Adoption view is specification only. No further design questions requested.

## Design

- Preserve third-party licensing and distribute the project NOTICE alongside LICENSE in crates, npm tarballs, helper archives and Python wheels. Add the trademark note to README without altering Apache terms.
- Kit credits expose product, text, mark reference and https://originlm.livestreaming.tools. Credits are on by default and hideable through KitBuilder config. A separate originlm-kit-gpui crate embeds the existing mark and renders a small credits link with caller-supplied muted color.
- originlm-update fetches /v1/releases/latest.json and /v1/catalog.json using a bounded blocking HTTP client on a background worker. The default base URL is https://originlm.livestreaming.tools, overridden by ORIGINLM_FEED_BASE_URL or JSON/Rust config. DO_NOT_TRACK or enabled=false disables all requests. Construction does not fetch. Checks run at most daily by default; redirects, oversized responses and invalid schemas fail closed. No account, conversation, device ID, credential or event telemetry is sent. Optional X-OriginLM-App contains only explicitly supplied app name/version. The desktop identifies itself.
- Release JSON: version, channel, publishedAt, notes. Catalog JSON: schemaVersion, generatedAt, providers. Catalog content is descriptive data, never execution grants or credential configuration. Checks do not download/install binaries; existing signed update activation stays separate.
- The desktop starts the worker outside example/test mode and reads feed config from ORIGINLM_FEED_CONFIG when supplied. No Adoption view implementation.

## Work and verification

1. License/NOTICE manifests and packaging propagation; verify actual package payloads.
2. Headless credits tests, metadata/config and reusable GPUI widget; dogfood in About.
3. Feed tests first: default/override URL, no requests under DNT/opt-out, periodic scheduling, app header, valid feed parsing, size/redirect/error bounds. Implement client and stoppable worker.
4. Add static JSON schemas/examples and feed/privacy docs. Write Adoption spec with registry APIs, reverse-dependency evidence limitations and own-app version provenance.
5. Run targeted local tests and desktop/widget compile checks; open a combined PR. CI is informational and not a gate. No release publication is requested in this stage.

## Local verification evidence (2026-11-08)

- `cargo build -p originlm-desktop -p originlm-kit-gpui --locked`: exit 0.
- `cargo test -p originlm-update -p originlm-kit --locked`: 29 tests passed, including 10 feed tests; optional live credentials tests remain opt-in.
- `python -m unittest discover -s tests/packaging`: 86 tests, 4 existing skips, passed. Original 14-crate publication contract preserved; new GPUI component is source-only for this stage.
- SDK npm dry-run payload includes LICENSE/NOTICE; owned crate license/NOTICE copies and manifest fields verified.
- Scoped rustfmt and staged whitespace/secret-pattern checks passed. CI is not a gate.
