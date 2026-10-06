<#
.SYNOPSIS
Saves screenshots of the real app, optionally after a scripted strike, at one or
more window sizes, plus a contact sheet that shows them all in one image.

.DESCRIPTION
Each capture briefly opens the app window, plays the strike at fixed steps,
saves the screen, and closes. Output goes to output\captures by default.

.EXAMPLE
.\tools\capture.ps1
Rest pose, normal and anatomy views, 1280x720.

.EXAMPLE
.\tools\capture.ps1 -Scenario torso_heavy_high
A tuned strike scenario (strike_scenarios --list shows them).

.EXAMPLE
.\tools\capture.ps1 -Strike "hammer:-0.26,0.34:0.30,0.34:power=4" -Sizes 800x600,1280x720,390x844
A custom swing in body coordinates at three window sizes.
#>
param(
    [string]$Scenario = "",
    [string]$Strike = "",
    # Comma-separated window sizes.
    [string]$Sizes = "1280x720",
    [ValidateSet("normal", "anatomy", "both")]
    [string]$View = "both",
    # Steps to play before saving; 0 plays the whole strike, or lets the body settle.
    [int]$Frames = 0,
    # Leave out the HUD, control buttons, and pointer ring.
    [switch]$NoUi,
    [string]$Out = "",
    # File name stem; defaults to the scenario name, "strike", or "rest".
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
    $Name = if ($Scenario) { $Scenario } elseif ($Strike) { "strike" } else { "rest" }
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
    $path = Join-Path $Out "$Name-$size.png"
    $arguments = @("--capture", $path, "--view", $View, "--size", $size, "--label", "$Name $size")
    if ($Scenario) { $arguments += @("--scenario", $Scenario) }
    if ($Strike) { $arguments += @("--strike", $Strike) }
    if ($Frames -gt 0) { $arguments += @("--frames", "$Frames") }
    if ($NoUi) { $arguments += "--no-ui" }
    Invoke-Checked -Label "Capture $Name at $size" -Command { & $Exe @arguments }
    if ($View -eq "both") {
        $images += Join-Path $Out "$Name-$size-normal.png"
        $images += Join-Path $Out "$Name-$size-anatomy.png"
    } else {
        $images += $path
    }
}

if ($images.Count -gt 1 -and (Test-Path $sheetExe)) {
    $columns = if ($View -eq "both") { 2 } else { [Math]::Min($images.Count, 3) }
    $sheet = Join-Path $Out "$Name-sheet.png"
    Invoke-Checked -Label "Contact sheet" -Command { & $sheetExe $sheet --height 480 --columns $columns @images }
}
