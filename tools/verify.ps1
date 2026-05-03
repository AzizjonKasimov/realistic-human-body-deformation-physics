param(
    [switch]$Configure,
    [switch]$SkipDiagnostics,
    [switch]$BuildApp,
    [switch]$StopRunningApp
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
Initialize-WindowsBuild -Force:$Configure

Invoke-WindowsTarget -Target "realistic_physics_tests" -Config "Debug" -CleanFirst
Invoke-ExeViaCmd `
    -ExePath (Join-Path $repoRoot "build\vs\Debug\realistic_physics_tests.exe") `
    -Label "Run simulation tests"

Invoke-WindowsTarget -Target "realistic_physics_strike_scenarios" -Config "Debug" -CleanFirst
Invoke-ExeViaCmd `
    -ExePath (Join-Path $repoRoot "build\vs\Debug\realistic_physics_strike_scenarios.exe") `
    -Arguments @("output\strike_scenarios.csv") `
    -Label "Run strike scenarios"

if (-not $SkipDiagnostics) {
    Invoke-WindowsTarget -Target "realistic_physics_diagnostics" -Config "Debug" -CleanFirst
    Invoke-ExeViaCmd `
        -ExePath (Join-Path $repoRoot "build\vs\Debug\realistic_physics_diagnostics.exe") `
        -Arguments @("output\anatomy_debug.svg") `
        -Label "Run anatomy diagnostics"
}

if ($BuildApp) {
    Stop-RunningAppIfRequested -StopRunningApp:$StopRunningApp
    Invoke-WindowsTarget -Target "realistic_physics" -Config "Release" -CleanFirst
}

Write-Host ""
Write-Host "Verification complete."
if (-not $SkipDiagnostics) {
    Write-Host "Anatomy snapshot: $repoRoot\output\anatomy_debug.svg"
}
if ($BuildApp) {
    Write-Host "App executable: $repoRoot\realistic_physics.exe"
}
