# Remove the OriginLM helper task, tray autostart, Native Messaging manifests and
# the binary. User data is kept unless -Purge is passed.
param(
  [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'OriginLM\bin'),
  [string]$StateDir = (Join-Path $env:USERPROFILE '.originlm'),
  [string]$DataDir = $(if ($env:LOCAL_AI_KIT_STATE_DIR) { $env:LOCAL_AI_KIT_STATE_DIR } else { Join-Path $env:LOCALAPPDATA 'originlm' }),
  [switch]$Purge
)
$ErrorActionPreference = 'Stop'
foreach ($path in @($InstallDir, $StateDir, $DataDir)) {
  if ([string]::IsNullOrWhiteSpace($path) -or -not [IO.Path]::IsPathRooted($path) -or $path.Contains("`n")) { throw 'Uninstall paths must be absolute and contain no newlines' }
}
if ($Purge) {
  foreach ($path in @($StateDir, $DataDir)) {
    $full = [IO.Path]::GetFullPath($path).TrimEnd('\', '/')
    if ($full -eq [IO.Path]::GetPathRoot($full).TrimEnd('\', '/') -or $full -eq [IO.Path]::GetFullPath($env:USERPROFILE).TrimEnd('\', '/')) { throw 'Refusing to purge the filesystem root or user profile' }
  }
}
Stop-ScheduledTask -TaskName 'OriginLM Helper' -ErrorAction SilentlyContinue
Unregister-ScheduledTask -TaskName 'OriginLM Helper' -Confirm:$false -ErrorAction SilentlyContinue
$startup = [Environment]::GetFolderPath('Startup')
if ($startup) { Remove-Item -Force (Join-Path $startup 'OriginLM Tray.lnk') -ErrorAction SilentlyContinue }

$manifestRoots = @(
  (Join-Path $env:LOCALAPPDATA 'Google\Chrome\User Data\NativeMessagingHosts'),
  (Join-Path $env:LOCALAPPDATA 'Chromium\User Data\NativeMessagingHosts'),
  (Join-Path $env:LOCALAPPDATA 'Microsoft\Edge\User Data\NativeMessagingHosts'),
  (Join-Path $env:LOCALAPPDATA 'BraveSoftware\Brave-Browser\User Data\NativeMessagingHosts')
)
foreach ($root in $manifestRoots) {
  Remove-Item -Force (Join-Path $root 'com.originlm.host.json') -ErrorAction SilentlyContinue
  Remove-Item -Force (Join-Path $root 'dev.originlm.helper.json') -ErrorAction SilentlyContinue
}

# native_host.py registers Windows hosts in HKCU, not only in browser folders.
foreach ($browser in @('Google\Chrome', 'Chromium', 'Microsoft\Edge', 'Mozilla')) {
  Remove-Item -Recurse -Force "HKCU:\Software\$browser\NativeMessagingHosts\com.originlm.host" -ErrorAction SilentlyContinue
}

Remove-Item -Force (Join-Path $InstallDir 'originlm.exe') -ErrorAction SilentlyContinue
if ($Purge) {
  Remove-Item -Recurse -Force $StateDir -ErrorAction SilentlyContinue
  Remove-Item -Recurse -Force $DataDir -ErrorAction SilentlyContinue
  Write-Host 'Removed the binary, task and user data'
} else {
  Remove-Item -Force (Join-Path $StateDir 'broker-token.json') -ErrorAction SilentlyContinue
  Write-Host "Removed the binary and task; user data kept in $StateDir (use -Purge to delete)"
}
