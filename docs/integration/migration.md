# Migrating to OriginLM

This repository is a fresh source distribution of the integrated Local AI Kit work. The original repository, consumers and running services are unchanged. The 14 Rust crates, including `originlm-cli`, are published at `0.1.0-alpha.22`; npm and PyPI remain at `0.1.0-alpha.20` until their publishers run; signed runtime archives are not published yet. Keep existing consumers pinned to their working source until the migration passes packaging and consumer checks.

## Provisional package identities

| Component | OriginLM identity |
| --- | --- |
| TypeScript SDK | `originlm` (including `/node`, `/mcp`, `/mcp/host`, `/mcp/remote` and `/mcp/node`) |
| Browser bridge | `@originlm/web` |
| React hooks | `@originlm/react` |
| Framework independent selection | `@originlm/selector-core` |
| React selectors | `@originlm/ui-react` |
| Native helper npm packages | `originlm-cli-linux-x64`, `originlm-cli-linux-arm64`, `originlm-cli-darwin-x64`, `originlm-cli-darwin-arm64`, `originlm-cli-windows-x64` |
| Python distribution and import | `originlm` / `import originlm` |
| Public Rust facade | `originlm` / `use originlm::LocalAi` |
| Rust implementation crates | `originlm-adapters`, `originlm-chatgpt-auth`, `originlm-cli`, `originlm-contracts`, `originlm-control`, `originlm-core`, `originlm-desktop`, `originlm-discovery`, `originlm-kit`, `originlm-platform`, `originlm-runtime`, `originlm-service`, `originlm-store`, `originlm-transports`, `originlm-update` |
| Private npm workspaces | `originlm-workspace`, `@originlm/demo`, `@originlm/site` |
| CLI binaries | `originlm`, `originlm-control`, `originlm-desktop`, `originlm-native-host`, `originlm-auth-blocker` |

Availability does not reserve these names. Public lookups found no exact-name package matches, but ownership and claimability of the `@originlm` npm scope are unverified. Confirm or obtain that scope before publication. Scope ownership, every auxiliary name and authenticated publication access must be checked before removing publication guards. All Cargo crates retain `publish = false`; npm packages retain `private: true`; Python retains its private distribution classifier.

## Source consumers

Replace `@local-ai-kit/sdk` imports with `originlm`, and other `@local-ai-kit/*` imports with `@originlm/*`. Replace the Rust dependency `local-ai-kit` with `originlm`, crate imports `local_ai_kit` with `originlm`, and Python imports `local_ai_kit` with `originlm`. Use local workspace paths or freshly built tarballs/wheels until publication. Existing `createLocalAI`, `LocalAIClient`, `LocalAIError` and Rust `LocalAi` API symbols remain available.

## Existing runtimes and data

OriginLM uses its own default state paths, binary names, service identifiers and native messaging host identifiers. Do not overwrite an existing runtime or run two services against the same state database. Stop an owned old service before migrating; back up its state and use explicit `--token-file`, socket/address and state-directory options to connect to a compatible existing broker. Re-register browser native messaging hosts and reinstall service descriptors for the new executable paths. Protocol methods and permission boundaries are retained.

Existing `LAI_*` and `LOCAL_AI_KIT_*` environment names remain supported where previously used, to avoid silently breaking host configuration. `ORIGINLM_STATE_DIR` also takes precedence over the legacy state-directory override. This export does not scan for, copy, delete or alter installed user runtime data, credentials or grants.
