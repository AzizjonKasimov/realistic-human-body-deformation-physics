<#
.SYNOPSIS
Plays the browser build on a phone-sized screen with real touch input, in a
headless browser: a first touch on the chest, then a swipe through the body.

.DESCRIPTION
Serves target\web (building it first unless -SkipBuild) and opens it in
headless Edge (or Chrome) emulating a phone with a touch screen, taps Start,
and plays touches over the DevTools protocol, which reach the page as a
finger's do. It checks that the first touch on the chest takes the hammer up
without striking the body (as the app's first touch, which grows the buttons
to finger size, once blew the chest apart), that a swipe into the belly
then strikes it (the page counts the sledgehammer's first blow), and that the
page logs no errors. A screenshot after each goes to output\touch_check, to
see where the hammer is and what the swipe did. Exits 1 on a failure.

The headless browser keeps animation frames running at full rate, which the
app's browser pane does not while it is hidden, so this works whether or not
the pane shows.

.PARAMETER Size
The phone's screen in CSS pixels, WIDTHxHEIGHT.

.PARAMETER SkipBuild
Use the existing target\web build.

.PARAMETER Port
Port to serve the web build on while the check runs.

.PARAMETER Browser
Path to a Chromium browser; Edge or Chrome is found by default.

.EXAMPLE
.\tools\touch_check.ps1
Builds the web version and plays the touches on a 390x844 phone.

.EXAMPLE
.\tools\touch_check.ps1 -Size 844x390 -SkipBuild
The same on a phone held sideways.
#>
param(
    [string]$Size = "390x844",
    [switch]$SkipBuild,
    [int]$Port = 8093,
    [string]$Browser = ""
)

. "$PSScriptRoot\common.ps1"
. "$PSScriptRoot\headless.ps1"

$repoRoot = Get-RepoRoot
$out = Join-Path $repoRoot "output\touch_check"
New-Item -ItemType Directory -Force $out | Out-Null
$width, $height = $Size -split "[xX]" | ForEach-Object { [int]$_ }
if (-not $width -or -not $height) {
    throw "-Size should look like 390x844."
}
if (-not $SkipBuild) {
    & (Join-Path $PSScriptRoot "build_web.ps1")
}
$siteDir = Join-Path $repoRoot "target\web"
if (-not (Test-Path -LiteralPath (Join-Path $siteDir "index.html"))) {
    throw "No web build in $siteDir; run without -SkipBuild."
}

$problems = [System.Collections.Generic.List[string]]::new()
$errors = [System.Collections.Generic.List[string]]::new()

# One finger, put down, moved, or lifted at (X, Y) in CSS pixels.
function Send-Touch([string]$Type, [double]$X, [double]$Y) {
    # @() keeps a list of one finger, or of none once it lifts, a list.
    $points = @(if ($Type -ne "touchEnd") { @{ x = $X; y = $Y; id = 1; radiusX = 6; radiusY = 6; force = 1 } })
    Invoke-Cdp $page "Input.dispatchTouchEvent" @{ type = $Type; touchPoints = $points } | Out-Null
}

# Waits while the page keeps drawing frames.
function Wait-Page([double]$Seconds) {
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 100
        Invoke-PageScript $page "return true;" | Out-Null
    }
}

# What the page counted so far (web/analytics.js), by name.
function Get-Counted {
    return @(Invoke-PageScript $page "return (window.__analytics || []).map(event => event.name);")
}

function Save-Screenshot([string]$Name) {
    $shot = Invoke-Cdp $page "Page.captureScreenshot" @{ format = "png" }
    $path = Join-Path $out $Name
    [IO.File]::WriteAllBytes($path, [Convert]::FromBase64String($shot.data))
    Write-Host "wrote $path"
}

$server = $null
$page = $null
try {
    $server = Start-Process pwsh -PassThru -WindowStyle Hidden -ArgumentList @(
        "-NoProfile", "-File", (Join-Path $PSScriptRoot "serve_web.ps1"), "-Port", $Port, "-Root", $siteDir
    )
    $url = "http://localhost:$Port/"
    $deadline = (Get-Date).AddSeconds(20)
    while ($true) {
        try {
            Invoke-WebRequest -Uri $url -Method Head -TimeoutSec 2 | Out-Null
            break
        } catch {
            if ($server.HasExited -or (Get-Date) -gt $deadline) {
                throw "The web server did not start on port $Port (is the port taken? try -Port)."
            }
            Start-Sleep -Milliseconds 200
        }
    }

    $page = Start-HeadlessBrowser $Browser "touch"
    $page.OnEvent = {
        param($message)
        $params = $message.params
        if ($message.method -eq "Runtime.exceptionThrown") {
            $errors.Add("exception: $($params.exceptionDetails.text) $($params.exceptionDetails.exception.description)")
        } elseif ($message.method -eq "Runtime.consoleAPICalled" -and $params.type -in @("error", "warning", "assert")) {
            $errors.Add("console $($params.type): $(($params.args | ForEach-Object { $_.value ?? $_.description }) -join ' ')")
        }
    }
    Invoke-Cdp $page "Runtime.enable" | Out-Null
    Invoke-Cdp $page "Emulation.setDeviceMetricsOverride" @{
        width = $width; height = $height; deviceScaleFactor = 3; mobile = $true
    } | Out-Null
    Invoke-Cdp $page "Emulation.setTouchEmulationEnabled" @{ enabled = $true; maxTouchPoints = 5 } | Out-Null
    Invoke-Cdp $page "Page.navigate" @{ url = $url } | Out-Null
    # Waits until the page answers `$Script` with true.
    function Wait-Until([string]$Script, [string]$What) {
        $deadline = (Get-Date).AddSeconds(30)
        while ($true) {
            $ready = $false
            try {
                $ready = Invoke-PageScript $page $Script
            } catch {
                # The page was still being replaced.
            }
            if ($ready) {
                return
            }
            if ((Get-Date) -gt $deadline) {
                throw "$What within 30 s."
            }
            Start-Sleep -Milliseconds 250
        }
    }
    Wait-Until "return document.readyState === 'complete' && !!document.getElementById('start');" "The page did not load"
    # Start as a finger does: a tap on the button.
    $box = Invoke-PageScript $page "const r = document.getElementById('start').getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 };"
    Send-Touch "touchStart" $box.x $box.y
    Send-Touch "touchEnd" $box.x $box.y
    Wait-Until "return !!window.wasm_exports;" "The simulation did not start"
    # The body settles; then the frames drawn in a second, since a swing
    # needs the page's frames running to be one.
    Wait-Page 1
    $fps = Invoke-PageScript $page @"
return await new Promise(done => {
  const start = performance.now();
  let frames = 0;
  const frame = () => { frames++; if (performance.now() - start < 1000) { requestAnimationFrame(frame); } else { done(frames); } };
  requestAnimationFrame(frame);
});
"@
    if ($fps -lt 30) {
        $problems.Add("The page drew only $fps frames a second, too few for a swipe to be a swing.")
    }

    # The chest lies about here in a phone's window, the body drawn about
    # 0.62 of its height tall between the HUD and the buttons.
    $bodyHeight = $height * 0.62
    $chest = @{ x = $width * 0.52; y = $height * 0.31 }
    Send-Touch "touchStart" $chest.x $chest.y
    Wait-Page 0.6
    Send-Touch "touchEnd" $chest.x $chest.y
    Wait-Page 0.8
    Save-Screenshot "first-touch.png"
    if ((Get-Counted) -contains "hit") {
        $problems.Add("The first touch on the chest struck the body; it should take the hammer up and pass through.")
    }

    # A firm swing into the belly, below the arms raised in the guard, so
    # the hammer is taken up clear of the body: from up to 0.6 of the
    # body's height beside it to its middle in about 0.2 s (about 5 m/s),
    # whatever size the body is drawn at. On a narrow screen it starts at
    # the screen's edge, which a finger cannot go past.
    $belly = $height * 0.39
    $from = @{ x = [Math]::Max(4, $chest.x - 0.6 * $bodyHeight); y = $belly }
    $to = @{ x = $chest.x + 0.05 * $bodyHeight; y = $belly }
    Send-Touch "touchStart" $from.x $from.y
    Wait-Page 0.5
    # Each move also waits for the browser's reply, about 10 ms.
    for ($i = 1; $i -le 10; $i++) {
        Send-Touch "touchMove" ($from.x + ($to.x - $from.x) * $i / 10) $from.y
        Start-Sleep -Milliseconds 8
    }
    Wait-Page 0.5
    Send-Touch "touchEnd" $to.x $to.y
    Wait-Page 1.5
    Save-Screenshot "swipe.png"
    $counted = Get-Counted
    if ($counted -notcontains "tool-sledgehammer") {
        $problems.Add("The swipe across the body did not strike it (the page counted: $($counted -join ', ')).")
    }
} finally {
    Stop-HeadlessBrowser $page
    if ($server -and -not $server.HasExited) {
        Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    }
}

foreach ($line in $errors) {
    $problems.Add("The page logged $line")
}
Write-Host ""
Write-Host "Touch on a $Size phone, $fps fps:"
if ($problems.Count -eq 0) {
    Write-Host "  the first touch took the hammer up without striking, the swipe struck the body, and the console stayed clean."
} else {
    $problems | ForEach-Object { Write-Host "  FAIL: $_" }
    exit 1
}
