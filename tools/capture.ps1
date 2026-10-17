<#
.SYNOPSIS
Saves screenshots of the real app, optionally after a scripted strike or
gesture, at one or more window sizes, plus a contact sheet that shows them all
in one image.

.DESCRIPTION
Each capture briefly opens the app window, plays the strike or gesture at fixed
steps, saves the screen, and closes. With -Every N it also saves the screen
every N steps on the way, as a film strip of how things move. Output goes to
output\captures by default.

The app simulates the body at the scenarios' size in any window and only draws
it larger or smaller, so a play must hurt it alike at every size: with more
than one size, this also checks that each capture's step-by-step log
(NAME-SIZE-events.csv) and the digest of the world it left are the same, and
fails, naming the first step that differs, when they are not.

.EXAMPLE
.\tools\capture.ps1
Rest pose, normal and anatomy views, 1280x720.

.EXAMPLE
.\tools\capture.ps1 -Scenario torso_heavy_high
A tuned scenario (strike_scenarios --list shows them).

.EXAMPLE
.\tools\capture.ps1 -Strike "hammer:-0.26,0.34:0.30,0.34:frames=14" -Sizes 800x600,1280x720,390x844
A custom swing in body coordinates at three window sizes.

.EXAMPLE
.\tools\capture.ps1 -Gesture "bat:-0.3,0.3:wait=10:down:-0.08,0.3/15:-0.08,0.6/60:up:wait=20" -Every 5 -View normal
A gesture played the way the app is, saved every 5 steps as a film strip.
#>
param(
    [string]$Scenario = "",
    [string]$Strike = "",
    [string]$Gesture = "",
    # One run of the strike sweep, ALONG,ACROSS,STRENGTH (strike_scenarios --variant).
    [string]$Variant = "",
    # Comma-separated window sizes.
    [string]$Sizes = "1280x720",
    [ValidateSet("normal", "anatomy", "both")]
    [string]$View = "both",
    # Steps to play before saving; 0 plays the whole strike, or lets the body settle.
    [int]$Frames = 0,
    # Also save the screen every this many steps, for a film strip.
    [int]$Every = 0,
    # Leave out the HUD, control buttons, and pointer ring.
    [switch]$NoUi,
    # Leave out the tool, to see what lies under it.
    [switch]$NoTool,
    # Leave out the name and step label, for images to publish.
    [switch]$NoLabel,
    [string]$Out = "",
    # File name stem; defaults to the scenario name, "strike", "gesture", or "rest".
    [string]$Name = "",
    # App to run instead of building and using target\release\realistic_physics.exe.
    [string]$Exe = "",
    [switch]$NoBuild
)

. "$PSScriptRoot\common.ps1"

$repoRoot = Get-RepoRoot
if (-not $Out) {
    $Out = Join-Path $repoRoot "output\captures"
}
New-Item -ItemType Directory -Force $Out | Out-Null
if (-not $Name) {
    $Name = if ($Scenario) { $Scenario } elseif ($Strike) { "strike" } elseif ($Gesture) { "gesture" } else { "rest" }
}
if (-not $Exe) {
    if (-not $NoBuild) {
        Invoke-Cargo -Arguments @("build", "--release", "--bin", "realistic_physics", "--bin", "contact_sheet") -Label "Build app and contact sheet (Release)"
    }
    $Exe = Join-Path $repoRoot "target\release\realistic_physics.exe"
}
$sheetExe = Join-Path $repoRoot "target\release\contact_sheet.exe"

$images = @()
# Per size: the world digest the capture printed and its step-by-step log.
$runs = @()
foreach ($size in ($Sizes -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ })) {
    $stem = "$Name-$size"
    $path = Join-Path $Out "$stem.png"
    # A film strip's frames are STEM-NNNN.png; clear an earlier strip's first.
    $stripPattern = "^$([regex]::Escape($stem))-\d{4}(-normal|-anatomy)?\.png$"
    Get-ChildItem -LiteralPath $Out -File | Where-Object { $_.Name -match $stripPattern } | Remove-Item
    $events = Join-Path $Out "$stem-events.csv"
    $arguments = @("--capture", $path, "--view", $View, "--size", $size, "--label", "$Name $size", "--events", $events)
    if ($Scenario) { $arguments += @("--scenario", $Scenario) }
    if ($Strike) { $arguments += @("--strike", $Strike) }
    if ($Gesture) { $arguments += @("--gesture", $Gesture) }
    if ($Variant) { $arguments += @("--variant", $Variant) }
    if ($Frames -gt 0) { $arguments += @("--frames", "$Frames") }
    if ($Every -gt 0) { $arguments += @("--every", "$Every") }
    if ($NoUi) { $arguments += "--no-ui" }
    if ($NoTool) { $arguments += "--no-tool" }
    if ($NoLabel) { $arguments += "--no-label" }
    $script:printed = @()
    Invoke-Checked -Label "Capture $Name at $size" -Command {
        $script:printed = & $Exe @arguments 2>&1
        $script:printed
    }
    $digest = $script:printed | ForEach-Object { "$_" } |
        Where-Object { $_ -match "^world digest (\S+)" } |
        ForEach-Object { $Matches[1] } |
        Select-Object -Last 1
    $runs += [pscustomobject]@{ Size = $size; Digest = $digest; Events = $events }
    if ($Every -gt 0) {
        $images += Get-ChildItem -LiteralPath $Out -File |
            Where-Object { $_.Name -match $stripPattern } |
            Sort-Object Name |
            ForEach-Object { $_.FullName }
    } elseif ($View -eq "both") {
        $images += Join-Path $Out "$stem-normal.png"
        $images += Join-Path $Out "$stem-anatomy.png"
    } else {
        $images += $path
    }
}

if ($images.Count -gt 1 -and (Test-Path $sheetExe)) {
    $columns = if ($Every -gt 0) {
        if ($View -eq "both") { 4 } else { 5 }
    } elseif ($View -eq "both") { 2 } else { [Math]::Min($images.Count, 3) }
    $height = if ($Every -gt 0) { 300 } else { 480 }
    $sheet = Join-Path $Out "$Name-sheet.png"
    Invoke-Checked -Label "Contact sheet" -Command { & $sheetExe $sheet --height $height --columns $columns @images }
}

# The window only changes how the world is drawn, so every size must have
# simulated the same play step for step. Builds from before v0.10.2 sized the
# body to the window and print no digest, so theirs are not compared.
if ($runs.Count -gt 1 -and ($runs | Where-Object { -not $_.Digest })) {
    Write-Host ""
    Write-Host "This build prints no world digest (older than v0.10.2), so the sizes' physics was not compared."
} elseif ($runs.Count -gt 1) {
    $first = $runs[0]
    $firstLog = Get-Content -LiteralPath $first.Events
    $differ = @()
    foreach ($run in $runs | Select-Object -Skip 1) {
        $log = Get-Content -LiteralPath $run.Events
        $step = 0
        while ($step -lt [Math]::Max($firstLog.Count, $log.Count) -and $firstLog[$step] -eq $log[$step]) {
            $step++
        }
        if ($run.Digest -ne $first.Digest -or $step -lt [Math]::Max($firstLog.Count, $log.Count)) {
            $differ += $run
            Write-Host ("{0} differs from {1}: world digest {2} against {3}" -f $run.Size, $first.Size, $run.Digest, $first.Digest)
            if ($step -lt [Math]::Max($firstLog.Count, $log.Count)) {
                Write-Host "  first different line of the step log ($($firstLog[0])):"
                Write-Host "  $($first.Size): $($firstLog[$step])"
                Write-Host "  $($run.Size): $($log[$step])"
            }
        }
    }
    if ($differ.Count -gt 0) {
        throw "The same play hurt the body differently in different window sizes; the app's physics must not depend on the window."
    }
    Write-Host ""
    Write-Host "Same physics at every size ($(($runs | ForEach-Object { $_.Size }) -join ', ')): world digest $($first.Digest), step logs identical."
}
