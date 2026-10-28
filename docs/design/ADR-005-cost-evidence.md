# ADR-005: Cost evidence and local estimates

Accepted 2026-10-28. Owner direction: provider-reported money takes precedence; otherwise estimate from actual token counts and an editable local price table. Never derive money from a quota percentage.

## Decision

Rust owns cost arithmetic and authorization. `costs.read` exposes bounded observations (latest 256 turns during the runtime lifetime), exact decimal USD strings, their basis, and price provenance. Ordinary clients see only their own turns and currently visible catalog identities. Administrators can see all visible turns and change the table through `costs.setPrices`. A missing model, token count, or exact matching price never becomes a zero-dollar inference.

Prices are local runtime configuration in `cost-prices.json`, saved by atomic replacement, with Unix mode 0600. Hosts without a state directory explicitly report in-memory configuration. Corrupt saved configuration disables estimates and reports a repair message instead of silently restoring defaults. Changing prices recalculates estimates; it never rewrites reported charges.

The initial defaults cover exact Claude model IDs from the [official pricing table](https://platform.claude.com/docs/en/about-claude/pricing), reviewed 2026-10-28. They are standard input/output API rates, not subscription invoices. Caching, tool charges, tier discounts, and other adjustments are not modeled. Alias model IDs require an explicit local row.

OpenRouter's documented HTTPS origins supply `usage.cost` in USD-denominated credits. Other compatible endpoints do not inherit that interpretation. Sources: [usage accounting](https://openrouter.ai/docs/cookbook/administration/usage-accounting), [credit denomination](https://openrouter.ai/support), reviewed 2026-10-28. No additional provider request is made to obtain this evidence.

## Limits and presentation

This is observed runtime usage, not account-wide spend. History is bounded and does not survive runtime restart. Partial turns retain their latest observed usage and are marked in progress until final. Only input and output token events qualify for an estimate; session-wide cumulative token counts and context occupancy do not.

Both applications must label estimated values, identify scope, expose unavailable inputs honestly, and provide local price editing in Settings. Example data remains synthetic and separately labeled. The web chart uses per-turn bars so missing observations do not imply a continuous measured spend curve.

## Verification status

Rust service and adapter builds executed on Linux. Decimal arithmetic, admin-only editing, persistence, and corrupt-file regression tests authored; execution is deferred to the final verification phase. UI integration and cross-platform execution remain in progress.
