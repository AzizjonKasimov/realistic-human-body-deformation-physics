<#
.SYNOPSIS
Times the simulation against a commit: plays the app's self-test (every tuned
scenario at fixed steps) with the working tree's release build and with the
commit's, taking turns, and compares their step times and injuries.

.DESCRIPTION
The commit is built in the cached worktree under target\compare that
compare.ps1 also uses, with that commit's own Cargo.toml and so its own
release profile, and its app is kept in target\compare\apps\<commit>. The two
builds take turns for -Runs rounds, so a change in the machine's load hits both
alike. The report gives each build's best and median average step and slowest
step, and the speedup on the best of each, and checks that every scenario's
injuries are the same in every run of both, as they must be when a change only
makes the simulation faster. It goes to output\bench\report.txt.

Each self-test opens the app's window for a few seconds. Other heavy work on
the machine, such as builds, shows up as noise: compare best runs, and add
runs when the two sides are close.

.EXAMPLE
.\tools\bench.ps1 -Ref main
The working tree against main, five runs each.

.EXAMPLE
.\tools\bench.ps1 -BaseExe C:\builds\before.exe -Exe C:\builds\after.exe -Runs 9
Two prebuilt apps, without building anything.
#>
param(
    [string]$Ref = "HEAD",
    [int]$Runs = 5,
    [string]$BaseExe = "",
    [string]$Exe = "",
    [string]$Out = ""
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\bench"
}
New-Item -ItemType Directory -Force $Out | Out-Null

# A prebuilt app is named by its path, relative to the repository when inside it.
function Get-ExeLabel([string]$Path) {
    $full = (Resolve-Path -LiteralPath $Path).Path
    if ($full.StartsWith($repoRoot, [StringComparison]::OrdinalIgnoreCase)) {
        return $full.Substring($repoRoot.Length).TrimStart([char]'\', [char]'/')
    }
    return $full
}

if ($BaseExe) {
    $baseLabel = Get-ExeLabel $BaseExe
} else {
    $sha = Resolve-Commit $Ref
    $short = $sha.Substring(0, 10)
    $baseLabel = "$Ref ($short)"
    $BaseExe = Join-Path $repoRoot "target\compare\apps\$short\realistic_physics.exe"
    if (-not (Test-Path -LiteralPath $BaseExe)) {
        $tree = Get-CommitWorktree $sha
        $targetDir = Join-Path $repoRoot "target\compare\target"
        $cargo = Get-CargoPath
        Invoke-Checked -Label "Build $baseLabel app (Release)" -Command {
            Push-Location $tree
            try {
                & $cargo build --release --bin realistic_physics --target-dir $targetDir
            } finally {
                Pop-Location
            }
        }
        New-Item -ItemType Directory -Force (Split-Path -Parent $BaseExe) | Out-Null
        Copy-Item -LiteralPath (Join-Path $targetDir "release\realistic_physics.exe") -Destination $BaseExe
    }
}
if ($Exe) {
    $label = Get-ExeLabel $Exe
} else {
    $label = "working tree"
    Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics") -Label "Build working tree app (Release)"
    $Exe = Join-Path $repoRoot "target\release\realistic_physics.exe"
}

# One self-test run's report.
function Invoke-SelfTest([string]$Path) {
    $lines = & $Path --selftest
    if ($LASTEXITCODE -ne 0) {
        throw "$Path --selftest failed with exit code $LASTEXITCODE"
    }
    $json = $lines | Where-Object { $_ -like "{*" } | Select-Object -Last 1
    if (-not $json) {
        throw "$Path --selftest printed no report"
    }
    return $json | ConvertFrom-Json
}

# One line per scenario with everything the self-test reports about it.
function Get-Injuries($Report) {
    foreach ($s in $Report.scenarios) {
        "{0}: in band {1}, bones {2}, ribs {3}, skin {4}, muscle {5}, bruises {6}, fluid {7}, blood loss {8}, tool turn {9}, fastest tissue {10}" -f `
            $s.name, $s.in_band, $s.bones, $s.ribs, $s.skin, $s.muscle, $s.bruises, $s.fluid, $s.blood_loss, $s.max_tool_turn, $s.fastest_tissue
    }
}

function Get-Median([double[]]$Values) {
    $sorted = @($Values | Sort-Object)
    $middle = [int][Math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2) {
        return $sorted[$middle]
    }
    return ($sorted[$middle - 1] + $sorted[$middle]) / 2
}

$sides = @(
    [pscustomobject]@{ Label = $baseLabel; Exe = $BaseExe; Runs = [System.Collections.Generic.List[object]]::new() },
    [pscustomobject]@{ Label = $label; Exe = $Exe; Runs = [System.Collections.Generic.List[object]]::new() }
)
Write-Host ""
Write-Host "==> Self-test, $Runs runs each, taking turns"
for ($run = 1; $run -le $Runs; $run++) {
    foreach ($side in $sides) {
        $result = Invoke-SelfTest $side.Exe
        $side.Runs.Add($result)
        Write-Host ("  run {0}/{1}  {2,-44} average {3,6:N3} ms, slowest {4,6:N3} ms" -f $run, $Runs, $side.Label, $result.avg_step_ms, $result.max_step_ms)
    }
}

$report = [System.Collections.Generic.List[string]]::new()
$first = $sides[0].Runs[0]
$report.Add("Self-test of $(@($first.scenarios).Count) scenarios, $($first.steps) steps, $Runs runs each, taking turns")
$report.Add("")
$report.Add(("  {0,-44} {1,22} {2,22}" -f "", "average step, ms", "slowest step, ms"))
$report.Add(("  {0,-44} {1,22} {2,22}" -f "", "best / median", "best / median"))
$best = @{}
foreach ($side in $sides) {
    $averages = [double[]]@($side.Runs | ForEach-Object { $_.avg_step_ms })
    $slowest = [double[]]@($side.Runs | ForEach-Object { $_.max_step_ms })
    $best[$side.Label] = @(($averages | Measure-Object -Minimum).Minimum, ($slowest | Measure-Object -Minimum).Minimum)
    $report.Add(("  {0,-44} {1,22} {2,22}" -f $side.Label,
            ("{0:N3} / {1:N3}" -f $best[$side.Label][0], (Get-Median $averages)),
            ("{0:N3} / {1:N3}" -f $best[$side.Label][1], (Get-Median $slowest))))
}
$before = $best[$sides[0].Label]
$after = $best[$sides[1].Label]
$report.Add("")
$report.Add(("Speedup: {0:N2}x on the best average step, {1:N2}x on the best slowest step" -f ($before[0] / $after[0]), ($before[1] / $after[1])))

$reference = @(Get-Injuries $first)
$differences = [System.Collections.Generic.List[string]]::new()
foreach ($side in $sides) {
    for ($run = 0; $run -lt $side.Runs.Count; $run++) {
        $lines = @(Get-Injuries $side.Runs[$run])
        for ($index = 0; $index -lt [Math]::Max($lines.Count, $reference.Count); $index++) {
            if ($lines[$index] -ne $reference[$index]) {
                $differences.Add("  $($side.Label), run $($run + 1): $($lines[$index])")
                $differences.Add("    instead of $($reference[$index])")
            }
        }
    }
}
if ($differences.Count -eq 0) {
    $report.Add("Injuries: the same in every scenario of every run")
} else {
    $report.Add("Injuries differ from the first run of $($sides[0].Label):")
    $report.AddRange($differences)
}
$failed = @($sides | ForEach-Object { $_.Runs } | Where-Object { -not $_.passed }).Count
if ($failed) {
    $report.Add("Self-test failed (scenarios out of band) in $failed runs")
}

$reportPath = Join-Path $Out "report.txt"
$report | Set-Content -LiteralPath $reportPath
Write-Host ""
$report | ForEach-Object { Write-Host $_ }
Write-Host ""
Write-Host "Report: $reportPath"
