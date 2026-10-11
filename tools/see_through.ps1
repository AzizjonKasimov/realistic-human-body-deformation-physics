<#
.SYNOPSIS
Checks that no wound lets the background show through the body.

.DESCRIPTION
Captures the end of each tuned strike scenario, and the rest pose, in the
normal view without the HUD, labels, or tool, then counts the patches of
background each capture's body encloses (see_through.exe). A hole is
background showing through flesh nothing drawn covers; a gap between body
parts that a limb or a piece of bone closes off counts too, so look at the
capture before calling one a bug. Captures go to output\see_through. Exits 1
if any capture has a hole. It briefly opens the app window for each capture.

.EXAMPLE
.\tools\see_through.ps1
Every tuned scenario and the rest pose.

.EXAMPLE
.\tools\see_through.ps1 -Scenarios hammer_into_wound,torso_heavy_high -NoBuild
Two scenarios, with the release builds already made.
#>
param(
    [string[]]$Scenarios = @(),
    [string]$Out = "",
    [switch]$NoBuild
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\see_through"
}
New-Item -ItemType Directory -Force $Out | Out-Null
if (-not $NoBuild) {
    Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics", "--bin", "see_through", "--bin", "strike_scenarios") -Label "Build app and checks (Release)"
}
if ($Scenarios.Count -eq 0) {
    $list = & (Join-Path $repoRoot "target\release\strike_scenarios.exe") --list
    $Scenarios = $list | ForEach-Object { ($_ -split "\s+")[0] } | Where-Object { $_ }
}

Get-ChildItem -LiteralPath $Out -Filter "*.png" | Remove-Item
$capture = Join-Path $PSScriptRoot "capture.ps1"
$common = @{ NoBuild = $true; NoUi = $true; NoLabel = $true; NoTool = $true; View = "normal"; Out = $Out }
& $capture @common -Name rest | Out-Null
foreach ($name in $Scenarios) {
    & $capture @common -Scenario $name | Out-Null
}

$images = Get-ChildItem -LiteralPath $Out -Filter "*.png" | Sort-Object Name | ForEach-Object { $_.FullName }
& (Join-Path $repoRoot "target\release\see_through.exe") @images
exit $LASTEXITCODE
