# T3 Code integration baseline

Upstream: https://github.com/pingdotgg/t3code (MIT). Pinned source: `e9236274a3a6e0c731a7df4092db1e686f039068`. Nightly reference: `7c3b3b3605efa695e9948051b569937fc1482c0f`.

## Local model providers

OriginLM maintains its own explicit local endpoint registry. See [snapshot.json](snapshot.json) for upstream facts.

## Cloud model providers

Coverage and limits are in [the support matrix](../../docs/adapters/SUPPORT-MATRIX.md).

## Installed CLI and tool detection

Only explicitly allowed operations may launch a provider. See [NOTICE.md](NOTICE.md) for adopted behavior and provenance.

## ChatGPT OAuth

Use the pinned review and the host-owned sign-in implementation. Tokens are never returned to consuming applications.

This source export contains no daily planning ledger. Upstream comparison is an explicit maintainer operation; released builds do not fetch upstream sources.
