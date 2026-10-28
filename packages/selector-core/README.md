# Selector core (`@originlm/selector-core`)

> Alpha. Not yet published to npm.

Pure TypeScript selection logic with no DOM, native modules, scans, network, credentials, persistence, or import-time side effects. This is not an execution authorization service.

`createCatalog(snapshot)` indexes route-scoped models. `resolveCandidates(catalog, requirements)` returns visible candidates with reasons and billing labels. `groupCandidates` changes presentation without losing execution identity. `validateOption` preserves native values; `reconcileSelection` reports incompatible saved options and vanished routes. `visibleQuotaPools` does not double-count an identical reported pool.

`eligible` means advisory UI eligibility against the supplied snapshot, never permission to execute. The native service must revalidate current identity, descriptor revision, grants, limits, and billing before a turn. Unknown critical evidence does not satisfy strict filters. Conditional capabilities require native-service qualification.

The `*View` types are explicitly structural UI projections, not a replacement for generated Rust contracts.

Development: Node >=22.16, TypeScript 7. Run `node --experimental-strip-types --test tests/*.test.mjs` then `tsc -p tsconfig.json`. Build emits dependency-free ESM and declarations in `dist/`.

The React hooks ([`@originlm/react`](../react/)), the eight selector layouts ([`@originlm/ui-react`](../ui-react/)) and the [demo app](../../apps/demo/) are separate consumers of this package.
