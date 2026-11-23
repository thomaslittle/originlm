# Native Google/Grok client status

OriginLM now has an explicit native-profile registry for Gemini CLI, Antigravity ACP, and Grok. Registration is opt-in and records only a provider, exact executable path, profile directory, display label, optional Antigravity auth-method ID, and optional host-declared billing mode. Inline tokens, API keys, arbitrary arguments, command strings, and environment injection are rejected.

The shared ACP engine negotiates native model catalogs/options, provider-owned authentication, streamed text, permission callbacks, cancellation, and session cleanup. The toolkit never exports a subscription token as an API credential. A successful authenticated ACP session proves that the native client accepted its own login for that session; it does **not** prove a plan name, quota, or billing source. Host-declared billing is labeled `user_supplied` evidence.

## Grok

The execution and metadata route is registered only for explicitly configured Grok connections. Passive discovery by itself does not enable execution. On Windows, the launcher requires the actual `grok.exe`, the default `~/.grok` profile, and an isolated leader socket owned by the toolkit. ACP permissions are denied unless the host explicitly enables review and approves the individual request.

Metadata probes (`grok models`, and the ACP initialize that follows a logged-in status, including an explicitly registered native profile) set `GROK_DISABLE_AUTOUPDATER=1`. That is the process-scoped switch Grok documents so a background health check does not start the CLI updater. Text sessions leave the variable unset. The behavior is a Rust reimplementation of T3 Code `packages/provider-grok/src/server/probeEnvironment.ts` at [2a7bd167](https://github.com/pingdotgg/t3code/commit/2a7bd1673c7bdad4304b7771944e73c29ecd3c01). No T3 Code source is copied.

If the first `grok models` result is logged out, the probe runs once more. A definite second result replaces the first. An unknown second result stays logged out. Reimplemented from [a2157ece](https://github.com/pingdotgg/t3code/commit/a2157eced708fd25fab7b616f5a68cb65312eaed). Login state still comes from `grok models`; Grok `auth.json` and the billing endpoint are not read.

Metadata-only qualification on September 25, 2026 used Grok CLI **1.0.41 (4220f3b224a6)** on Windows x64. It reused the existing native login and returned an authenticated ACP session, **Grok 4.7** and a native `reasoning_effort` option. No prompt/model turn was sent. Plan and quota were not part of that qualification (Grok now reports them through `_x.ai/billing`, read by `olm usage`); the test profile's `subscription` billing value remained explicitly user-supplied evidence. This is not paid-generation qualification.

## Gemini CLI

Gemini CLI uses its documented ACP mode and `GEMINI_CLI_HOME`. Browser authentication is disabled for normal probes/execution; an existing selected native login may be reused by the installed client. No API key is copied into the toolkit. The shared ACP route is implemented and fixture-tested, but no compatible Gemini CLI executable was available on the qualification PC, so live account/generation qualification remains untested.

## Antigravity

Antigravity's ACP runtime is either the Antigravity CLI (`agy`, 1.5.0+, launched as `agy acp --gemini-dir <gemini home> --app-data-dir antigravity-acp` with no harness, as T3 Code runs it) or the older `agy_acp_server` plus its sibling `localharness_external`; the desktop IDE's `antigravity.cmd` launcher is intentionally rejected. Discovery finds `agy` on PATH, in the CLI's Windows installer directory `%LOCALAPPDATA%\agy\bin`, and in T3's managed runtime under `~/.t3/tools/antigravity-acp/<platform>/versions/<hash>`. The ACP server runs only in the private `~/.gemini/antigravity-acp` profile: pointed at the CLI's own directory with file storage forced, it rewrites the CLI's saved sign-in and removes it when that copy has expired. The launcher provides the packaged browser blocker, file-backed ACP storage and a profile-local temp directory. Interactive auth URLs are routed to the blocker rather than opened automatically. Windows live qualification on 2026-11-04: two managed profiles initialized, both reported authenticated with 13 native models (Gemini 4 Argon, Gemini 3.8 Flash families), and no Antigravity GUI process launched. On 2026-11-22 `agy` 1.5.14 initialized the same way in a fresh `antigravity-acp` profile.

ACP does not report quota; `olm usage` reads it from Code Assist `retrieveUserQuotaSummary` (daily host first, then production; `fetchAvailableModels` only for an account without a summary) with the login of the profile's own client, in memory for one request, never refreshed or written back:

| Profile | Client | Where the login is read |
| --- | --- | --- |
| `~/.gemini/antigravity-cli` | `agy` CLI | `antigravity-oauth-token` in the profile (the CLI's file store), then the OS credential store under go-keyring service `gemini`, user `antigravity`: Windows Credential Manager target `gemini:antigravity`, macOS Keychain. The Linux Secret Service entry is not read (it needs the system D-Bus library) |
| `~/.gemini/antigravity` | desktop IDE | `Antigravity/User/globalStorage/state.vscdb`, key `antigravityUnifiedStateSync.oauthToken`, under `%APPDATA%` (Windows), `~/Library/Application Support` (macOS) or `$XDG_CONFIG_HOME`, default `~/.config` (Linux) |

These two profiles are usage-only (`ANTIGRAVITY_PROFILE_USAGE_ONLY`). Any other profile reports `ANTIGRAVITY_CREDENTIAL_NOT_FOUND`. An expired login is reported, not renewed: `ANTIGRAVITY_LOGIN_EXPIRED` for the CLI (open `agy`) and `ANTIGRAVITY_IDE_LOGIN_EXPIRED` for the IDE (open Antigravity; the IDE renews its hour-long token only while it runs). Nothing is read from T3 Code's profile directories.

## Safety and billing

Native profile registration does not scan, launch, authenticate, or perform inference. Metadata probes and execution require separate host grants. Native text sessions reject workspace-write/native-policy escalation in this connector; individual ACP permission requests can be mediated when review is explicitly enabled. Billing remains unknown unless the provider reports it or the host deliberately supplies a billing classification. Paid fallback remains separately authorized.

Direct Gemini/xAI API routes remain separate metered integrations. They are never substituted for a failed native subscription route without explicit API configuration and billing permission.
