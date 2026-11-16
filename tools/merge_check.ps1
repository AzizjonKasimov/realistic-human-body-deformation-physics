<#
.SYNOPSIS
Runs every check a change must pass before it merges, on the working tree and
on a commit (main unless -Ref), and prints one verdict: for each check the
good-enough rule holds, whether the change kept it, made it better, or made it
clearly worse.

.DESCRIPTION
The good-enough rule (README, "Check A Change Before Merging") holds a change
to this: the tests and verify.ps1 pass, the browser build simulates as the
native one and still runs smoothly, and nothing else that must hold is worse
beyond chance: the scenarios' bands and real-life checks across the sweep,
the rules every play follows, and the watchdogs for a body that breaks, in
the sweep and in random blows over three seeds; and in random fights between
the side-view fighters, their torn and slipped joints, broken bones, and
falls with no knockout. The rest may drift; it is reported, not held.

The round runs these steps, each logged in -Out (output\merge_check):

  verify   verify.ps1 -SkipSweep (compare plays the sweep)
  compare  compare.ps1 -Sweep -Fights -SideFights -Random against the
           commit: the sweep, random fights, random side-view fights, and
           random blows on both sides, each set side by side with how likely
           chance is to make each difference
  moved    the moved-body tests on the working tree (strike_scenarios --moved)
  parity   parity.ps1: the browser build simulates as the native one
  bench    bench.ps1: the step time against the commit
  web      the phone checks (touch_check.ps1) upright and sideways, and with
           -Fight the Fight page's too

A step that passed before on the same working tree, commit, and options is
not run again, so a round stopped partway, or run again after a fix to a
failed step's cause outside the code, picks up where it stopped; -Fresh runs
every step. The whole round takes about 40 minutes, the commit's results
cached for the next round against it. The verdict goes to report.txt, and the script
exits 1 when a check that must hold failed or got clearly worse.

.PARAMETER Ref
The commit to compare against; main by default.

.PARAMETER Blows
Random blows a seed (2000).

.PARAMETER Seeds
The random blows' seeds (1, 2, 3).

.PARAMETER Fights
Random fights, the same fights on both sides (300).

.PARAMETER SideFights
Random fights between the side-view fighters, the same fights on both sides
(100, 20 s each). A commit without strike_scenarios --side-fights (before
v0.18) is not compared on them: the row says so and does not block.

.PARAMETER Fight
A checkout of the game's repository: also build the site with the Fight page
and play its buttons on a phone.

.PARAMETER Skip
Steps to leave out, such as bench or web; a check they hold is listed as not
run.

.PARAMETER Fresh
Run every step, even those that passed before on this tree.

.PARAMETER Report
Run nothing, and print the last round's verdict again from its results.

.EXAMPLE
.\tools\merge_check.ps1
The working tree against main.

.EXAMPLE
.\tools\merge_check.ps1 -Ref v0.15.1 -Fight E:\PersonalProjects\realistic_physics_fight
Against the commit v0.15.1 was tagged on, with the Fight page's phone checks.
#>
param(
    [string]$Ref = "main",
    [int]$Blows = 2000,
    # Lists, or comma separated text, as a script run with pwsh -File takes them.
    [string[]]$Seeds = @("1", "2", "3"),
    [int]$Fights = 300,
    [int]$SideFights = 100,
    [string]$Fight = "",
    [string[]]$Skip = @(),
    [switch]$Fresh,
    [switch]$Report,
    [string]$Out = ""
)

. "$PSScriptRoot\common.ps1"

$Seeds = @($Seeds -split "," | ForEach-Object { [int]$_.Trim() })
$Skip = @($Skip -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$steps = "verify", "compare", "moved", "parity", "bench", "web"
foreach ($name in $Skip) {
    if ($steps -notcontains $name) {
        throw "-Skip takes $($steps -join ', '); got $name"
    }
}

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\merge_check"
}
New-Item -ItemType Directory -Force $Out | Out-Null
$refSha = Resolve-Commit $Ref
$refShort = $refSha.Substring(0, 10)
$pwsh = (Get-Process -Id $PID).Path
if ($Fight) {
    $Fight = (Resolve-Path -LiteralPath $Fight).Path
}

# The working tree as it is: its commit, its changes, and files git does not
# track yet, hashed, so a step that passed on this very tree is not run again.
# This script is left out: how it reads the results changes none of them.
function Get-TreeState([string]$Root) {
    $self = ":(exclude)tools/merge_check.ps1"
    $head = (& git -C $Root rev-parse HEAD).Trim()
    $changes = (& git -C $Root diff HEAD --binary -- . $self) -join "`n"
    $untracked = @(& git -C $Root ls-files --others --exclude-standard -- . $self)
    $hashes = if ($untracked) { (& git -C $Root hash-object -- @untracked) -join "," } else { "" }
    $bytes = [Text.Encoding]::UTF8.GetBytes("$head`n$($untracked -join ',')`n$hashes`n$changes")
    return [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).Substring(0, 16)
}

$tree = Get-TreeState $repoRoot
$fightState = if ($Fight) { Get-TreeState $Fight } else { "" }
$progressFile = Join-Path $Out "progress.csv"
$progress = @(if (Test-Path -LiteralPath $progressFile) { Import-Csv -LiteralPath $progressFile })
$exits = @{}
# With -Report, the steps whose last run was on another tree or commit.
$stale = @()

# Runs one step's commands into its log, unless it passed before on the same
# tree, commit, and options ($Key).
function Invoke-Step([string]$Name, [string]$Key, [scriptblock]$Commands) {
    if ($Skip -contains $Name) {
        Write-Host "==> ${Name}: left out (-Skip)"
        return
    }
    $state = "$tree $Key"
    $before = $progress | Where-Object { $_.step -eq $Name } | Select-Object -Last 1
    if ($Report) {
        # The last round's result as it stands, on whatever tree it ran.
        if ($before) {
            $exits[$Name] = [int]$before.exit
            if ($before.state -ne $state) {
                $script:stale += $Name
            }
        }
        return
    }
    if (-not $Fresh -and $before -and $before.state -eq $state -and $before.exit -eq "0") {
        Write-Host "==> ${Name}: passed on this tree at $($before.when), not run again"
        $exits[$Name] = 0
        return
    }
    $log = Join-Path $Out "$Name.log"
    Write-Host "==> ${Name} (log: $log)"
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $exit = 0
    try {
        & $Commands *> $log
    } catch {
        "$_" | Add-Content -LiteralPath $log
        $exit = 1
    }
    $exits[$Name] = $exit
    Write-Host ("    {0} in {1:N0} s" -f $(if ($exit -eq 0) { "passed" } else { "FAILED" }), $watch.Elapsed.TotalSeconds)
    $script:progress = @($progress | Where-Object { $_.step -ne $Name }) + [pscustomobject]@{
        step  = $Name
        state = $state
        exit  = $exit
        when  = (Get-Date).ToString("yyyy-MM-dd HH:mm")
    }
    $progress | Export-Csv -LiteralPath $progressFile -NoTypeInformation
}

# One of the project's scripts in its own PowerShell, failing the step when it
# fails.
function Invoke-Tool([string]$Script, [string[]]$Arguments) {
    & $pwsh -NoProfile -File (Join-Path $PSScriptRoot $Script) @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Script $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
}

$compareOut = Join-Path $Out "compare"
$movedOut = Join-Path $Out "moved"
$benchOut = Join-Path $Out "bench"
$seedList = $Seeds -join ","

Invoke-Step "verify" "" { Invoke-Tool "verify.ps1" @("-SkipSweep") }
Invoke-Step "compare" "$refSha $Blows $seedList $Fights $SideFights" {
    Invoke-Tool "compare.ps1" @("-Ref", $refSha, "-Sweep", "-Fights", $Fights, "-SideFights", $SideFights, "-Random", $Blows, "-RandomSeeds", $seedList, "-Out", $compareOut)
}
Invoke-Step "moved" "" {
    Invoke-Cargo -Arguments @("build", "--release", "--bin", "strike_scenarios") -Label "Build the scenario tool (Release)"
    New-Item -ItemType Directory -Force $movedOut | Out-Null
    & (Join-Path $repoRoot "target\release\strike_scenarios.exe") (Join-Path $movedOut "strike_scenarios.csv") --moved
    if ($LASTEXITCODE -ne 0) {
        throw "the moved-body tests failed with exit code $LASTEXITCODE"
    }
}
Invoke-Step "parity" "" { Invoke-Tool "parity.ps1" @() }
Invoke-Step "bench" $refSha { Invoke-Tool "bench.ps1" @("-Ref", $refSha, "-Out", $benchOut) }
Invoke-Step "web" "$fightState" {
    if ($Fight) {
        Invoke-Tool "build_web.ps1" @("-Fight", $Fight)
    } else {
        Invoke-Tool "build_web.ps1" @()
    }
    foreach ($size in "390x844", "844x390") {
        Invoke-Tool "touch_check.ps1" @("-SkipBuild", "-Size", $size)
        if ($Fight) {
            Invoke-Tool "touch_check.ps1" @("-Fight", "-Size", $size)
        }
    }
}

# The verdict. Each check is ok (passed, the same, or better), LOOK (for a
# person to judge, not blocking), WORSE or FAILED (blocking), or not run.
$rows = [System.Collections.Generic.List[object]]::new()
function Add-Row([string]$Group, [string]$Check, [string]$Verdict, [string[]]$Details) {
    $rows.Add([pscustomobject]@{ Group = $Group; Check = $Check; Verdict = $Verdict; Details = @($Details | Where-Object { $_ }) })
}
function Get-Stars([double]$P) {
    if ($P -lt 0.01) { return "**" }
    if ($P -lt 0.05) { return "*" }
    return ""
}
# A comparison's findings, or nothing when the compare step did not finish;
# the comma keeps the rows one array however many there are.
function Read-Findings([string]$Name) {
    $path = Join-Path $compareOut "${Name}_diff.csv"
    if ($exits["compare"] -ne 0 -or -not (Test-Path -LiteralPath $path)) {
        return $null
    }
    return , @(Import-Csv -LiteralPath $path)
}
# A finding in words: "a whole bone outside the skin: 55 -> 86 of 6000 **",
# with the tool by tool line random_diff.txt gives it.
function Show-Finding($Finding, [string]$ReportFile = "") {
    $counts = if ($Finding.a_of -eq $Finding.b_of) {
        "$($Finding.a) -> $($Finding.b) of $($Finding.a_of)"
    } else {
        "$($Finding.a)/$($Finding.a_of) -> $($Finding.b)/$($Finding.b_of)"
    }
    $text = "$($Finding.item): $counts $(Get-Stars ([double]$Finding.p))".TrimEnd()
    if ($ReportFile -and (Test-Path -LiteralPath $ReportFile)) {
        $lines = @(Get-Content -LiteralPath $ReportFile)
        $heading = "\b" + [regex]::Escape($Finding.item) + ": in "
        for ($at = 0; $at + 1 -lt $lines.Count; $at++) {
            if ($lines[$at] -match $heading) {
                if ($lines[$at + 1] -match "^      \S") {
                    $text += " ($($lines[$at + 1].Trim()))"
                }
                break
            }
        }
    }
    return $text
}
# A check from the findings $Where picks out of $All: WORSE when any got worse
# beyond chance, else the same or better, with the borderline real-life checks
# listed.
function Add-Findings([string]$Group, [string]$Check, $All, [scriptblock]$Where, [string]$ReportFile = "", [string]$Compared = "") {
    if ($null -eq $All) {
        Add-Row $Group $Check "not run" @("the compare step did not finish")
        return
    }
    $Findings = @($All | Where-Object $Where)
    $worse = @($Findings | Where-Object { $_.change -eq "worse" })
    $better = @($Findings | Where-Object { $_.change -eq "better" })
    $edge = @($Findings | Where-Object { $_.change -eq "borderline" })
    $moved = @($Findings | Where-Object { $_.change -eq "moved" })
    $details = @(
        $worse | ForEach-Object { "worse:  $(Show-Finding $_ $ReportFile)" }
        $better | ForEach-Object { "better: $(Show-Finding $_ $ReportFile)" }
        $edge | ForEach-Object { "borderline, reported: $(Show-Finding $_)" }
        $moved | Select-Object -First 5 | ForEach-Object { "moved:  $(Show-Finding $_ $ReportFile)" }
        if ($moved.Count -gt 5) { "and $($moved.Count - 5) more moved" }
    )
    $verdict = if ($worse) { "WORSE" } elseif ($better) { "better" } else { "same" }
    if ($Compared) {
        $count = if ($Findings.Count) { "$Compared compared: $($Findings.Count)" } else { "no $Compared on either side" }
        $details = @($count) + $details
    }
    Add-Row $Group $Check $verdict $details
}
# The bands across the sweep: a line a scenario whose runs in band moved
# beyond chance, the bands that moved under it in brackets, and a band that
# got worse in a scenario whose runs in band did not move.
function Add-Bands([string]$Check, $All) {
    if ($null -eq $All) {
        Add-Row $must $Check "not run" @("the compare step did not finish")
        return
    }
    $scenarios = @($All | Where-Object { $_.holds -eq "must" -and $_.section -eq "in band" })
    $bands = @($All | Where-Object { $_.holds -eq "must" -and $_.section -eq "out of band" })
    $moved = @($scenarios | Where-Object { $_.change -in "worse", "better" })
    $details = @("scenarios compared: $($scenarios.Count)")
    foreach ($row in $moved) {
        $under = @($bands | Where-Object { $_.item.StartsWith("$($row.item): ") } | ForEach-Object {
                "$($_.item.Substring($row.item.Length + 2)) $($_.a) -> $($_.b) $(Get-Stars ([double]$_.p))".TrimEnd()
            })
        $line = "$($row.change): $($row.item) in band $($row.a) -> $($row.b) of $($row.a_of) $(Get-Stars ([double]$row.p))".TrimEnd()
        if ($under) {
            $line += " (out of band on $($under -join ', '))"
        }
        $details += $line
    }
    $alone = @($bands | Where-Object {
            $scenario = $_.item.Split(": ")[0]
            $_.change -eq "worse" -and -not ($moved | Where-Object { $_.item -eq $scenario })
        })
    $details += @($alone | ForEach-Object { "worse:  $(Show-Finding $_)" })
    $verdict = if (@($moved | Where-Object { $_.change -eq "worse" }) -or $alone) {
        "WORSE"
    } elseif ($moved) {
        "better"
    } else {
        "same"
    }
    Add-Row $must $Check $verdict $details
}
# A check a step's exit decides; a step that may drift only reports, so its
# finishing is "reported", not "passed".
function Add-Step([string]$Group, [string]$Check, [string]$Step, [string[]]$Details) {
    $ran = if ($Group -eq $must) { "passed" } else { "reported" }
    $verdict = if (-not $exits.ContainsKey($Step)) { "not run" } elseif ($exits[$Step] -eq 0) { $ran } else { "FAILED" }
    if ($verdict -eq "FAILED") {
        $Details = @("see $(Join-Path $Out "$Step.log")") + $Details
    }
    Add-Row $Group $Check $verdict $Details
}
function Get-Lines([string]$Path, [string]$Pattern) {
    if (Test-Path -LiteralPath $Path) {
        return @(Get-Content -LiteralPath $Path | Where-Object { $_ -match $Pattern } | ForEach-Object { $_.Trim() })
    }
    return @()
}

$must = "Must hold"
$drift = "May drift, reported"
$sweep = Read-Findings "sweep"
$blowFindings = Read-Findings "random"
$fightRows = Read-Findings "fights"
$sideRows = Read-Findings "side_fights"
$randomReport = Join-Path $compareOut "random_diff.txt"

Add-Step $must "The tests and verify.ps1" "verify"
Add-Step $must "The browser build simulates as the native one" "parity" (Get-Lines (Join-Path $repoRoot "output\parity\report.txt") "scenarios match exactly")
$phones = Get-Lines (Join-Path $Out "web.log") "^Touch on a "
Add-Step $must "The browser still runs smoothly (phone checks)" "web" ($phones | ForEach-Object { $_.TrimEnd(":") })
$slow = @($phones | Where-Object { $_ -match ", (\d+) fps" -and [int]$Matches[1] -lt 60 })
if ($slow -and $exits["web"] -eq 0) {
    $rows[$rows.Count - 1].Verdict = "LOOK"
    $rows[$rows.Count - 1].Details += "under 60 frames a second: is the machine busy, or did the change slow a frame past 16.7 ms?"
}
Add-Bands "The scenarios' bands across the sweep" $sweep
Add-Findings $must "The real-life checks across the sweep" $sweep { $_.holds -eq "must" -and $_.section -eq "real-life check" } "" "checks"
Add-Findings $must "The watchdogs for a body that breaks, in the sweep" $sweep { $_.holds -eq "must" -and $_.section -eq "watchdog" } "" "watchdogs"
Add-Findings $must "The watchdogs for a body that breaks, in random blows" $blowFindings { $_.holds -eq "must" -and $_.section -like "Watchdogs*" } $randomReport "watchdogs"
Add-Findings $must "The rules every play follows, in random blows" $blowFindings { $_.holds -eq "must" -and $_.section -like "The rules*" } $randomReport "rules"
$sideCheck = "Side-view fights: joints, bones, and falls"
if ($exits["compare"] -eq 0 -and $null -eq $sideRows) {
    Add-Row $must $sideCheck "not compared" @("$Ref has no strike_scenarios --side-fights; compare against a commit that has it (v0.18 on)")
} else {
    Add-Findings $must $sideCheck $sideRows { $_.holds -eq "must" } (Join-Path $compareOut "side_fights_diff.txt") "counts"
}

Add-Findings $drift "The other watchdogs, in the sweep" $sweep { $_.holds -eq "drift" -and $_.section -eq "watchdog" } "" "watchdogs"
Add-Findings $drift "The other watchdogs and flung flesh, in random blows" $blowFindings { $_.holds -eq "drift" -and $_.section -ne "Injuries" } $randomReport "watchdogs and rules"
Add-Findings $drift "The injuries random blows did" $blowFindings { $_.section -eq "Injuries" } $randomReport "kinds of injury"
if ($null -ne $sweep) {
    $medians = @($sweep | Where-Object { $_.section -eq "median number" })
    Add-Row $drift "The sweep's median numbers" $(if ($medians) { "moved" } else { "same" }) @(
        $medians | Select-Object -First 5 | ForEach-Object { "$($_.item) $(Get-Stars ([double]$_.p))" }
        if ($medians.Count -gt 5) { "and $($medians.Count - 5) more; see $(Join-Path $compareOut 'sweep_diff.txt')" }
    )
} else {
    Add-Row $drift "The sweep's median numbers" "not run" @()
}
Add-Findings $drift "Random fights, fight against fight" $fightRows { $_.holds -eq "drift" } "" "counts"
if ($null -ne $sideRows) {
    Add-Findings $drift "Side-view fights: torn fibres, knockouts, and poses" $sideRows { $_.holds -eq "drift" } (Join-Path $compareOut "side_fights_diff.txt") "counts"
}
Add-Step $drift "The moved-body tests" "moved" (Get-Lines (Join-Path $movedOut "strike_moved_report.txt") "scenarios differ under 1%")
Add-Step $drift "Speed" "bench" @(
    Get-Lines (Join-Path $benchOut "report.txt") "^Speedup"
    Get-Lines (Join-Path $repoRoot "output\parity\report.txt") "^Step time"
)

$verdict = [System.Collections.Generic.List[string]]::new()
if ($stale) {
    $verdict.Add("The last round's verdict, read again: $($stale -join ', ') ran on another tree, commit, or options than now.")
}
$verdict.Add("Merge check: the working tree ($((& git -C $repoRoot rev-parse --short HEAD).Trim())$(if ((& git -C $repoRoot status --porcelain) -ne $null) { ' with changes' })) against $Ref ($refShort), $(Get-Date -Format 'yyyy-MM-dd HH:mm')")
$verdict.Add("Random blows: $Blows from each of seeds $($Seeds -join ', '); random fights: $Fights; random side-view fights: $SideFights. * a difference chance gives under 5% of the time, ** under 1%.")
foreach ($group in $must, $drift) {
    $verdict.Add("")
    $verdict.Add("${group}:")
    foreach ($row in $rows | Where-Object { $_.Group -eq $group }) {
        $verdict.Add(("  {0,-56} {1}" -f $row.Check, $row.Verdict))
        foreach ($detail in $row.Details) {
            $verdict.Add("      $detail")
        }
    }
}
$blocking = @($rows | Where-Object { $_.Group -eq $must -and $_.Verdict -in "WORSE", "FAILED", "not run" })
$looks = @($rows | Where-Object { $_.Group -eq $must -and $_.Verdict -eq "LOOK" })
$verdict.Add("")
if ($blocking) {
    $verdict.Add("Not ready by the good-enough rule: $(($blocking | ForEach-Object { $_.Check.ToLower() + ' ' + $_.Verdict.ToLower() }) -join '; '). Fix what got worse before asking to merge, or check in if no fix is in sight; with many things compared, a lone * may be chance, so play it again with other seeds before chasing it.")
} elseif ($looks) {
    $verdict.Add("Ready by the good-enough rule, with $($looks.Count) thing(s) to look at above.")
} else {
    $verdict.Add("Ready by the good-enough rule: every check that must hold passed or is no worse beyond chance.")
}
$reportPath = Join-Path $Out "report.txt"
$verdict | Set-Content -LiteralPath $reportPath
Write-Host ""
$verdict | ForEach-Object { Write-Host $_ }
Write-Host ""
Write-Host "Report: $reportPath"
if ($blocking) {
    exit 1
}
