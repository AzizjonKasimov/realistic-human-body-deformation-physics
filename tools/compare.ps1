<#
.SYNOPSIS
Shows how a change moves the simulation: plays the strike scenarios and the
visual damage and anatomy diagnostics on the working tree and on a commit
(default HEAD, so it shows what uncommitted changes do) and lists what changed.

.DESCRIPTION
The commit is built in a git worktree under target\compare, and its results
are cached by commit, so later runs against the same commit only replay the
working tree. Results and report.txt go to output\compare.

.EXAMPLE
.\tools\compare.ps1
Working tree against HEAD.

.EXAMPLE
.\tools\compare.ps1 -Ref HEAD~1 -Sweep
Also compare how often each scenario stays in band when its swing moves a
little (both sides need strike_scenarios --sweep).

.EXAMPLE
.\tools\compare.ps1 -Capture torso_heavy_high
Also save app screenshots of that scenario from both sides in one contact
sheet (both sides need the app's --capture mode).

.EXAMPLE
.\tools\compare.ps1 -Clean
Remove the cached commit worktrees and their results.
#>
param(
    [string]$Ref = "HEAD",
    [switch]$Sweep,
    [string]$Capture = "",
    [string]$Out = "",
    [switch]$Clean
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
$compareRoot = Join-Path $repoRoot "target\compare"
if ($Clean) {
    if (Test-Path -LiteralPath $compareRoot) {
        Get-ChildItem -LiteralPath $compareRoot -Directory |
            Where-Object { $_.Name -match "^[0-9a-f]{10}$" } |
            ForEach-Object { & git -C $repoRoot worktree remove --force $_.FullName }
        $results = Join-Path $compareRoot "results"
        if (Test-Path -LiteralPath $results) {
            Remove-Item -LiteralPath $results -Recurse -Force
        }
    }
    & git -C $repoRoot worktree prune
    Write-Host "Removed the compare worktrees and cached results; the shared build cache in target\compare\target stays."
    return
}
$sha = Resolve-Commit $Ref
$short = $sha.Substring(0, 10)
$baseTarget = Join-Path $compareRoot "target"
$baseResults = Join-Path $compareRoot "results\$short"
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\compare"
}
$currentResults = Join-Path $Out "current"
New-Item -ItemType Directory -Force $Out, $currentResults, $baseResults | Out-Null

function Test-SourceHas([string]$Tree, [string]$RelativePath, [string]$Text) {
    $path = Join-Path $Tree $RelativePath
    return (Test-Path -LiteralPath $path) -and (Select-String -LiteralPath $path -SimpleMatch $Text -Quiet)
}

# Plays every diagnostic from one tree's release build into $Results.
function Invoke-Diagnostics([string]$Tree, [string]$TargetDir, [string]$Results, [string]$Label, [bool]$WithSweep) {
    $cargo = Get-CargoPath
    Invoke-Checked -Label "Build $Label (Release)" -Command {
        Push-Location $Tree
        try {
            & $cargo build --release --bins --target-dir $TargetDir
        } finally {
            Pop-Location
        }
    }
    $bin = Join-Path $TargetDir "release"
    Invoke-Checked -Label "Strike scenarios ($Label)" -Command {
        & (Join-Path $bin "strike_scenarios.exe") (Join-Path $Results "strike_scenarios.csv")
    }
    # The visual diagnostic exits nonzero when its gates fail; that is a result too.
    $visual = & (Join-Path $bin "visual_damage_diagnostics.exe") (Join-Path $Results "damage_visual_debug.svg") 2>&1
    $visual | Set-Content -LiteralPath (Join-Path $Results "visual.txt")
    $anatomy = & (Join-Path $bin "anatomy_diagnostics.exe") (Join-Path $Results "anatomy_debug.svg") 2>&1
    $anatomy | Set-Content -LiteralPath (Join-Path $Results "anatomy.txt")
    if ($WithSweep) {
        Invoke-Checked -Label "Strike sweep ($Label)" -Command {
            & (Join-Path $bin "strike_scenarios.exe") (Join-Path $Results "strike_scenarios.csv") --sweep
        }
    }
}

# The commit's side, built once in a worktree and cached by commit.
$baseTree = Get-CommitWorktree $sha
$baseSweeps = $Sweep -and (Test-SourceHas $baseTree "src\bin\strike_scenarios.rs" "--sweep")
$baseDone = Join-Path $baseResults "done.txt"
$haveBase = (Test-Path -LiteralPath $baseDone) -and
    ((-not $baseSweeps) -or (Test-Path -LiteralPath (Join-Path $baseResults "strike_sweep.csv")))
if ($haveBase) {
    Write-Host "Using cached results for $short"
} else {
    Invoke-Diagnostics -Tree $baseTree -TargetDir $baseTarget -Results $baseResults -Label "$Ref ($short)" -WithSweep $baseSweeps
    Set-Content -LiteralPath $baseDone -Value $sha
}
Invoke-Diagnostics -Tree $repoRoot -TargetDir (Join-Path $repoRoot "target") -Results $currentResults -Label "working tree" -WithSweep $Sweep
Copy-Item -Recurse -Force (Join-Path $baseResults "*") (New-Item -ItemType Directory -Force (Join-Path $Out "base"))

$report = [System.Collections.Generic.List[string]]::new()
$console = [System.Collections.Generic.List[string]]::new()
function Add-Line([string]$Text, [switch]$Headline) {
    $report.Add($Text)
    if ($Headline) {
        $console.Add($Text)
    }
}

# Changed values in two CSVs with one row per key; $Headline columns also go to the console.
function Compare-Rows([string]$Title, [string]$BasePath, [string]$CurrentPath, [string]$Key, [string[]]$Skip, [string[]]$Headline) {
    $base = @(Import-Csv -LiteralPath $BasePath)
    $current = @(Import-Csv -LiteralPath $CurrentPath)
    $keys = @($base.$Key) + @($current.$Key) | Select-Object -Unique
    $changed = 0
    $lines = [System.Collections.Generic.List[object]]::new()
    foreach ($name in $keys) {
        $b = $base | Where-Object { $_.$Key -eq $name } | Select-Object -First 1
        $c = $current | Where-Object { $_.$Key -eq $name } | Select-Object -First 1
        if (-not $b -or -not $c) {
            $lines.Add(@("  ${name}: only in $(if ($b) { 'base' } else { 'working tree' })", $true))
            $changed++
            continue
        }
        $differences = foreach ($column in $c.PSObject.Properties.Name) {
            if ($Skip -contains $column -or $b.$column -eq $c.$column) {
                continue
            }
            [pscustomobject]@{ Column = $column; Base = $b.$column; Current = $c.$column }
        }
        if (-not $differences) {
            continue
        }
        $changed++
        $lines.Add(@("  $name", $true))
        foreach ($difference in $differences) {
            $lines.Add(@(("    {0,-34} {1,12} -> {2}" -f $difference.Column, $difference.Base, $difference.Current), ($Headline -contains $difference.Column)))
        }
    }
    Add-Line "$Title`: $changed of $($keys.Count) changed" -Headline
    foreach ($line in $lines) {
        Add-Line $line[0] -Headline:$line[1]
    }
}

Add-Line "Comparing the working tree with $Ref ($short)" -Headline
Add-Line "" -Headline

$strikeHeadline = @(
    "bone_fractures", "rib_fractures", "skin_tears", "muscle_tears", "muscle_fiber_tears",
    "contusion_events", "skin_flap_detachments", "vessel_lacerations", "organ_penetrations",
    "max_organ_damage", "cavity_pressure_events", "wound_reopens", "blood_loss", "final_free_fragments"
)
Compare-Rows -Title "Strike scenarios" -BasePath (Join-Path $baseResults "strike_summary.csv") -CurrentPath (Join-Path $currentResults "strike_summary.csv") -Key "scenario" -Skip @("scenario", "region", "intent", "tool") -Headline $strikeHeadline

$baseWarnings = @(Get-Content -LiteralPath (Join-Path $baseResults "strike_tuning_report.txt") | Where-Object { $_ -notmatch "^All strike" })
$currentWarnings = @(Get-Content -LiteralPath (Join-Path $currentResults "strike_tuning_report.txt") | Where-Object { $_ -notmatch "^All strike" })
Add-Line "" -Headline
Add-Line "Tuning warnings: $($baseWarnings.Count) -> $($currentWarnings.Count)" -Headline
foreach ($warning in $currentWarnings | Where-Object { $baseWarnings -notcontains $_ }) {
    Add-Line "  new:  $warning" -Headline
}
foreach ($warning in $baseWarnings | Where-Object { $currentWarnings -notcontains $_ }) {
    Add-Line "  gone: $warning" -Headline
}

Add-Line "" -Headline
$visualHeadline = @(
    "skin_wound_edges", "incision_segments", "exposed_muscle_triangles", "visible_contusions",
    "visible_fluid_particles", "lacerated_vessels", "fractured_bones", "rib_fractures",
    "fracture_caps", "stats_organ_penetrations", "damage_primitives"
)
Compare-Rows -Title "Visual damage captures" -BasePath (Join-Path $baseResults "damage_visual_summary.csv") -CurrentPath (Join-Path $currentResults "damage_visual_summary.csv") -Key "scenario" -Skip @("scenario", "intent", "tool") -Headline $visualHeadline
foreach ($side in @(@("base", $baseResults), @("working tree", $currentResults))) {
    $failures = @(Get-Content -LiteralPath (Join-Path $side[1] "visual.txt") | Where-Object { $_ -match "WARN:" })
    if ($failures) {
        Add-Line "  visual gates failing on $($side[0]):" -Headline
        foreach ($failure in $failures) {
            Add-Line "    $($failure.Trim())" -Headline
        }
    }
}

# "name=value" pairs from the anatomy diagnostic's output.
function Read-Pairs([string]$Path) {
    $pairs = [ordered]@{}
    foreach ($match in (Get-Content -LiteralPath $Path | Select-String -Pattern "(\w+)=(\S+)" -AllMatches).Matches) {
        $pairs[$match.Groups[1].Value] = $match.Groups[2].Value
    }
    return $pairs
}
$baseAnatomy = Read-Pairs (Join-Path $baseResults "anatomy.txt")
$currentAnatomy = Read-Pairs (Join-Path $currentResults "anatomy.txt")
$anatomyChanges = foreach ($name in $currentAnatomy.Keys) {
    if ($baseAnatomy[$name] -ne $currentAnatomy[$name]) {
        "$name $($baseAnatomy[$name]) -> $($currentAnatomy[$name])"
    }
}
Add-Line "" -Headline
Add-Line ("Anatomy: " + $(if ($anatomyChanges) { $anatomyChanges -join ", " } else { "unchanged" })) -Headline

if ($Sweep) {
    Add-Line "" -Headline
    if (-not $baseSweeps) {
        Add-Line "Sweep: $short has no strike_scenarios --sweep; only the working tree's report is in $currentResults" -Headline
    } else {
        Add-Line "Sweep (runs in band, then medians base -> working tree):" -Headline
        $baseRuns = @(Import-Csv -LiteralPath (Join-Path $baseResults "strike_sweep.csv"))
        $currentRuns = @(Import-Csv -LiteralPath (Join-Path $currentResults "strike_sweep.csv"))
        $sweepMetrics = @("bone_fractures", "rib_fractures", "skin_tears", "contusion_events", "vessel_lacerations", "wound_reopens")
        function Get-Median($Rows, [string]$Column) {
            $values = @($Rows | ForEach-Object { [double]$_.$Column } | Sort-Object)
            if (-not $values) { return "-" }
            return $values[[int][Math]::Floor($values.Count / 2)]
        }
        foreach ($name in (@($baseRuns.scenario) + @($currentRuns.scenario) | Select-Object -Unique)) {
            $b = @($baseRuns | Where-Object { $_.scenario -eq $name })
            $c = @($currentRuns | Where-Object { $_.scenario -eq $name })
            $bIn = @($b | Where-Object { $_.in_band -eq "1" }).Count
            $cIn = @($c | Where-Object { $_.in_band -eq "1" }).Count
            $medians = foreach ($metric in $sweepMetrics) {
                $bm = Get-Median $b $metric
                $cm = Get-Median $c $metric
                if ($bm -ne $cm) { "$metric $bm -> $cm" }
            }
            Add-Line ("  {0,-30} {1}/{2} -> {3}/{4}  {5}" -f $name, $bIn, $b.Count, $cIn, $c.Count, ($medians -join ", ")) -Headline
        }
    }
}

if ($Capture) {
    Add-Line "" -Headline
    $baseCaptures = Test-SourceHas $baseTree "src\bin\realistic_physics\capture.rs" "--capture"
    if (-not $baseCaptures) {
        Add-Line "Capture: $short has no capture mode; only the working tree can be captured" -Headline
    } else {
        $captureDir = Join-Path $Out "captures"
        & "$PSScriptRoot\capture.ps1" -Scenario $Capture -Name "base-$Capture" -Out $captureDir -Exe (Join-Path $baseTarget "release\realistic_physics.exe") | Out-Null
        & "$PSScriptRoot\capture.ps1" -Scenario $Capture -Name "current-$Capture" -Out $captureDir -NoBuild | Out-Null
        $images = @("base-$Capture-1280x720-normal", "current-$Capture-1280x720-normal", "base-$Capture-1280x720-anatomy", "current-$Capture-1280x720-anatomy") |
            ForEach-Object { Join-Path $captureDir "$_.png" }
        $sheet = Join-Path $Out "$Capture-base-vs-current.png"
        Invoke-Checked -Label "Contact sheet" -Command { & (Join-Path $repoRoot "target\release\contact_sheet.exe") $sheet --height 480 --columns 2 @images }
        Add-Line "Capture: $sheet (left: $short, right: working tree)" -Headline
    }
}

$reportPath = Join-Path $Out "report.txt"
$report | Set-Content -LiteralPath $reportPath
Write-Host ""
$console | ForEach-Object { Write-Host $_ }
Write-Host ""
Write-Host "Full report: $reportPath"
