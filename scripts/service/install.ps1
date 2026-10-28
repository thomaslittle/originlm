# OriginLM background service installer for Windows.
# No admin rights: one binary under LOCALAPPDATA and one logon task.
#
#   & ./scripts/service/install.ps1 -ReleaseBase $OriginLMReleaseBase
param(
  [Parameter(Mandatory=$true)][ValidateNotNullOrEmpty()][string]$ReleaseBase,
  [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'OriginLM\bin'),
  [string]$StateDir = (Join-Path $env:USERPROFILE '.originlm')
)
$ErrorActionPreference = 'Stop'

$target = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
  'X64' { 'x86_64-pc-windows-msvc' }
  'Arm64' { throw 'Windows ARM64 packages are not available in this source preview.' }
  default { throw "Unsupported architecture: $($_)" }
}
$archive = "originlm-$target.zip"
$temp = Join-Path ([System.IO.Path]::GetTempPath()) ("originlm-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $temp | Out-Null
try {
  Write-Host "Downloading $archive"
  Invoke-WebRequest -UseBasicParsing -Uri "$ReleaseBase/$archive" -OutFile (Join-Path $temp $archive)
  Invoke-WebRequest -UseBasicParsing -Uri "$ReleaseBase/SHA256SUMS" -OutFile (Join-Path $temp 'SHA256SUMS')
  $checksumLines = @(Get-Content (Join-Path $temp 'SHA256SUMS') | Where-Object { $_ -match ('^[0-9a-fA-F]{64}  ' + [regex]::Escape($archive) + '$') })
  if ($checksumLines.Count -ne 1) { throw 'Expected exactly one checksum for this archive' }
  $expected = $checksumLines[0].Substring(0, 64)
  $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $temp $archive)).Hash
  if ($expected -ine $actual) { throw 'Checksum mismatch' }

  # Extract only the exact helper entry. No archive path can escape the temp directory.
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $zip = [System.IO.Compression.ZipFile]::OpenRead((Join-Path $temp $archive))
  $payload = Join-Path $temp 'originlm.exe'
  try {
    $entries = @($zip.Entries | Where-Object { $_.FullName -ceq "helper-$target/originlm.exe" })
    if ($entries.Count -ne 1 -or $entries[0].Length -le 0 -or $entries[0].Length -gt 40MB) { throw 'Invalid helper archive member' }
    [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entries[0], $payload, $false)
  } finally { $zip.Dispose() }
  # An explicitly configured certificate identity must never silently downgrade.
  if ($env:LAI_WINDOWS_SIGNER_THUMBPRINT) {
    $signature = Get-AuthenticodeSignature -FilePath $payload
    if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Thumbprint -ine $env:LAI_WINDOWS_SIGNER_THUMBPRINT) { throw 'Helper signature verification failed' }
  }
  New-Item -ItemType Directory -Force -Path $InstallDir, $StateDir | Out-Null
  Copy-Item $payload (Join-Path $InstallDir 'originlm.exe') -Force

  $token = Join-Path $StateDir 'broker-token.json'
  $socket = 'originlm-user'
  $arguments = "serve --ipc $socket --independent --token-file `"$token`" --allow-passive-discovery"
  $action = New-ScheduledTaskAction -Execute (Join-Path $InstallDir 'originlm.exe') -Argument $arguments
  $trigger = New-ScheduledTaskTrigger -AtLogOn
  $settings = New-ScheduledTaskSettingsSet -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -AllowStartIfOnBatteries
  Register-ScheduledTask -TaskName 'OriginLM Helper' -Action $action -Trigger $trigger -Settings $settings -Force | Out-Null
  Start-ScheduledTask -TaskName 'OriginLM Helper'

  Write-Host "Installed. Task: OriginLM Helper (Task Scheduler logon task)"
  Write-Host "Uninstall from the source checkout: & ./scripts/service/uninstall.ps1"
} finally {
  Remove-Item -Recurse -Force $temp -ErrorAction SilentlyContinue
}
