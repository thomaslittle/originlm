# Native browser demo and HTTP API

Run `npm run dev` to start the native paired HTTP helper and the demo UI on Vite hot module replacement (`http://127.0.0.1:5173/`; the helper listens on a free loopback port that Vite proxies). The command shows real detected data after an explicit pairing; with no paired runtime the UI shows clearly labeled example data.

On Windows:

```powershell
npm run dev
```

On macOS/Linux use the same `npm run dev` command. Open the printed local URL and choose **Show current pairing code**, then pair explicitly. The browser reads the single-use code from the same-origin loopback endpoint; after expiry or redemption, it issues a replacement code automatically from that explicit button. Hosts can call `GET /runtime/pairing-code` / `getPairingCode({ baseUrl })`, and `POST /runtime/pairing-code` / `issuePairingCode({ baseUrl })` when a replacement is needed. After pairing, the browser performs passive discovery and sequential metadata-only checks for providers the host allows: Codex, Claude Code, Grok, Gemini CLI, Antigravity, OpenCode, Ollama, LM Studio, and Cursor's cached metadata. The page shows progress and an outcome for every detected profile. Checks may launch a native metadata client or contact a configured local endpoint, but they do not request model generation. Gemini CLI and Antigravity still need explicit native profile registration with `--native-profiles`; Cursor's cached identity and plan remain stale and cannot verify authentication. The demo grants supported text execution in an isolated temporary workspace; a turn still requires account consent and an explicit **Run live turn** click. A profile appears as authenticated only after a fresh provider result confirms it. Unsupported providers report an explicit error; there is no synthetic fallback.

The launcher grants supported text execution in a new, empty temporary workspace and requests read-only execution. The prompt can be drafted in either mode; Run remains unavailable unless the host grants execution and a route is selected. Granting execution does not start a turn. In the browser, explicitly allow use of the selected account and click **Run live turn** to send a prompt. The temporary workspace is removed when the helper exits.

When the OpenCode CLI is available, the demo starts a temporary loopback-only OpenCode server protected by a random Basic Auth password. The password is passed only through child-process environments; a temporary endpoint file contains the environment variable name, never its value. The server is stopped and its temporary config is removed when the helper exits. OpenCode's `/provider` response supplies models for its connected providers. A nonempty connected-provider list marks the OpenCode profile authenticated as reported by OpenCode; it does not independently validate each upstream credential or prove plan, billing, or quota. No inference request is sent.

The pair code expires after five minutes and allows ten failed attempts. Bearer sessions expire after an hour. The embedding Rust host may call `HttpServer::issue_pairing_code()` to authorize another app; otherwise restart the CLI to issue another code. Tokens stay in the browser client closure, not URLs, local storage, provider credentials, or logs.

## Explicit execution

Add `--allow-codex-execution --workspace ABSOLUTE_DIRECTORY` to authorize native sessions in the named `default` workspace. This does not start inference. A user must choose a model and explicitly start a turn in Playground. The demo requests read-only execution. Optional CLI flags `--allow-writes`, `--allow-native-policy`, `--review-native-requests`, and `--allow-paid-options` only raise the host's maximum permission; a request must also select the corresponding behavior. `native_policy` delegates tool and filesystem/network limits to the installed native client; it is not a Kit sandbox. No metered fallback is automatic.

The HTTP API binds 127.0.0.1 only. Host and Origin are checked, pairing is single-use, API methods require a bearer and protocol handshake, and caller permissions are enforced by the same Rust service used by stdio. Native apps without a browser Origin can use the explicit bearer flow. A page served from another origin is not implicitly trusted.

`POST /v1/pair` takes `{code}`. `POST /v1/rpc` takes the standard JSON-RPC envelope. Read-only REST projections include `/v1/snapshot`, `/v1/connections`, `/v1/models`, and `/v1/usage`. `/v1/events` exposes authenticated SSE with `sessionId` and `after` filters; lost history produces an explicit gap. Stdio supports the equivalent `events.subscribe` / `events.unsubscribe` lifecycle, with bounded output and typed event errors.

## Verification

`node --test scripts/test_live_demo.mjs` exercises real native HTTP and Chromium against an isolated home. `node scripts/check_live_metadata_ui.mjs --allow-metadata` is an explicitly opt-in real Codex metadata browser test; it submits no generation request. Native transport and stream tests are in `crates/originlm-transports/tests`.

The in-memory fixture service remains available to automated tests only. The public browser bootstrap has no fixture fallback.

## Startup warm-up (opt-in)

The launcher passes `--warm-metadata`. With it (HTTP only, plus `--allow-passive-discovery` and at least one `--allow-metadata` provider) the helper runs one passive scan and the metadata checks the host already permitted right after it starts, so results exist when a browser pairs. Different providers are checked concurrently; the profiles of one provider are checked in order, so two checks never race that provider's credential refresh. The helper keeps each observation and marks it fresh for 15 minutes; the demo shows fresh observations immediately ("Already current") and only checks the rest. Without the flag the helper never scans on startup.

## Persisted observations

While serving with a metadata provider grant, the helper keeps its last metadata observations in its state directory (`metadata-cache.json`; `LOCAL_AI_KIT_STATE_DIR`, `%LOCALAPPDATA%\originlm`, or `<home>/.originlm` with `--home`) and reloads them after a restart. They return with their original age, so anything older than the 15-minute freshness window is served as stale and refreshed; entries older than 7 days are dropped. Account identity (labels, identity sources) is never written, the file is written atomically with owner-only permissions on Unix, and `--no-metadata-cache` turns the cache off. A corrupt or foreign file is ignored.

The launcher does not register its own protected OpenCode server by default, because the discovered OpenCode installation is already listed and checked. Set `LAI_DEMO_OPENCODE_ENDPOINT=1` to demonstrate an explicitly registered endpoint (it then appears as a second `OpenCode local` connection).
