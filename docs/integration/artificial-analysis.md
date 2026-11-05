# Artificial Analysis Intelligence Index

OriginLM can cache the free Artificial Analysis language-model list and use its
Intelligence Index to order model catalogs. The cache is optional: the desktop
app opens normally without a key, shows `—` for unknown scores, and keeps the
provider-native catalog order available at all times.

Artificial Analysis attribution is required when this data is displayed. The
Models inspector, the score tooltip, and Settings all credit
[Artificial Analysis](https://artificialanalysis.ai/).

## Set the key

Use the free Data API key from
<https://artificialanalysis.ai/api-key-management-redirect>. OriginLM reads it
from the environment of the runtime that owns the cache:

```bash
# macOS / Linux (shared broker or desktop)
export ARTIFICIAL_ANALYSIS_API_KEY="aa-..."
originlm serve --ipc <address> --allow-passive-discovery
originlm-desktop
```

```powershell
# Windows PowerShell
$env:ARTIFICIAL_ANALYSIS_API_KEY = "aa-..."
originlm serve --ipc <address> --allow-passive-discovery
originlm-desktop
```

The desktop app hosts a private runtime when no shared broker is running, so it
inherits the same variable. If a shared broker is already running, set the key
in that broker's environment; the desktop reads only its cached catalog through
the normal client API.

A missing key is never fatal. Settings shows `No API key in this runtime`;
cached scores remain visible and can still be used for ordering. The key is kept
in memory only; it is never written to the cache file, logged, or included in a
diagnostic export.

## Sync cadence and request budget

- Endpoint: `GET https://artificialanalysis.ai/api/v2/language/models/free`
  with `x-api-key`.
- The complete free list is normally two pages (200 models per page), so one
  refresh is about two requests.
- OriginLM refreshes at most every **6 hours** (about 8 requests per day, well
  inside the free tier's ~100 requests per 24 hours). A failed sync retries
  after 15 minutes, and a `429` honours `Retry-After`.
- A window read or a client `models.list` call never calls Artificial Analysis.
- Settings has **Refresh now** for an explicit administrator refresh. It still
  reuses the cache if the runtime is rate-limited.

The durable cache is `intelligence-cache.json` in the runtime state folder
(`<state dir>/intelligence-cache.json`). It stores `fetchedAt`, the index
version (`intelligenceIndexVersion`), the free tier name, and one record per AA
slug. A cache written by a future format is ignored rather than guessed at.

## Ordering in the desktop app

The Models page defaults to **Index** order (highest first). Every model row
shows the score, or `—` when Artificial Analysis has not published one. Unknown
scores sort last and keep their existing relative order. Switch to **Catalog**
to restore the provider-native order; the choice is saved with the other
desktop preferences.

## Opt in for your own apps

The third-party default is **off**. A client can always opt in per request:

```json
{"method":"models.list","params":{"sort":"intelligence"}}
```

`models.page` accepts the same `sort` value and binds it into its cursor. Use
`sort":"catalog"` for the native order. Clients can read the cached data,
including the index version and fetched time, with:

```json
{"method":"intelligence.catalog","params":{}}
```

Settings → **Artificial Analysis Intelligence Index** has **Default order for
your apps**. When enabled, clients that omit a `sort` parameter receive Index
order; the setting is persisted by the runtime and defaults to off. This makes
the choice explicit for apps while leaving the desktop's own Models page
independent.

`intelligence.configure` (administrator only) sets that default. The runtime
also exposes `intelligence.refresh` (administrator only) for an explicit sync.

## Matching and unknown values

AA records are matched to OriginLM models by stable slug, model name, and the
provider-native model ID after normalizing case and separators and removing
well-known release-date/`latest` suffixes. A key shared by two different AA
models is treated as ambiguous and left unknown rather than guessing. Null AA
scores stay null: OriginLM never turns missing data into a zero.
