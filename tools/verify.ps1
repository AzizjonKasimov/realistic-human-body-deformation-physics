<#
.SYNOPSIS
Checks formatting and tests, plays the strike scenarios and their offset
sweep, and runs the anatomy and visual damage diagnostics.

.PARAMETER SkipDiagnostics
Skip the anatomy and visual damage diagnostics.

.PARAMETER SkipSweep
Skip replaying each scenario with its swing moved slightly.

.PARAMETER Capture
Also save real app screenshots of the body at rest in a desktop, a small, and
a phone-sized window (output\captures\rest-sheet.png).

.PARAMETER BuildApp
Also rebuild the root realistic_physics.exe.

.PARAMETER StopRunningApp
Close a running realistic_physics.exe that would block -BuildApp.

.PARAMETER SoundCheck
Also check that the root realistic_physics.exe's sound reaches the speakers
(tools\sound_check.ps1; it briefly opens the app window). Runs after -BuildApp.
#>
param(
    [switch]$SkipDiagnostics,
    [switch]$SkipSweep,
    [switch]$Capture,
    [switch]$BuildApp,
    [switch]$StopRunningApp,
    [switch]$SoundCheck
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
$timings = [System.Collections.Generic.List[string]]::new()
function Measure-Step([string]$Name, [scriptblock]$Step) {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    & $Step
    $timings.Add(("{0,-30} {1,6:N1} s" -f $Name, $watch.Elapsed.TotalSeconds))
}

Measure-Step "Formatting" { Invoke-Cargo -Arguments @("fmt", "--check") -Label "Check Rust formatting" }
Measure-Step "Tests" { Invoke-Cargo -Arguments @("test") -Label "Run Rust simulation tests" }
# The patched sound backend is a separate crate, so its mixer tests run on their own.
Measure-Step "quad-snd tests" { Invoke-Cargo -Arguments @("test", "--manifest-path", "vendor\quad-snd\Cargo.toml", "--target-dir", "target\vendor") -Label "Run the patched quad-snd's tests" }
# The scenario and diagnostic binaries run in release: the same results as a
# debug build, several times faster.
Measure-Step "Strike scenarios" { Invoke-Cargo -Arguments @("run", "--release", "--bin", "strike_scenarios", "--", "output\strike_scenarios.csv") -Label "Run Rust strike scenarios" }
if (-not $SkipSweep) {
    Measure-Step "Strike sweep" { Invoke-Cargo -Arguments @("run", "--release", "--bin", "strike_scenarios", "--", "output\strike_scenarios.csv", "--sweep") -Label "Sweep strike scenarios (swing moved slightly)" }
}

if (-not $SkipDiagnostics) {
    Measure-Step "Anatomy diagnostics" { Invoke-Cargo -Arguments @("run", "--release", "--bin", "anatomy_diagnostics", "--", "output\anatomy_debug.svg") -Label "Run Rust anatomy diagnostics" }
    Measure-Step "Visual damage diagnostics" { Invoke-Cargo -Arguments @("run", "--release", "--bin", "visual_damage_diagnostics", "--", "output\damage_visual_debug.svg") -Label "Run Rust visual damage diagnostics" }
}

if ($Capture) {
    Measure-Step "Captures" { & "$PSScriptRoot\capture.ps1" -Sizes "1280x720,800x600,390x844" }
}

if ($BuildApp) {
    Stop-RunningAppIfRequested -StopRunningApp:$StopRunningApp
    Measure-Step "Build app" { Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics") -Label "Build Rust app (Release)" }
    Copy-RustAppToRepoRoot
}

if ($SoundCheck) {
    Measure-Step "Sound check" {
        Invoke-Checked -Label "Check the app's sound reaches the speakers" -Command { & pwsh -NoProfile -File "$PSScriptRoot\sound_check.ps1" }
    }
}

Write-Host ""
Write-Host "Verification complete."
$timings | ForEach-Object { Write-Host "  $_" }
Write-Host "Strike tuning report: $repoRoot\output\strike_tuning_report.txt"
if (-not $SkipSweep) {
    Write-Host "Strike sweep report: $repoRoot\output\strike_sweep_report.txt"
}
if (-not $SkipDiagnostics) {
    Write-Host "Anatomy snapshot: $repoRoot\output\anatomy_debug.svg"
    Write-Host "Damage visual snapshot: $repoRoot\output\damage_visual_debug.svg"
    Write-Host "Damage visual metrics: $repoRoot\output\damage_visual_summary.csv"
}
if ($Capture) {
    Write-Host "Rest captures: $repoRoot\output\captures\rest-sheet.png"
}
if ($BuildApp) {
    Write-Host "App executable: $repoRoot\realistic_physics.exe"
}
