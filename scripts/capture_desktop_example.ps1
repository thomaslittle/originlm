# Capture the desktop control center in example mode on Windows.
# Manual design capture, not a PR gate. See TESTING.md.
# Run from the repository root after `cargo build -p originlm-desktop`.
# Example mode reads no accounts. PrintWindow captures only the OriginLM window's
# own pixels, so other windows never end up in the images; a capture whose
# window handle is not the OriginLM process's main window is discarded.
#
#   pwsh scripts/capture_desktop_example.ps1                                  # every page, dark and light, plus mini view
#   pwsh scripts/capture_desktop_example.ps1 -Pages Overview -Themes light    # one page, one theme
#   pwsh scripts/capture_desktop_example.ps1 -Pages Overview -States loading,offline,stale,empty
#   pwsh scripts/capture_desktop_example.ps1 -Pages Overview,Developer -Size 1000x660 -Suffix min
#
# File names: <page>-<theme>[-<state>][-<suffix>].png, page lowercased with
# non-letters removed (for example "appsgrants-dark.png").
param(
  [string[]]$Pages = @("Overview", "Usage history", "Accounts", "Models", "Developer", "Apps & grants", "Sessions", "Playground", "Diagnostics", "Settings"),
  [string[]]$Themes = @("dark", "light"),
  # Example-mode state previews (LAI_DESKTOP_EXAMPLE_STATE): loading, offline, stale, empty.
  [string[]]$States = @(),
  [string]$OutDir = "docs/design/screens/desktop/after",
  [string]$Size = "1440x920",
  [string]$Suffix = "",
  [switch]$NoMini,
  [string]$Exe = "",
  [int]$Wait = 5,
  # Send this many Tab presses before capturing. Used for the council
  # focus smoke, so reviewers get visible-focus evidence from a still.
  [int]$Tabs = 0
)
$ErrorActionPreference = "Stop"
# `pwsh -File` passes "a,b" as one string; accept both forms.
$States = @($States | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Themes = @($Themes | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
$Pages = @($Pages | ForEach-Object { $_ -split "," } | Where-Object { $_ })
if (-not $Exe) {
  $metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
  $Exe = Join-Path $metadata.target_directory "debug/originlm-desktop.exe"
}
if (-not (Test-Path $Exe)) { throw "Build first: cargo build -p originlm-desktop ($Exe not found)" }
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class OriginLMCapture {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);
  [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr h);
  [DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@
[OriginLMCapture]::SetProcessDPIAware() | Out-Null
New-Item -ItemType Directory -Force $OutDir | Out-Null
$state = Join-Path $env:TEMP "originlm-desktop-capture-state"
New-Item -ItemType Directory -Force $state | Out-Null

function Capture([string]$page, [string]$theme, [string]$exampleState, [string]$file, [bool]$mini, [int]$tabs = 0) {
  Remove-Item (Join-Path $state "prefs.json") -ErrorAction SilentlyContinue
  $env:LAI_DESKTOP_STATE_DIR = $state
  $env:LAI_DESKTOP_WINDOW = $Size
  $env:LAI_DESKTOP_PAGE = $page
  $env:LAI_DESKTOP_THEME = $theme
  # Council test run: freeze the eye-logo and easing animations so two
  # captures of the same screen/theme are pixel-identical.
  $env:ORIGINLM_REDUCED_MOTION = "1"
  # Pin "now" so relative labels ("Updated 2s", reset countdowns) do not drift
  # between two captures. 2026-11-06T12:00:00Z.
  $env:ORIGINLM_CAPTURE_TIME = "1793966400"
  if ($exampleState) { $env:LAI_DESKTOP_EXAMPLE_STATE = $exampleState } else { Remove-Item Env:LAI_DESKTOP_EXAMPLE_STATE -ErrorAction SilentlyContinue }
  if ($mini) { $env:LAI_DESKTOP_MINI_VIEW = "1" } else { Remove-Item Env:LAI_DESKTOP_MINI_VIEW -ErrorAction SilentlyContinue }
  $process = Start-Process -FilePath $Exe -ArgumentList "--example" -PassThru
  try {
    $deadline = (Get-Date).AddSeconds(30)
    do { Start-Sleep -Milliseconds 300; $process.Refresh() } until ($process.MainWindowHandle -ne 0 -or (Get-Date) -gt $deadline)
    Start-Sleep -Seconds $Wait
    $process.Refresh()
    $handle = $process.MainWindowHandle
    if ($handle -eq 0) { throw "OriginLM window did not appear for $page" }
    # Only ever capture a window owned by the process this script started.
    [uint32]$owner = 0
    [OriginLMCapture]::GetWindowThreadProcessId($handle, [ref]$owner) | Out-Null
    if ($owner -ne $process.Id) { Write-Warning "Window does not belong to OriginLM; discarded"; return }
    if ($tabs -gt 0) {
      # Focus smoke: Tab through the real focus order, then capture the ring.
      # WM_KEYDOWN/WM_KEYUP for VK_TAB (0x09) so GPUI sees native key input.
      # Attach to the foreground thread so Windows actually lets this process
      # activate the freshly launched window; otherwise Tab is delivered to a
      # non-focused window and quietly dropped.
      $fg = [OriginLMCapture]::GetForegroundWindow()
      [uint32]$fgPid = 0
      $fgThread = [OriginLMCapture]::GetWindowThreadProcessId($fg, [ref]$fgPid)
      [uint32]$appPid = 0
      $appThread = [OriginLMCapture]::GetWindowThreadProcessId($handle, [ref]$appPid)
      [OriginLMCapture]::AttachThreadInput($fgThread, $appThread, $true) | Out-Null
      [OriginLMCapture]::SetForegroundWindow($handle) | Out-Null
      [OriginLMCapture]::SetFocus($handle) | Out-Null
      [OriginLMCapture]::AttachThreadInput($fgThread, $appThread, $false) | Out-Null
      Start-Sleep -Milliseconds 500
      for ($i = 0; $i -lt $tabs; $i++) {
        # Real key events, not window messages: GPUI's focus traversal listens
        # to the normal keyboard queue.
        [OriginLMCapture]::keybd_event(0x09, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 40
        [OriginLMCapture]::keybd_event(0x09, 0, 2, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 160
      }
      Start-Sleep -Milliseconds 800
    }
    $rect = New-Object OriginLMCapture+RECT
    [OriginLMCapture]::GetWindowRect($handle, [ref]$rect) | Out-Null
    # Drop the invisible resize border Windows reports around the frame.
    $border = 8
    $width = $rect.R - $rect.L; $height = $rect.B - $rect.T
    $full = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($full)
    $dc = $graphics.GetHdc()
    [OriginLMCapture]::PrintWindow($handle, $dc, 2) | Out-Null
    $graphics.ReleaseHdc($dc); $graphics.Dispose()
    $crop = $full.Clone((New-Object System.Drawing.Rectangle $border, 0, ($width - 2 * $border), ($height - $border)), $full.PixelFormat)
    $crop.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    $crop.Dispose(); $full.Dispose()
    Write-Output "saved $file"
  } finally {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    Remove-Item Env:LAI_DESKTOP_EXAMPLE_STATE -ErrorAction SilentlyContinue
  }
}

function Name([string]$page, [string]$theme, [string]$extra) {
  $name = ($page -replace '[^A-Za-z]', '').ToLower() + "-$theme"
  if ($extra) { $name += "-$extra" }
  if ($Suffix) { $name += "-$Suffix" }
  return Join-Path $OutDir "$name.png"
}

foreach ($theme in $Themes) {
  foreach ($page in $Pages) {
    if ($States.Count -gt 0) {
      foreach ($exampleState in $States) { Capture $page $theme $exampleState (Name $page $theme $exampleState) $false $Tabs }
    } else {
      Capture $page $theme "" (Name $page $theme "") $false $Tabs
    }
  }
  if (-not $NoMini -and $States.Count -eq 0 -and $Pages -contains "Overview") {
    Capture "Overview" $theme "" (Name "mini" $theme "") $true
  }
}
