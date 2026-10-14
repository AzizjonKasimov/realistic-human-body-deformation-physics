<#
.SYNOPSIS
Opens the viewer: plays a scenario, swing, or gesture once, then lets you step
through it frame by frame and see what happened and why.

.DESCRIPTION
The viewer shows the body as the app draws it, with what the injury rules
decided on each frame in the panel on the right, what the watchdogs flagged,
and a timeline along the bottom with a tick for each frame something was hurt
(red where a watchdog first flagged something). Keys:

  Space        play and pause
  Left/Right   a frame back or on (with Shift, ten)
  Home/End     the first or last frame
  1            the skin's stretch over the body (again: the muscle's; again: off)
  2            bruising, and flesh crushed
  3            what moves, and how fast
  4            where each injury happened (on at the start)
  5            the bones and joints, numbered
  A            the see-through view
  W            what moved the selection over the last 20 frames
  Esc          drop the selection

Click a point of flesh or a bone to select it: the panel says what it is, how
it moves, and what happened to it. Click the timeline to jump there.

With -Shot it saves the frame as it would show, with the overlays and the
selection asked for, and exits, to look at a run with no one at the keyboard.

.EXAMPLE
.\tools\inspect.ps1 -Scenario bat_hard_forearm
Step through the hard bat blow on the raised forearm.

.EXAMPLE
.\tools\inspect.ps1 -Strike "knife:0.331,0.406:-0.299,0.447:power=3:frames=8:windup=6:settle=87" -Frame 9
A play random testing found, on the frame it broke the spine.

.EXAMPLE
.\tools\inspect.ps1 -Scenario bat_hard_forearm -Frame 12 -Overlays stretch,events -Select bone:14 -Why -Shot output\inspect\forearm.png
Save frame 12 with the skin's stretch, what happened, and what moved the forearm's piece.
#>
param(
    [string]$Scenario = "",
    [string]$Strike = "",
    [string]$Gesture = "",
    # One run of the strike sweep, ALONG,ACROSS,STRENGTH (strike_scenarios --variant).
    [string]$Variant = "",
    # The frame to start on; by default the first.
    [int]$Frame = -1,
    # What to draw over the body: stretch, muscle, bruise, speed, events, bones.
    [string]$Overlays = "",
    # A point or bone to start with selected: point:N or bone:N.
    [string]$Select = "",
    # With -Select, start with what moved it over the last 20 frames.
    [switch]$Why,
    # Save the frame as shown to this PNG and exit.
    [string]$Shot = "",
    # Settings changed from the defaults, "NAME=VALUE,NAME=VALUE" (strike_scenarios --set).
    [string]$Set = "",
    [switch]$NoBuild
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $NoBuild) {
    Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics") -Label "Build app (Release)"
}
$exe = Join-Path $repoRoot "target\release\realistic_physics.exe"
$arguments = @("--inspect")
if ($Scenario) { $arguments += @("--scenario", $Scenario) }
if ($Strike) { $arguments += @("--strike", $Strike) }
if ($Gesture) { $arguments += @("--gesture", $Gesture) }
if ($Variant) { $arguments += @("--variant", $Variant) }
if ($Frame -ge 0) { $arguments += @("--frame", "$Frame") }
if ($Overlays) { $arguments += @("--overlays", $Overlays) }
if ($Select) { $arguments += @("--select", $Select) }
if ($Why) { $arguments += "--why" }
if ($Set) { $arguments += @("--set", $Set) }
if ($Shot) {
    $path = if ([IO.Path]::IsPathRooted($Shot)) { $Shot } else { Join-Path $repoRoot $Shot }
    $arguments += @("--shot", $path)
    & $exe @arguments
    if ($LASTEXITCODE -ne 0) { throw "the viewer failed (exit $LASTEXITCODE)" }
} else {
    # The viewer stays open until its window is closed.
    Start-Process -FilePath $exe -ArgumentList $arguments -WorkingDirectory $repoRoot
}
