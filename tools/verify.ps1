param(
    [switch]$SkipDiagnostics,
    [switch]$BuildApp,
    [switch]$StopRunningApp
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot

Invoke-Cargo -Arguments @("fmt", "--check") -Label "Check Rust formatting"
Invoke-Cargo -Arguments @("test") -Label "Run Rust simulation tests"
Invoke-Cargo -Arguments @("run", "--bin", "strike_scenarios", "--", "output\strike_scenarios.csv") -Label "Run Rust strike scenarios"

if (-not $SkipDiagnostics) {
    Invoke-Cargo -Arguments @("run", "--bin", "anatomy_diagnostics", "--", "output\anatomy_debug.svg") -Label "Run Rust anatomy diagnostics"
    Invoke-Cargo -Arguments @("run", "--bin", "visual_damage_diagnostics", "--", "output\damage_visual_debug.svg") -Label "Run Rust visual damage diagnostics"
}

if ($BuildApp) {
    Stop-RunningAppIfRequested -StopRunningApp:$StopRunningApp
    Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics") -Label "Build Rust app (Release)"
    Copy-RustAppToRepoRoot
}

Write-Host ""
Write-Host "Verification complete."
Write-Host "Strike tuning report: $repoRoot\output\strike_tuning_report.txt"
if (-not $SkipDiagnostics) {
    Write-Host "Anatomy snapshot: $repoRoot\output\anatomy_debug.svg"
    Write-Host "Damage visual snapshot: $repoRoot\output\damage_visual_debug.svg"
    Write-Host "Damage visual metrics: $repoRoot\output\damage_visual_summary.csv"
}
if ($BuildApp) {
    Write-Host "App executable: $repoRoot\realistic_physics.exe"
}
