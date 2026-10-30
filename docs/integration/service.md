# Optional background service

The service shares one runtime across granted applications. SDKs can instead own a
private helper. The service installer installs the helper only, not the dashboard.
The packages are published; there is no hosted installer.

## Prepare and install

Build installer-compatible archives using the preparation workflow described in
[release preparation](../release/PUBLISHING.md). The archive and `SHA256SUMS` must be
served from a release base you control. From a reviewed source checkout:

```sh
# Set this to the actual prepared artifact endpoint; no default host is assumed.
LAI_RELEASE_BASE="$ORIGINLM_RELEASE_BASE" sh scripts/service/install.sh
```

```powershell
# Set $OriginLMReleaseBase to the actual prepared artifact endpoint first.
& ./scripts/service/install.ps1 -ReleaseBase $OriginLMReleaseBase
```

Unix installs to `~/.local/bin`; Windows to `%LOCALAPPDATA%\OriginLM\bin`.
The scripts register a user LaunchAgent, systemd user service or Windows logon task
and start an independent IPC broker. They do not enable loopback HTTP or a website
content bridge. Each consumer must receive its own scoped grant; the initial broker
descriptor is not permission to silently share administrator privileges.

The archive also carries `LICENSE`, complete per-target dependency notices and
license texts, an SBOM and a member manifest. The installers still extract only the
exact `helper-<target>/originlm[.exe]` member; extra members are never executed or
used as filesystem paths.

Checksums detect corruption, not publisher authenticity. Unix verifies minisign when
`LAI_MINISIGN_PUBKEY` is configured and fails if the verifier/signature is unavailable.
Windows optionally pins `LAI_WINDOWS_SIGNER_THUMBPRINT` and requires a valid matching
Authenticode signature. The `v0.1.0-alpha.21` alpha is explicitly unsigned, so those
configured pins reject it; no Authenticode, notarization or trusted update signature
is claimed. Public signed releases remain unqualified.

Unix overrides: `LAI_RELEASE_BASE`, `LAI_BIN_DIR`, `LAI_STATE_DIR`, `LAI_BROKER_SOCKET`,
`LAI_MINISIGN_PUBKEY`. Paths must be absolute and cannot contain newlines. `LAI_STATE_DIR`
locates the service descriptor; runtime storage follows `LOCAL_AI_KIT_STATE_DIR` and
platform defaults. Windows has `ReleaseBase`, `InstallDir`, `StateDir` parameters;
Windows ARM64 is explicitly unsupported by the prepared packages.

## Uninstall

```sh
sh scripts/service/uninstall.sh
# Only to remove stored data as well:
sh scripts/service/uninstall.sh --purge
```

```powershell
& ./scripts/service/uninstall.ps1
# Only to remove stored data as well:
& ./scripts/service/uninstall.ps1 -Purge
```

Use the same path overrides used for installation. Ordinary uninstall removes the
service/binary and known registration files while preserving data and workspaces.
Purge is destructive and opt-in. The app provides an in-app mini-view, not an OS tray.

## Status and limits

`originlm status --json` inspects an existing service; it does not start or scan
one. Service managers provide restart/logon behavior. That behavior needs execution
on each OS; parsing a script on Linux does not qualify launchd or Task Scheduler.
No self-update or notification-delivery guarantee is implied by installation.
