<#
.SYNOPSIS
Records one run for the Rerun viewer and opens it: every point of flesh,
fiber (coloured by how far it is stretched), bone, joint, the tool, and the
blood on a timeline of frames, with what the injury rules decided and what the
watchdogs flagged as a log, and the fists, the head, each joint's slip and
turn, each bone's load, and how far each bone's flesh has slid plotted over
the run.

.DESCRIPTION
Builds strike_scenarios with the `record` feature in its own target folder
(target\record), so the everyday builds never compile the Rerun SDK, plays the
run with --record, and opens the recording in the Rerun viewer
(`cargo install rerun-cli --version 0.38.1 --locked`, the SDK's version).
Recordings go to output\record\NAME.rrd. Two recordings opened together (the
viewer takes several files) show what a change did, frame by frame.

.PARAMETER Scenario
A tuned scenario to record (strike_scenarios --only).

.PARAMETER Strike
A custom swing (strike_scenarios --strike).

.PARAMETER Gesture
A custom gesture (strike_scenarios --gesture).

.PARAMETER Variant
One run of the sweep, ALONG,ACROSS,STRENGTH.

.PARAMETER Set
Settings changed from the defaults, "NAME=VALUE,NAME=VALUE".

.PARAMETER Try
Experiments switched on in the code (RP_TRY), comma separated.

.PARAMETER Name
The recording's name; the scenario's name, or "custom", by default.

.PARAMETER NoOpen
Only record; do not open the viewer.

.EXAMPLE
.\tools\record.ps1 -Scenario shoulder_blunt
Record the bat brought down on the shoulder and open it.

.EXAMPLE
.\tools\record.ps1 -Scenario shoulder_blunt -Set "gravity=2.867" -Name shoulder_old_gravity
The same at the old 0.29 g, to open beside the first.
#>
param(
    [string]$Scenario = "",
    [string]$Strike = "",
    [string]$Gesture = "",
    [string]$Variant = "",
    [string]$Set = "",
    [string]$Try = "",
    [string]$Name = "",
    [switch]$NoOpen
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
$targetDir = Join-Path $repoRoot "target\record"
$cargo = Get-CargoPath
Invoke-Checked -Label "Build strike_scenarios with recording (Release)" -Command {
    Push-Location $repoRoot
    try {
        & $cargo build --release --features record --bin strike_scenarios --target-dir $targetDir
    } finally {
        Pop-Location
    }
}

$arguments = @()
if ($Scenario) {
    $arguments += @("--only", $Scenario)
} elseif ($Strike) {
    $arguments += @("--strike", $Strike)
} elseif ($Gesture) {
    $arguments += @("--gesture", $Gesture)
} else {
    throw "Name a -Scenario, -Strike, or -Gesture to record"
}
if ($Variant) {
    $arguments += @("--variant", $Variant)
}
if ($Set) {
    $arguments += @("--set", $Set)
}
if (-not $Name) {
    $Name = $(if ($Scenario) { $Scenario } else { "custom" })
}
$outDir = Join-Path $repoRoot "output\record"
New-Item -ItemType Directory -Force $outDir | Out-Null
$recording = Join-Path $outDir "$Name.rrd"
$arguments = @((Join-Path $outDir "strike_scenarios.csv"), "--record", $recording, "--journal") + $arguments

$previous = $env:RP_TRY
$env:RP_TRY = $Try
try {
    & (Join-Path $targetDir "release\strike_scenarios.exe") @arguments | Select-String -Pattern "recorded|cannot|watchdog" | ForEach-Object { Write-Host $_.Line }
} finally {
    $env:RP_TRY = $previous
}

if (-not $NoOpen) {
    $viewer = Get-Command rerun -ErrorAction SilentlyContinue
    if (-not $viewer) {
        Write-Host "Install the viewer to open it: cargo install rerun-cli --version 0.38.1 --locked"
    } else {
        Start-Process -FilePath $viewer.Source -ArgumentList "`"$recording`""
    }
}
