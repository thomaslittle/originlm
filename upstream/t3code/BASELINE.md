# T3 Code integration baseline

Upstream: https://github.com/pingdotgg/t3code (MIT). Pinned source: `8b44f42b14c8e273156474a924ccf29680dd7857`. Nightly reference: `8b44f42b14c8e273156474a924ccf29680dd7857` (`v0.1.8-nightly.20261104.4165`).

## Local model providers

OriginLM maintains its own explicit local endpoint registry. See [snapshot.json](snapshot.json) for upstream facts.

## Cloud model providers

Coverage and limits are in [the support matrix](../../docs/adapters/SUPPORT-MATRIX.md).

## Installed CLI and tool detection

Only explicitly allowed operations may launch a provider. See [NOTICE.md](NOTICE.md) for adopted behavior and provenance.

## ChatGPT OAuth

Use the pinned review and the host-owned sign-in implementation. Tokens are never returned to consuming applications.

This source export contains no daily planning ledger. Upstream comparison is an explicit maintainer operation; released builds do not fetch upstream sources.
