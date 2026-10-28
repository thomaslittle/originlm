# Troubleshooting and FAQ

## The helper is missing

`RUNTIME_MISSING` means no usable helper was bundled/configured, or startup failed.
For Node, install both the local SDK and matching platform tarball. For Python, use
the platform wheel that contains `_bin/`. During development set `LAI_HELPER_PATH`
to an absolute executable. No import downloads a replacement. Rust can embed the
runtime when allowed and no helper exists; this is reported as `in_process`.

## Packaged Electron cannot start the helper

Set your packager's `asarUnpack` to include
`node_modules/@originlm/cli-*/bin/**`. Electron cannot `spawn` a native binary
through its virtual ASAR filesystem. The SDK selects the physical
`.asar.unpacked` sibling and reports `HELPER_NOT_UNPACKED` if it is missing.
Keep SDK/runtime access in the main process and use scoped IPC for the renderer.
See [Electron’s ASAR limitations](https://www.electronjs.org/docs/latest/tutorial/asar-archives).

## The helper path is rejected

`INVALID_EXECUTABLE` (or Rust's invalid request) means the explicit helper must be
an absolute executable file. Relative paths, directories and `.cmd`/`.bat`/`.ps1`
wrappers are rejected. On Unix, restore executable permission only on a binary you
built/trust. An invalid override never silently selects a different executable.

## A service is running but the app cannot connect

Use the scoped grant descriptor issued to that app and check `LAI_BROKER_ADDRESS`
and `LAI_BROKER_TOKEN_FILE`. Unix descriptors must be owner-only regular files
(typically 0600), not links. Malformed/oversized files are rejected. Do not paste
an administrator token into browser code or logs. Revoked/denied authorization is
an error, not a trigger to start another runtime.

## The service and SDK use different protocol versions

Automatic clients negotiate. An incompatible shared protocol can select the private
helper, or Rust's allowed embedded fallback. `FALLBACK_PROTOCOL_MISMATCH` explains
that choice; `mode` identifies the selected lifetime. A missing/broken fallback
still fails. Update the dependency or service; do not disable grant checks.

## Does OriginLM give me free inference?

No. It uses supported access the user already has and explicitly authorizes. Provider
terms, plan allowances, API charges and native client restrictions still apply.
It does not export subscription credentials as API keys, rotate accounts, silently
select paid fallback or evade limits.

## Why are usage or costs unknown?

Not every provider reports a quota, total, reset time or cost. Evidence can be stale
or unavailable. Costs cover only turns observed by this runtime, with up to 256
observations retained during its lifetime. Estimates need both token counts and an
exact local price row. Configure prices in Settings; missing data is never zero.
Reported OpenRouter USD usage cost takes precedence where that endpoint supplies it.
Subscription prices are not per-token API prices.

## Does everything stay on the machine?

Control state, grants and local price tables stay local; there is no OriginLM cloud
relay. Explicit provider operations can contact that provider and send prompts,
workspace content or tool data according to the granted operation. Local-model
execution can stay local. See [privacy](../security/privacy.md) and the threat model.

## Can a hosted website connect silently?

No. The current extension exposes Native Messaging to its registered extension
page, not arbitrary websites. Loopback pairing is same-origin only. Detection is
not authorization, and an available runtime is not an authenticated badge.

## Where is the tray?

This GPUI build has no OS tray. Ctrl+Shift+M opens an in-app allowance mini-view.
Neither the dashboard nor its mini-view is installed by adding an SDK dependency.

## How do I remove it?

Close the app, stop source servers and remove SDK dependencies with the package
manager. If installed, run the local [service uninstaller](service.md) with the same
path overrides. Ordinary uninstall preserves data/workspaces; purge is explicit.
Do not remove provider clients or their credentials as part of OriginLM uninstall.

## What should a bug report include?

OS/architecture, OriginLM source commit/package version, active mode, error code,
reproduction with synthetic data, and whether the issue is fixture-only or actually
platform-executed. Exclude tokens, pairing codes, account identifiers, prompts and
workspace contents. Report security vulnerabilities through [SECURITY.md](../../SECURITY.md).


## Redirected state and uninstall

The helper honors `LOCAL_AI_KIT_STATE_DIR`, otherwise the platform state directory:
`XDG_STATE_HOME/originlm` (or `~/.local/state/originlm`) on Linux,
`~/Library/Application Support/originlm` on macOS, and
`LOCALAPPDATA/originlm` on Windows. Node/Python child transports preserve these
standard directory variables. Pass provider-specific environment additions explicitly.

The Unix service remover accepts `LAI_DATA_DIR` for an explicitly chosen data
location; PowerShell accepts `-DataDir`. Ordinary removal keeps runtime data.
`--purge`/`-Purge` removes the selected runtime and service state directories, and
refuses a filesystem-root or home-directory target. Browser registrations for
`com.originlm.host` are removed. Prepared native-host payload folders outside
those installation/state roots remain project files; remove a known owned folder
explicitly rather than deleting unrelated workspace contents.
