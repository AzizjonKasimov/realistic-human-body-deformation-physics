<#
.SYNOPSIS
Saves screenshots of the real app, optionally after a scripted strike or
gesture, at one or more window sizes, plus a contact sheet that shows them all
in one image.

.DESCRIPTION
Each capture briefly opens the app window, plays the strike or gesture at fixed
steps, saves the screen, and closes. With -Every N it also saves the screen
every N steps on the way, as a film strip of how things move. Output goes to
output\captures by default.

.EXAMPLE
.\tools\capture.ps1
Rest pose, normal and anatomy views, 1280x720.

.EXAMPLE
.\tools\capture.ps1 -Scenario torso_heavy_high
A tuned scenario (strike_scenarios --list shows them).

.EXAMPLE
.\tools\capture.ps1 -Strike "hammer:-0.26,0.34:0.30,0.34:frames=14" -Sizes 800x600,1280x720,390x844
A custom swing in body coordinates at three window sizes.

.EXAMPLE
.\tools\capture.ps1 -Gesture "bat:-0.3,0.3:wait=10:down:-0.08,0.3/15:-0.08,0.6/60:up:wait=20" -Every 5 -View normal
A gesture played the way the app is, saved every 5 steps as a film strip.
#>
param(
    [string]$Scenario = "",
    [string]$Strike = "",
    [string]$Gesture = "",
    # Comma-separated window sizes.
    [string]$Sizes = "1280x720",
    [ValidateSet("normal", "anatomy", "both")]
    [string]$View = "both",
    # Steps to play before saving; 0 plays the whole strike, or lets the body settle.
    [int]$Frames = 0,
    # Also save the screen every this many steps, for a film strip.
    [int]$Every = 0,
    # Leave out the HUD, control buttons, and pointer ring.
    [switch]$NoUi,
    [string]$Out = "",
    # File name stem; defaults to the scenario name, "strike", "gesture", or "rest".
    [string]$Name = "",
    # App to run instead of building and using target\release\realistic_physics.exe.
    [string]$Exe = "",
    [switch]$NoBuild
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\captures"
}
New-Item -ItemType Directory -Force $Out | Out-Null
if (-not $Name) {
    $Name = if ($Scenario) { $Scenario } elseif ($Strike) { "strike" } elseif ($Gesture) { "gesture" } else { "rest" }
}
if (-not $Exe) {
    if (-not $NoBuild) {
        Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics", "--bin", "contact_sheet") -Label "Build app and contact sheet (Release)"
    }
    $Exe = Join-Path $repoRoot "target\release\realistic_physics.exe"
}
$sheetExe = Join-Path $repoRoot "target\release\contact_sheet.exe"

$images = @()
foreach ($size in ($Sizes -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
    $stem = "$Name-$size"
    $path = Join-Path $Out "$stem.png"
    # A film strip's frames are STEM-NNNN.png; clear an earlier strip's first.
    $stripPattern = "^$([regex]::Escape($stem))-\d{4}(-normal|-anatomy)?\.png$"
    Get-ChildItem -LiteralPath $Out -File | Where-Object { $_.Name -match $stripPattern } | Remove-Item
    $arguments = @("--capture", $path, "--view", $View, "--size", $size, "--label", "$Name $size")
    if ($Scenario) { $arguments += @("--scenario", $Scenario) }
    if ($Strike) { $arguments += @("--strike", $Strike) }
    if ($Gesture) { $arguments += @("--gesture", $Gesture) }
    if ($Frames -gt 0) { $arguments += @("--frames", "$Frames") }
    if ($Every -gt 0) { $arguments += @("--every", "$Every") }
    if ($NoUi) { $arguments += "--no-ui" }
    Invoke-Checked -Label "Capture $Name at $size" -Command { & $Exe @arguments }
    if ($Every -gt 0) {
        $images += Get-ChildItem -LiteralPath $Out -File |
            Where-Object { $_.Name -match $stripPattern } |
            Sort-Object Name |
            ForEach-Object { $_.FullName }
    } elseif ($View -eq "both") {
        $images += Join-Path $Out "$stem-normal.png"
        $images += Join-Path $Out "$stem-anatomy.png"
    } else {
        $images += $path
    }
}

if ($images.Count -gt 1 -and (Test-Path $sheetExe)) {
    $columns = if ($Every -gt 0) {
        if ($View -eq "both") { 4 } else { 5 }
    } elseif ($View -eq "both") { 2 } else { [Math]::Min($images.Count, 3) }
    $height = if ($Every -gt 0) { 300 } else { 480 }
    $sheet = Join-Path $Out "$Name-sheet.png"
    Invoke-Checked -Label "Contact sheet" -Command { & $sheetExe $sheet --height $height --columns $columns @images }
}
