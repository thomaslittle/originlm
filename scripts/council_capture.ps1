# UI council test run: deterministic capture for the three in-scope screens.
# Wraps capture_desktop_example.ps1 (real PrintWindow pixels, example mode,
# no accounts read) and adds native-scale header/main/footer crops plus a
# manifest with SHA-256 hashes so two rounds can be compared byte-for-byte.
#
#   pwsh scripts/council_capture.ps1 -Round 0
#   pwsh scripts/council_capture.ps1 -Round 1
param(
  [string]$Round = "0",
  [string]$OutRoot = "docs/design/council",
  [string]$Size = "1440x920",
  [string]$Exe = ""
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$pages = @("Overview", "Usage history", "Accounts")
$themes = @("dark", "light")
$outDir = Join-Path $OutRoot "round-$Round"
New-Item -ItemType Directory -Force $outDir | Out-Null

$captureArgs = @{
  Pages  = $pages
  Themes = $themes
  OutDir = $outDir
  Size   = $Size
  NoMini = $true
}
if ($Exe) { $captureArgs.Exe = $Exe }
& (Join-Path $PSScriptRoot "capture_desktop_example.ps1") @captureArgs

function Save-Crop([string]$source, [string]$dest, [int]$x, [int]$y, [int]$w, [int]$h) {
  $image = [System.Drawing.Image]::FromFile($source)
  try {
    $rect = New-Object System.Drawing.Rectangle $x, $y, $w, $h
    $crop = (New-Object System.Drawing.Bitmap $w, $h)
    $graphics = [System.Drawing.Graphics]::FromImage($crop)
    try {
      $graphics.DrawImage($image, (New-Object System.Drawing.Rectangle 0, 0, $w, $h), $rect, [System.Drawing.GraphicsUnit]::Pixel)
    } finally {
      $graphics.Dispose()
    }
    $crop.Save($dest, [System.Drawing.Imaging.ImageFormat]::Png)
    $crop.Dispose()
  } finally {
    $image.Dispose()
  }
}

$regions = @(
  @{ Name = "header"; X = 240; Y = 0; W = 1200; H = 150 },
  @{ Name = "main"; X = 240; Y = 150; W = 1200; H = 510 },
  @{ Name = "footer"; X = 0; Y = 856; W = 1440; H = 64 }
)
$entries = @()
foreach ($page in $pages) {
  $slug = ($page -replace "[^A-Za-z]", "").ToLower()
  foreach ($theme in $themes) {
    $file = Join-Path $outDir "$slug-$theme.png"
    if (-not (Test-Path $file)) { throw "capture missing: $file" }
    $hash = (Get-FileHash $file -Algorithm SHA256).Hash
    $width = 1440; $height = 920
    $image = [System.Drawing.Image]::FromFile($file)
    try { $width = $image.Width; $height = $image.Height } finally { $image.Dispose() }
    $crops = @()
    foreach ($region in $regions) {
      $cropFile = Join-Path $outDir ("{0}-{1}-{2}.png" -f $slug, $theme, $region.Name)
      $h = [Math]::Min($region.H, $height - $region.Y)
      $w = [Math]::Min($region.W, $width - $region.X)
      Save-Crop $file $cropFile $region.X $region.Y $w $h
      $crops += [ordered]@{
        region = $region.Name
        file   = [System.IO.Path]::GetFileName($cropFile)
        x = $region.X; y = $region.Y; width = $w; height = $h
        sha256 = (Get-FileHash $cropFile -Algorithm SHA256).Hash
      }
    }
    $entries += [ordered]@{
      screen = $page
      slug = $slug
      theme = $theme
      file = [System.IO.Path]::GetFileName($file)
      width = $width
      height = $height
      scale = "100%"
      sha256 = $hash
      crops = $crops
    }
  }
}

# Focus smoke: same Overview capture after eight Tab presses, so reviewers can
# score Interaction states / focus from evidence instead of "no evidence".
# Dark only: Windows' foreground lock makes the second process's Tab delivery
# unreliable, and the pass-1 focus evidence was the dark capture.
Get-ChildItem $outDir -Filter "focus-smoke-*.png" -ErrorAction SilentlyContinue | Remove-Item -Force
$focusOut = Join-Path $outDir "focus-smoke"
New-Item -ItemType Directory -Force $focusOut | Out-Null
$focusArgs = @{
  Pages  = @("Overview")
  Themes = @("dark")
  OutDir = $focusOut
  Size   = $Size
  NoMini = $true
  Tabs   = 8
}
if ($Exe) { $focusArgs.Exe = $Exe }
& (Join-Path $PSScriptRoot "capture_desktop_example.ps1") @focusArgs
foreach ($theme in @("dark")) {
  $src = Join-Path $focusOut "overview-$theme.png"
  if (-not (Test-Path $src)) { throw "focus capture missing: $src" }
  $dest = Join-Path $outDir "focus-smoke-$theme.png"
  Move-Item -Force $src $dest
  $image = [System.Drawing.Image]::FromFile($dest)
  try { $width = $image.Width; $height = $image.Height } finally { $image.Dispose() }
  $entries += [ordered]@{
    screen  = "Overview (focus smoke)"
    slug    = "focussmoke"
    theme   = $theme
    file    = [System.IO.Path]::GetFileName($dest)
    width   = $width
    height  = $height
    scale   = "100%"
    sha256  = (Get-FileHash $dest -Algorithm SHA256).Hash
    crops   = @()
  }
}
Remove-Item -Recurse -Force $focusOut

$manifest = [ordered]@{
  round = $Round
  captured_at = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  machine = $env:COMPUTERNAME
  command = "pwsh scripts/council_capture.ps1 -Round $Round"
  window = $Size
  reduced_motion = $true
  example_mode = $true
  focus_smoke = $true
  screens = $entries
}
$manifestPath = Join-Path $outDir "manifest.json"
$manifest | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 $manifestPath
Write-Output "wrote $manifestPath"
