param(
    [switch]$StopRunningApp
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
Stop-RunningAppIfRequested -StopRunningApp:$StopRunningApp

Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics") -Label "Build Rust app (Release)"
Copy-RustAppToRepoRoot

Write-Host ""
Write-Host "Built $repoRoot\realistic_physics.exe"
