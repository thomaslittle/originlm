# Desktop Adoption view — specification only

Status: proposed; no view or statistics collector is implemented in this change.

## Purpose and surfaces

Show aggregate distribution activity, public dependency declarations and the owner's explicitly maintained app versions. Downloads are not users, installations, active sessions or endorsements. Never combine registry counts into a misleading total of people.

A future view should offer three sections: daily download history by registry, declared reverse dependencies, and own-app package/version provenance. Each datum carries its source URL, observed-at timestamp, covered UTC date range and stale/unavailable state. Missing data renders “Unavailable”, never zero. Crates are the primary distribution; npm and Python remain separate series.

## Data sources

| Source | Read interface | Interpretation |
| --- | --- | --- |
| crates.io | `/api/v1/crates/originlm/downloads?include=versions` | Daily download counts; recent versions plus older-version aggregate. Keep those buckets distinct. |
| crates.io reverse dependencies | `/api/v1/crates/originlm/reverse_dependencies?page=1&per_page=100` | Paginated public dependency declarations, joined with response version records. Deduplicate crate names; retain dependency kind, version requirement and observation time. |
| npm | `https://api.npmjs.org/downloads/range/last-month/originlm` | Daily public SDK download counts. Display covered dates returned by the service. |
| PyPI | `bigquery-public-data.pypi.file_downloads` | Owner-operated scheduled aggregate query filtered to project originlm and a bounded UTC window; serve only daily aggregates as static JSON. No BigQuery credentials in desktop. |
| Own apps | Explicit owner-maintained manifest or approved repository inspection | Declared and resolved dependency evidence, not inferred end-user deployment. |

Primary references: [crates download controller](https://github.com/rust-lang/crates.io/blob/main/src/controllers/krate/downloads.rs), [reverse dependency controller](https://github.com/rust-lang/crates.io/blob/main/src/controllers/krate/rev_deps.rs), [npm download API](https://github.com/npm/registry/blob/main/docs/download-counts.md), [PyPI public datasets](https://docs.pypi.org/api/bigquery/). Endpoint adapters must tolerate upstream changes. Verify service policies before implementation. Do not scrape reverse-dependency counts for npm/Python or invent an equivalent authoritative API: show unavailable unless an explicit supported dataset is selected later. Public code search may provide separately labelled, incomplete evidence of git/path/vendored consumers.

## Own-app panel

Rows initially cover Twinhaus, crntly-desktop, Remotif, Jawjack and Haiwire. Store repository URL, branch/commit, mechanism (registry/git/path/vendored/helper binary/none), declared requirement, resolved version/revision, release compatibility, lockfile or manifest evidence URL, and latest locally verified build/test result with timestamp and commit. Distinguish an open bump PR from landed source and from a deployed build. “No dependency found” is a valid state; no automatic dependency injection. Do not expose private repository data through the public static feed.

## Fetching, privacy and failure behavior

Prefer an owner-hosted daily static aggregate to fan-out API calls per desktop. Use identifiable service User-Agent, bounded pagination, ETag/Last-Modified where supported, daily caching, backoff on 429/5xx and visible stale timestamps. Manual refresh must still honor DO_NOT_TRACK and configured opt-out. Keep last-good values; failures never rewrite history as zero. Network work stays off the UI thread. No user/device identity, installation heartbeat or application usage event is required. Registry access does not supply unique adopter counts.

## Acceptance criteria for a later implementation

- Fixture tests cover partial service failure, pagination, duplicate reverse deps, missing days, UTC boundaries and stale cache.
- UI labels distinguish declared package use, locally tested source and deployed app version.
- No credentials, raw download logs or private consumer metadata reach public feeds.
- DNT/opt-out tests demonstrate zero statistics requests; aggregate charts retain source/date labels when exported.
- No implementation is included in the observability/attribution PR.
