# Intelligence Index ordering (RankLLMs)

OriginLM can cache Intelligence Index scores from the free
[RankLLMs leaderboard](https://rankllms.com/) and use them to order model
catalogs. RankLLMs needs **no API key**, so this works for ordinary OriginLM
users without an Artificial Analysis organization account.

The cache is optional: the desktop app opens normally, shows `—` for unknown
scores, and keeps the provider-native catalog order available at all times.

## Source and attribution

- Endpoint: `GET https://api.rankllms.com/api/v1/leaderboard?limit=500`.
- Fields cached: `intelligence_index`, `coding_index`, `agentic_index`,
  OpenRouter id, provider, and input/output prices where RankLLMs publishes
  them.
- Index scores originate from Artificial Analysis where applicable.
- When this data is displayed, credit both
  [RankLLMs](https://rankllms.com/) and the
  [Artificial Analysis](https://artificialanalysis.ai/) source. The Models
  inspector, score tooltip, and Settings do this.

No `ARTIFICIAL_ANALYSIS_API_KEY` is read or required. OriginLM does not call an
Artificial Analysis endpoint directly; if you already have an OpenRouter key,
that remains an independent OriginLM feature and is not part of this cache.

## Sync cadence and request budget

- One refresh is a single bounded leaderboard request.
- The runtime refreshes at most every **6 hours** (about four requests per
  day). A failed sync retries after 15 minutes, and `429` honours
  `Retry-After`.
- A window read or a client `models.list` call never contacts RankLLMs.
- Settings has **Refresh now** for an explicit administrator refresh.

The durable cache is `intelligence-cache.json` in the runtime state folder
(`<state dir>/intelligence-cache.json`). It stores `fetchedAt`, the source, the
current app-default preference, and one record per RankLLMs slug. A cache written
by an older source or a future format is ignored rather than guessed at.

## Ordering in the desktop app

The Models page defaults to **Index** order (highest first). Every model row
shows the score, or `—` when RankLLMs has not published one. Unknown scores sort
last and keep their existing relative order. Switch to **Catalog** to restore
the provider-native order; the choice is saved with the other desktop
preferences.

## Opt in for your own apps

The third-party default is **off**. A client can always opt in per request:

```json
{"method":"models.list","params":{"sort":"intelligence"}}
```

`models.page` accepts the same `sort` value and binds it into its cursor. Use
`{"sort":"catalog"}` for the native order. Clients can read the cached data,
including the source, fetched time, and `defaultSortForApps`, with:

```json
{"method":"intelligence.catalog","params":{}}
```

Settings → **Intelligence Index (RankLLMs)** has **Default order for your apps**.
When enabled, clients that omit a `sort` parameter receive Index order; the
setting is persisted by the runtime and defaults to off. This makes the choice
explicit for apps while leaving the desktop's own Models page independent.

`intelligence.configure` (administrator only) sets that default. The runtime
also exposes `intelligence.refresh` (administrator only) for an explicit sync.

## Matching and unknown values

RankLLMs records are matched to OriginLM models by stable slug, model name,
OpenRouter id, and the provider-native model ID after normalizing case and
separators and removing well-known release-date/`latest` suffixes. A key shared
by two different models is treated as ambiguous and left unknown rather than
guessing. Null scores stay null: OriginLM never turns missing data into a zero.
