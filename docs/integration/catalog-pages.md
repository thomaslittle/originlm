# Large catalog pages

`models.page` is an additive public method available through every native transport and the generated TypeScript/Python SDK method families. Existing `models.list` remains unchanged.

```ts
let cursor: string | undefined;
const models = [];
do {
  const page = await ai.models.page({ limit: 200, ...(cursor ? { cursor } : {}) });
  models.push(...page.models);
  cursor = page.nextCursor ?? undefined;
} while (cursor);
```

Production callers should bound the total rows/bytes they retain and check that every page has the same `revision`. Request `snapshot.get` with explicit fields excluding `models` to obtain account/route metadata without a large response. Check its revision against the page revision and again after the last page; do not return a mixed catalog after an account or permission change.

Cursors are scoped to the caller, route filter, position, and projected revision. A changed account, grant projection, or catalog produces `SELECTION_STALE`. Revocation never preserves previously visible rows. Cursors are not credentials and never replace authorization.

Limits are 1–500 rows per request, default 100, and approximately 240 KiB of serialized model data. A single descriptor above the limit returns `FRAME_TOO_LARGE`, not an empty page with an endlessly repeating cursor. Filtering remains route-scoped, and native model IDs are unchanged.

The core page implementation is covered by ordering, scope, stale-catalog, revocation, and size-limit tests. `readCatalogSnapshot` is exported by `originlm`, the public snapshot store uses it automatically when `models.page` is negotiated, and the showroom uses the same helper for initial load, scan refreshes, and connection probes. Older runtimes without `models.page` retain the existing single-snapshot path with the same explicit size guard.
