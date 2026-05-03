param(
    [switch]$Configure,
    [switch]$StopRunningApp
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
Initialize-WindowsBuild -Force:$Configure
Stop-RunningAppIfRequested -StopRunningApp:$StopRunningApp
Invoke-WindowsTarget -Target "realistic_physics" -Config "Release"

Write-Host ""
Write-Host "Built $repoRoot\realistic_physics.exe"
