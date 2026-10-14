<#
.SYNOPSIS
Films the same strike, gesture, or scenario with a commit's app (default main)
and with the working tree's, and lays the two film strips out in one sheet, the
commit's frames above the working tree's, to show how a change alters what the
body does under a blow.

.DESCRIPTION
The commit's app is built once and kept as bench.ps1 keeps it
(target\compare\apps\<commit>). Each side plays in the real app through
capture.ps1, which saves the screen every -Every steps, so the frames in a
column show the same moment. Output goes to output\before_after by default:
each side's frames and NAME-before-after.png.

A scenario plays on both sides only if the commit has it. -RefScenario plays
another one on the commit's side, as when a scenario was restaged or is new;
-Strike and -Gesture play the same swing on both. With none of them, the sheet
shows both bodies at rest.

.EXAMPLE
.\tools\before_after.ps1 -Scenario bat_hard_forearm -Every 12 -Frames 72 -Crop 400,30,480,590
A scenario both sides have, every 12 steps up to step 72, showing the body.

.EXAMPLE
.\tools\before_after.ps1 -Strike "bat:-0.4,-0.24:0.1,0.16:frames=9:settle=90" -Every 16 -Frames 96 -Name head
One custom swing on both sides.

.EXAMPLE
.\tools\before_after.ps1 -Scenario bat_tap_on_the_guard -RefScenario bat_moderate_forearm -Every 15
A new scenario against the nearest one the commit has.
#>
param(
    [string]$Ref = "main",
    [string]$Scenario = "",
    # The scenario to play on the commit's side instead of -Scenario.
    [string]$RefScenario = "",
    [string]$Strike = "",
    [string]$Gesture = "",
    # Steps to play; 0 plays the whole strike, or lets the body settle.
    [int]$Frames = 0,
    # Save the screen every this many steps; 0 saves only the last.
    [int]$Every = 0,
    [ValidateSet("normal", "anatomy")]
    [string]$View = "normal",
    [string]$Size = "1280x720",
    # Height of each frame in the sheet, in pixels.
    [int]$Height = 360,
    # The part of each frame to show, X,Y,W,H in the frame's pixels from the
    # top left, such as 400,30,480,590 for the body at 1280x720.
    [int[]]$Crop = @(),
    [switch]$NoUi,
    # Use the working tree's last release build instead of building it.
    [switch]$NoBuild,
    [string]$Name = "",
    [string]$Out = ""
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\before_after"
}
New-Item -ItemType Directory -Force $Out | Out-Null
if (-not $Name) {
    $Name = if ($Scenario) { $Scenario } elseif ($Strike) { "strike" } elseif ($Gesture) { "gesture" } else { "rest" }
}
$sha = Resolve-Commit $Ref
$refLabel = "$Ref ($($sha.Substring(0, 10)))"
$refExe = Get-CommitApp $sha $refLabel
if (-not $NoBuild) {
    Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics", "--bin", "contact_sheet") -Label "Build working tree app and contact sheet (Release)"
}

# Films one side and returns its frames in order of their steps.
function Invoke-Side([string]$Side, [string]$Exe, [string]$Play) {
    $stem = "$Name-$Side"
    $capture = @{ Name = $stem; Out = $Out; Sizes = $Size; View = $View; Exe = $Exe }
    if ($Play) { $capture.Scenario = $Play }
    if ($Strike) { $capture.Strike = $Strike }
    if ($Gesture) { $capture.Gesture = $Gesture }
    if ($Frames -gt 0) { $capture.Frames = $Frames }
    if ($Every -gt 0) { $capture.Every = $Every }
    if ($NoUi) { $capture.NoUi = $true }
    Write-Host ""
    Write-Host "==> Film $Side"
    & "$PSScriptRoot\capture.ps1" @capture | Out-Null
    $pattern = if ($Every -gt 0) {
        "^$([regex]::Escape("$stem-$Size"))-\d{4}\.png$"
    } else {
        "^$([regex]::Escape("$stem-$Size"))\.png$"
    }
    $frames = @(Get-ChildItem -LiteralPath $Out -File | Where-Object { $_.Name -match $pattern } | Sort-Object Name | ForEach-Object { $_.FullName })
    if ($frames.Count -eq 0) {
        throw "capture.ps1 saved no frames for $Side"
    }
    return $frames
}

# A function hands back a one-frame list as the bare path; @() keeps it a list.
$before = @(Invoke-Side "before" $refExe $(if ($RefScenario) { $RefScenario } else { $Scenario }))
$after = @(Invoke-Side "after" (Join-Path $repoRoot "target\release\realistic_physics.exe") $Scenario)

# The shorter strip holds its last frame, so each column stays one moment.
$columns = [Math]::Max($before.Count, $after.Count)
$rows = foreach ($strip in @(, $before) + @(, $after)) {
    0..($columns - 1) | ForEach-Object { $strip[[Math]::Min($_, $strip.Count - 1)] }
}
$sheet = Join-Path $Out "$Name-before-after.png"
if ($Crop.Count -notin 0, 4) {
    throw "-Crop takes four numbers, X,Y,W,H"
}
$cropArguments = if ($Crop) { @("--crop", ($Crop -join ",")) } else { @() }
Invoke-Checked -Label "Contact sheet" -Command {
    & (Join-Path $repoRoot "target\release\contact_sheet.exe") $sheet --height $Height --columns $columns @cropArguments @rows
}
Write-Host ""
Write-Host "wrote $sheet (top: $refLabel, bottom: working tree)"
