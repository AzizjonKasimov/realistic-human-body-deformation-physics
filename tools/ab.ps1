<#
.SYNOPSIS
Plays the same sweep two ways in one build and sets them side by side: the
defaults or settings A against settings B, or the old way of a change against
the new, with how likely chance is to make each difference.

.DESCRIPTION
Each side is a sweep (strike_scenarios --sweep, or --quick for 9 runs a
scenario) written to output\ab\A and output\ab\B, then compared with
strike_scenarios --diff into output\ab\sweep_diff.txt. Settings are any of the
engine's Materials by name (strike_scenarios --set). Experiments are changes
being tried behind trying("name") in the code, switched on through RP_TRY;
the test nothing_is_left_being_tried fails until one way is chosen.

.PARAMETER A
Settings for side A, "NAME=VALUE,NAME=VALUE"; the defaults if empty.

.PARAMETER B
Settings for side B.

.PARAMETER TryA
Experiments switched on for side A (RP_TRY), comma separated.

.PARAMETER TryB
Experiments switched on for side B.

.PARAMETER Only
Scenarios to play, comma separated; all if empty.

.PARAMETER Quick
9 runs a scenario instead of 45: a change's direction in a fifth of the time.

.EXAMPLE
.\tools\ab.ps1 -B "gravity=2.867" -Only shoulder_blunt,bat_hard_swing -Quick
The scenarios at full gravity against the old 0.29 g.

.EXAMPLE
.\tools\ab.ps1 -TryB pinned_joints
The code's old way against a change behind trying("pinned_joints").
#>
param(
    [string]$A = "",
    [string]$B = "",
    [string]$TryA = "",
    [string]$TryB = "",
    [string]$Only = "",
    [switch]$Quick,
    [string]$Out = ""
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\ab"
}
Invoke-Cargo -Arguments @("build", "--release", "--bin", "strike_scenarios") -Label "Build the scenario tool (Release)"
$exe = Join-Path $repoRoot "target\release\strike_scenarios.exe"

function Invoke-Side([string]$Name, [string]$Settings, [string]$Try) {
    $dir = Join-Path $Out $Name
    New-Item -ItemType Directory -Force $dir | Out-Null
    $arguments = @((Join-Path $dir "strike_scenarios.csv"), $(if ($Quick) { "--quick" } else { "--sweep" }))
    if ($Only) {
        $arguments += @("--only", $Only)
    }
    if ($Settings) {
        $arguments += @("--set", $Settings)
    }
    $previous = $env:RP_TRY
    $env:RP_TRY = $Try
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        & $exe @arguments *> (Join-Path $dir "sweep.log")
        if ($LASTEXITCODE -ne 0) {
            throw "side $Name failed (exit $LASTEXITCODE); see $(Join-Path $dir 'sweep.log')"
        }
    } finally {
        $env:RP_TRY = $previous
    }
    $what = @($(if ($Settings) { $Settings }), $(if ($Try) { "trying $Try" })) | Where-Object { $_ }
    Write-Host ("side {0}: {1} in {2:N0} s" -f $Name, $(if ($what) { $what -join "; " } else { "defaults" }), $watch.Elapsed.TotalSeconds)
}

Invoke-Side "A" $A $TryA
Invoke-Side "B" $B $TryB
& $exe (Join-Path $Out "strike_scenarios.csv") --diff (Join-Path $Out "A") (Join-Path $Out "B")
