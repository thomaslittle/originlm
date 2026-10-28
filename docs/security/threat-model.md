# OriginLM threat model

## Trusted boundaries

The installed native helper and embedding application configure policy. Renderers, browser pages, model output, provider messages, and imported catalogs are untrusted. File presence is metadata, not a grant. Native provider clients retain their own authentication and tool policies; this toolkit is not a universal OS sandbox.

## Controls

Stdio is owned by its parent application. Shared broker sockets/pipes and loopback HTTP require explicit pairing; grants are not editable through an arbitrary RPC payload. HTTP checks origin and Host, rejects query-string credentials, bounds messages, and isolates each application identity. A closed app releases only its sessions and results.

Account/profile/route/model/billing identities remain distinct. Account failure must not be hidden by old usage. Backend denial of included usage cannot be overridden by generic paid-option consent or inferred reset times. No automatic API fallback, token extraction, account rotation, overage purchase, or reset-credit consumption exists.

Native commands are structured arguments with owned-process cleanup. Wrappers need verified launch recipes; ambiguous `agent` binaries are not attributed by filename alone. API credentials are bound to explicitly configured endpoints, redirects are rejected, and response bodies/streams are bounded. Error messages are sanitized.

Artifacts are bounded memory-only bytes, expire, and are private to their owner/session. Reading them never accepts a filesystem path. Browser downloads must not execute returned content. Cursor metadata comes from the installed `cursor-agent` CLI. Its dashboard allowance request is off by default; with `--allow-usage cursor` the adapter sends the session token Cursor already stored locally, keeps it in memory for that request only, and never logs, serializes, exports or writes it anywhere else. Without that grant the app shows the cached identity/plan as explicitly stale.

## Known limits

A malicious application running as the same OS user can often inspect or replace that user's processes/files; checksums do not establish publisher authenticity. Native provider startup may honor user-configured extensions, hooks, or tools unless a supported isolated recipe is used. Unsupported provider policies fail closed rather than becoming a fictitious read-only mode.

Live inference and macOS execution remain separate qualification requirements. The current native Google/Grok execution integration is not registered; its experimental recipes are not a supported authentication bypass. No compatibility with every installed client version is promised.

## Verification

Run the negative transport, grants, identity, artifact, native-process, endpoint, and package-integrity suites through `scripts/verify.py`. Use synthetic homes and local test servers. Qualification reports must identify the exact SHA and actual platform, not just an intended target.

## v1 integration controls and remaining boundaries

Grant-descriptor reads validate the opened regular file and enforce a hard 16 KiB
read limit. Unix rejects symlinks, group/world permissions and swapped file identities;
nonblocking opens avoid FIFO hangs. Windows validates regular-file/reparse behavior;
no general ACL ownership audit is claimed. The same-user attacker limitation still
applies. Control IPC authenticates and negotiates before use, bounds response frames,
and discards failed streams rather than reusing an ambiguous reply.

Cost observations contain identifiers/counts/charges, not prompt text, and are filtered
to the requesting application unless it is an administrator. Price edits require that
administrator surface. Corrupt price files disable estimates with a visible error.

The release workflow prepares unsigned previews only. Checksums alone are not trust;
configured minisign/Authenticode pins fail closed. Public signing, notarization and
publication are not implemented guarantees. The web bridge has no page-facing extension
content script and does not enable cross-origin loopback requests. The GPUI mini-view
is not an OS tray. See ADR-005 through ADR-007 and the current review evidence.
