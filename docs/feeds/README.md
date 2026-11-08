# Static update and model catalog feeds

The default base URL is `https://originlm.livestreaming.tools`. Serve two static JSON files, with `Content-Type: application/json`:

| Path relative to base | Shape | Schema |
| --- | --- | --- |
| `/v1/releases/latest.json` | `version`, `channel`, `publishedAt`, `notes` | [release.schema.json](release.schema.json) |
| `/v1/catalog.json` | `schemaVersion`, `generatedAt`, `providers` | [catalog.schema.json](catalog.schema.json) |

The example files are illustrative, not production feeds. Any static host can serve them. Publish files atomically and use cache headers appropriate to daily checks. Schema version 1 is supported. Provider IDs must be unique; model IDs must be unique within a provider. At most 16,384 models total are accepted. SemVer versions and RFC3339 timestamps are validated by the Rust client. Text limits in the client count UTF-8 bytes; schemas count characters, so producers should stay within the byte limits too. Names must contain a non-whitespace character. Catalog entries are descriptive metadata: they grant no execution permissions, install no providers, and configure no credentials.

## Client configuration and privacy

`originlm-update::feeds::FeedConfig` defaults to enabled, the base URL above, and `intervalSeconds: 86400`. Client construction is inert; `FeedClient::poll_at` performs due requests. `FeedWorker::start` performs the initial check on a background thread and repeats periodically. `LatestRelease::is_newer_than(current_version)` compares SemVer versions (including prereleases) so hosts can present an available update. Hosts choose which channel to display. There is no automatic binary download or installation; signed update activation remains separate. Dropping the worker cancels later polls; an already-running bounded poll may finish.

`ORIGINLM_FEED_BASE_URL` overrides `FeedConfig.base_url`, which defaults to the hosted URL. HTTP and HTTPS bases are supported, including a path prefix; credentials, query strings and fragments are rejected. Requests do not follow redirects. Each request has a ten-second timeout (five-second connection timeout); release and catalog bodies are bounded to 256 KiB and 4 MiB. Failed requests wait until the next configured interval (60 seconds to seven days). Last valid data remains in memory, alongside errors indicating a potentially stale snapshot. No disk cache is created.

Set `DO_NOT_TRACK=1` or `enabled: false` to prevent **all** feed requests. Nonempty DNT values other than `0` and `false` disable requests. No account, conversation, credential, device identifier or telemetry event is sent. A normal HTTP server can still observe the request's network address, time and User-Agent (`originlm/<version>`). Hosts should choose their own access-log retention policy.

App identification is optional: set `app: {"name":"my-app","version":"1.2.3"}` to send `X-OriginLM-App: my-app/1.2.3`. Names use ASCII letters, digits, dots, underscores or hyphens; versions use SemVer. OriginLM desktop supplies `originlm-desktop/<version>`. Other embedding apps must explicitly supply their own identity; this change does not modify them.

Desktop accepts a JSON config file via `ORIGINLM_FEED_CONFIG`:

```json
{"enabled": false, "baseUrl": "https://originlm.livestreaming.tools", "intervalSeconds": 86400}
```

An absent variable uses defaults. An explicitly configured missing, malformed or oversized file disables the desktop worker. Environment/config changes apply at next startup. Example mode and tests never start the desktop worker. The owner provisions the static host separately; this code does not deploy it.

## Credits for embedding applications

Headless `originlm-kit` owns `CreditsConfig` and the metadata returned by `Kit::credits()`: “Runs on OriginLM”, product name, mark reference and product URL. Credits are enabled by default; `KitBuilder::show_attribution(false)` or `CreditsConfig { show_attribution: false }` hides them. This is a product preference, not an additional license term.

`originlm-kit-gpui::credits(kit.credits(), muted_color)` returns an optional reusable About/credits link with the bundled mark. Place it in your app's credits surface, using your theme's muted text color. It performs no network requests; activating the link opens the product website. OriginLM desktop uses this exact renderer as its first consumer. Set `show_attribution: false` in the desktop's existing preferences JSON to hide it. The new GPUI crate is source-only (`publish = false`) in this PR; path/git consumers can reuse it. A future coordinated release must qualify its GPUI/toolchain requirements before registry publication. Headless consumers acquire no GPUI dependency.
