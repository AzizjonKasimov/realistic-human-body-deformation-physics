<#
.SYNOPSIS
Plays the same sweep, the same random fights, or the same random blows, two
ways in one build and sets them side by side: the defaults or settings A against settings B, or the
old way of a change against the new, with how likely chance is to make each
difference.

.DESCRIPTION
Each side is a sweep (strike_scenarios --sweep, or --quick for 9 runs a
scenario) written to output\ab\A and output\ab\B, then compared with
strike_scenarios --diff into output\ab\sweep_diff.txt; with -Fights, random
fights written to output\ab\fights\A and B and compared into
output\ab\fights_diff.txt; with -Random, random blows from each seed written
to output\ab\random\A and B and compared into output\ab\random_diff.txt.
Settings are any of the
engine's Materials by name (strike_scenarios --set). Experiments are changes
being tried behind trying("name") in the code, switched on through RP_TRY;
the test nothing_is_left_being_tried fails until one way is chosen. To play
more than two settings, or a timing in a gesture, use strike_scenarios
--grid, which sets every combination against the first in one table
(README, "Look Into A Run").

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

.PARAMETER Fights
Play this many random fights each way instead of the sweep (strike_scenarios
--fights), the same presses on both sides, and set them fight against fight
(output\ab\fights_diff.txt): each fighter's torn and slipped joints, joint by
joint, broken bones, torn skin, and watchdogs. 100 fights take about a
minute a side.

.PARAMETER Seed
With -Fights, which fights to play (1 unless given).

.PARAMETER Mix
With -Fights, which presses: game (as the Fight page is played), spam (the
left fighter's punches alone), or spam-lean (its punches and leans).

.PARAMETER Random
Play this many random blows from each seed each way instead of the sweep
(strike_scenarios --random), the same blows on both sides, and set them blow
against blow (output\ab\random_diff.txt): the plays each watchdog tripped in
and each rule every play follows was broken in, tool by tool, and the
injuries. 2000 blows take about a minute and a half a seed a side.

.PARAMETER RandomSeeds
With -Random, the seeds, comma separated (1,2,3).

.EXAMPLE
.\tools\ab.ps1 -TryB new_way -Fights 100
The same 100 random fights with the code's old way and with a change behind
trying("new_way").

.EXAMPLE
.\tools\ab.ps1 -TryB new_way -Random 2000
The same 2000 random blows from each of seeds 1, 2, and 3, with the code's
old way and with a change behind trying("new_way").

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
    [int]$Fights = 0,
    [int]$Seed = 1,
    [ValidateSet("game", "spam", "spam-lean")]
    [string]$Mix = "game",
    [int]$Random = 0,
    [string]$RandomSeeds = "1,2,3",
    [string]$Out = ""
)
if (($Fights -gt 0 -or $Random -gt 0) -and ($Only -or $Quick)) {
    throw "-Fights and -Random play random fights or blows instead of the sweep, so they take no -Only or -Quick"
}
if ($Fights -gt 0 -and $Random -gt 0) {
    throw "-Fights and -Random each play instead of the sweep; play one at a time"
}

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\ab"
}
Invoke-Cargo -Arguments @("build", "--release", "--bin", "strike_scenarios") -Label "Build the scenario tool (Release)"
$exe = Join-Path $repoRoot "target\release\strike_scenarios.exe"

# With -Fights each side plays the fights into fights\A and fights\B instead,
# and with -Random the blows into random\A and random\B, a folder a seed, so a
# side's folder never holds a sweep and fights or blows from different runs.
$sides = if ($Fights -gt 0) {
    Join-Path $Out "fights"
} elseif ($Random -gt 0) {
    Join-Path $Out "random"
} else {
    $Out
}

function Invoke-Side([string]$Name, [string]$Settings, [string]$Try) {
    $dir = Join-Path $sides $Name
    if ($Random -gt 0 -and (Test-Path -LiteralPath $dir)) {
        # Seeds played before and not asked for now would join the comparison.
        Remove-Item -LiteralPath $dir -Recurse -Force
    }
    New-Item -ItemType Directory -Force $dir | Out-Null
    # Each run's arguments and log; random blows play a run a seed.
    $runs = [System.Collections.Generic.List[object]]::new()
    if ($Fights -gt 0) {
        $arguments = @((Join-Path $dir "strike_scenarios.csv"), "--fights", $Fights, "--seed", $Seed)
        if ($Mix -ne "game") {
            $arguments += @("--mix", $Mix)
        }
        $runs.Add(@{ Arguments = $arguments; Log = (Join-Path $dir "fights.log") })
    } elseif ($Random -gt 0) {
        foreach ($each in $RandomSeeds -split ",") {
            $seedDir = Join-Path $dir "seed-$($each.Trim())"
            New-Item -ItemType Directory -Force $seedDir | Out-Null
            $runs.Add(@{
                    Arguments = @((Join-Path $seedDir "strike_scenarios.csv"), "--random", $Random, "--seed", $each.Trim())
                    Log       = (Join-Path $seedDir "random.log")
                })
        }
    } else {
        $arguments = @((Join-Path $dir "strike_scenarios.csv"), $(if ($Quick) { "--quick" } else { "--sweep" }))
        if ($Only) {
            $arguments += @("--only", $Only)
        }
        $runs.Add(@{ Arguments = $arguments; Log = (Join-Path $dir "sweep.log") })
    }
    $previous = $env:RP_TRY
    $env:RP_TRY = $Try
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        foreach ($run in $runs) {
            $arguments = $run.Arguments
            if ($Settings) {
                $arguments += @("--set", $Settings)
            }
            & $exe @arguments *> $run.Log
            if ($LASTEXITCODE -ne 0) {
                throw "side $Name failed (exit $LASTEXITCODE); see $($run.Log)"
            }
        }
    } finally {
        $env:RP_TRY = $previous
    }
    $what = @($(if ($Settings) { $Settings }), $(if ($Try) { "trying $Try" })) | Where-Object { $_ }
    Write-Host ("side {0}: {1} in {2:N0} s" -f $Name, $(if ($what) { $what -join "; " } else { "defaults" }), $watch.Elapsed.TotalSeconds)
}

Invoke-Side "A" $A $TryA
Invoke-Side "B" $B $TryB
& $exe (Join-Path $Out "strike_scenarios.csv") --diff (Join-Path $sides "A") (Join-Path $sides "B")
